// Injected into Steam's SharedJSContext (cdp.rs); reports one snapshot per topic via
// `__framemateEmit`. Undocumented Steam internals: guard every access, report failures as
// {topic: "error"}. Must be idempotent: it is re-evaluated on every reconnect.
(() => {
  const W = window;
  try { W.__framemate?.dispose?.(); } catch {}

  const handles = [], timers = [];
  let disposed = false;
  W.__framemate = {
    dispose() {
      disposed = true;
      for (const h of handles) try { h.unregister(); } catch {}
      for (const t of timers) clearInterval(t);
    },
  };

  const encode = v => JSON.stringify(v, (_, x) => (typeof x === "bigint" ? String(x) : x));
  const emit = (topic, data) => {
    if (disposed) return;
    try { W.__framemateEmit(encode({ topic, data })); } catch {}
  };
  const fail = (topic, e) => emit("error", { topic, message: String(e?.message ?? e) });
  const subscribe = (topic, register, handler) => {
    try {
      const h = register((...args) => {
        try { handler(...args); } catch (e) { fail(topic, e); }
      });
      if (h?.unregister) handles.push(h);
      return h;
    } catch (e) { fail(topic, e); }
  };
  const poll = (topic, ms, fn) => {
    const run = async () => {
      try {
        const data = await fn();
        if (data !== undefined) emit(topic, data);
      } catch (e) { fail(topic, e); }
    };
    run();
    timers.push(setInterval(run, ms));
  };
  const appName = appid => {
    try { return W.appStore.GetAppOverviewByAppID(Number(appid))?.display_name ?? null; } catch { return null; }
  };

  // --- battery ---
  const battery = () => subscribe("battery", cb => SteamClient.System.RegisterForBatteryStateChanges(cb), s =>
    emit("battery", {
      has_battery: s.bHasBattery,
      level: s.flLevel,
      seconds_remaining: s.nSecondsRemaining,
      ac_state: s.eACState, // 1 = on battery, 2 = on AC (observed)
      battery_state: s.eBatteryState,
    }));

  // --- sleep: Steam's own suspend preparation (cloud sync etc.) runs ~1 s before it asks logind to
  // suspend, while Wi-Fi is still up. logind's PrepareForSleep is too late: NetworkManager drops Wi-Fi then.
  const suspend = () => subscribe("suspend", cb => SteamClient.User.RegisterForPrepareForSystemSuspendProgress(cb), p =>
    emit("suspend", { at_ms: Date.now(), progress: p ?? null }));

  // --- downloads: this device only (other PCs show up via Remote Downloads) ---
  const isLocal = clientId => String(clientId) === "0";
  const downloads = () => {
    // The queue callback can stop arriving (seen after a reboot plus suspends) while the overview
    // below keeps firing, leaving the queue empty during a download. Registering again makes Steam
    // send the current queue at once, so do that whenever the overview names an app the queue
    // lacks, and once a minute anyway.
    let itemsHandle = null;
    let listed = new Set();
    const subscribeItems = () => {
      try { itemsHandle?.unregister(); } catch {}
      if (handles.includes(itemsHandle)) handles.splice(handles.indexOf(itemsHandle), 1);
      itemsHandle = subscribe("downloads", cb => SteamClient.Downloads.RegisterForDownloadItems(cb), (_flag, clients) => {
        const local = (clients ?? []).filter(c => isLocal(c.remote_client_id)).flatMap(c => c.item_data ?? []);
        listed = new Set(local.map(i => i.appid));
        emit("downloads", local.map(i => ({
          appid: i.appid,
          name: appName(i.appid),
          queue_index: i.queue_index,
          active: i.active,
          paused: i.paused,
          completed: i.completed,
          completed_time: i.completed_time,
          percent: i.overall_percent_complete ?? null,
          error: i.update_error || null,
          buildid: i.buildid,
          target_buildid: i.target_buildid,
        })));
      });
    };
    subscribeItems();
    timers.push(setInterval(subscribeItems, 60000));
    let resubscribedFor = null;
    // ~1 Hz while downloading; may describe a remote client's transfer, reported as idle.
    subscribe("download_overview", cb => SteamClient.Downloads.RegisterForDownloadOverview(cb), o => {
      if (!isLocal(o.remote_client_id)) {
        emit("download_overview", { appid: null });
        return;
      }
      if (o.update_appid && !listed.has(o.update_appid) && resubscribedFor !== o.update_appid) {
        resubscribedFor = o.update_appid;
        subscribeItems();
      }
      emit("download_overview", {
        appid: o.update_appid || null,
        name: o.update_appid ? appName(o.update_appid) : null,
        state: o.update_state || null,
        paused: o.paused,
        percent: o.overall_percent_complete,
        bytes_per_sec: o.update_network_bytes_per_second,
        peak_bytes_per_sec: o.update_peak_network_bytes_per_second,
        disc_bytes_per_sec: o.update_disc_bytes_per_second,
        eta_sec: Math.max(0, ...(o.progress ?? []).map(p => p.estimated_time_remaining_sec || 0)),
        is_install: o.update_is_install,
        is_workshop: o.update_is_workshop,
        is_shader: o.update_is_shader,
        is_upload: o.update_is_upload,
        throttle_bytes_per_sec: o.download_throttle_rate,
      });
    });
  };

  // --- running apps ---
  const runningApps = () =>
    Array.from(W.SteamUIStore?.RunningApps ?? [], a => ({ appid: a.appid, name: a.display_name ?? appName(a.appid) }));
  const apps = () => {
    poll("running_apps", 5000, runningApps);
    subscribe("last_app_event", cb => SteamClient.GameSessions.RegisterForAppLifetimeNotifications(cb), n => {
      emit("last_app_event", { appid: n.unAppID, name: appName(n.unAppID), running: n.bRunning, at_ms: Date.now(), raw: n });
      setTimeout(() => emit("running_apps", runningApps()), 500);
    });
  };

  // --- VR devices. openvr.h property ids: 1001 ModelNumber, 1002 SerialNumber,
  // 1011 DeviceIsCharging, 1012 DeviceBatteryPercentage, 1029 DeviceClass ---
  let vrPaths = [];
  const vrDevice = async path => {
    const P = SteamClient.OpenVR.DeviceProperties;
    const get = async (fn, id) => { try { return await P[fn](path, id); } catch { return null; } };
    let connected = null;
    try { connected = await SteamClient.OpenVR.Device.BIsConnected(path); } catch {}
    return {
      path,
      model: await get("GetStringDeviceProperty", 1001),
      serial: await get("GetStringDeviceProperty", 1002),
      device_class: await get("GetInt32DeviceProperty", 1029), // 1 = HMD, 2 = controller
      battery: await get("GetFloatDeviceProperty", 1012),
      charging: await get("GetBoolDeviceProperty", 1011),
      connected,
    };
  };
  const vrDevices = () => Promise.all(vrPaths.map(vrDevice));
  const vr = () => {
    subscribe("vr_devices", cb => SteamClient.OpenVR.RegisterForVRTrackedDevices(cb), paths => {
      vrPaths = Array.isArray(paths) ? paths : [];
      vrDevices().then(d => emit("vr_devices", d), e => fail("vr_devices", e));
    });
    poll("vr_devices", 15000, () => (vrPaths.length ? vrDevices() : undefined));

    const state = {};
    subscribe("vr_state", cb => SteamClient.OpenVR.RegisterForHMDActivityLevelChanged(cb), level => {
      state.hmd_activity = level;
      emit("vr_state", { ...state });
    });
    subscribe("vr_state", cb => SteamClient.OpenVR.RegisterForVRSceneAppChange(cb), appid => {
      state.scene_appid = appid || null;
      state.scene_app_name = appid ? appName(appid) : null;
      emit("vr_state", { ...state });
    });
  };

  // --- network, perf, system, user (polled) ---
  const stores = () => {
    poll("network", 10000, () => {
      const n = W.SystemNetworkStore, d = W.SystemPerfStore?.msgDiagnosticInfo;
      return {
        connected: n?.hasNetworkConnection ?? null,
        internet: n?.hasInternetConnection ?? null,
        steam: n?.hasSteamConnection ?? null,
        wifi_enabled: n?.isWifiEnabled ?? null,
        interfaces: (d?.interfaces ?? [])
          .filter(i => i.name !== "lo" && Number(i.rx_bytes_total) > 0)
          .map(i => ({
            name: i.name,
            rx_bytes_per_sec: i.rx_bytes_per_sec,
            tx_bytes_per_sec: i.tx_bytes_per_sec,
            rx_bytes_total: String(i.rx_bytes_total),
            tx_bytes_total: String(i.tx_bytes_total),
          })),
      };
    });
    poll("perf", 15000, () => {
      const p = W.SystemPerfStore, a = p?.msgState?.settings?.per_app ?? {};
      return {
        battery_temp_c: p?.nBatteryTemperatureC ?? null,
        current_game_id: p?.nCurrentGameID != null ? String(p.nCurrentGameID) : null,
        tdp_limit_w: a.is_tdp_limit_enabled ? a.tdp_limit : null,
        fps_limit: a.is_fps_limit_enabled ? a.fps_limit : null,
        low_latency_mode: a.is_low_latency_mode_enabled ?? null,
      };
    });
    poll("system_info", 600000, async () => {
      const s = await SteamClient.System.GetSystemInfo();
      return {
        os: s.sOSName,
        os_version: s.sOSVersionId,
        os_variant: s.sOSVariantId,
        os_build: s.sOSBuildId,
        kernel: s.sKernelVersion,
        hostname: s.sHostname,
        product: s.sProductVendor,
        steam_version: s.nSteamVersion,
        steam_build_date: s.sSteamBuildDate,
        gpu: s.sVideoCardName,
        gpu_driver: s.sVideoDriverVersion,
        cpu_cores: s.nCPULogicalCores,
        ram_mb: s.nSystemRAMSizeMB,
      };
    });
    poll("user", 60000, () => {
      const u = W.App?.m_CurrentUser, p = W.g_FriendsUIApp?.FriendStore?.self?.persona;
      return {
        account_name: u?.strAccountName ?? null,
        persona_name: p?.m_strPlayerName ?? null,
        persona_state: p?.m_ePersonaState ?? null, // 0 offline, 1 online, 2 busy, 3 away, ...
        offline_mode: u?.bIsOfflineMode ?? null,
      };
    });
  };

  // Steam's stores load after the context is created; wait for them.
  const start = () => {
    if (disposed) return;
    if (typeof SteamClient === "undefined" || !W.appStore || !W.SteamUIStore) {
      timers.push(setTimeout(start, 2000));
      return;
    }
    for (const section of [battery, suspend, downloads, apps, vr, stores]) {
      try { section(); } catch (e) { fail(section.name, e); }
    }
  };
  start();
  return "framemate shim installed";
})()
