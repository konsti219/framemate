import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.components as PlasmaComponents
import org.kde.kirigami as Kirigami

PlasmoidItem {
    id: root

    property var st: parseJson(Plasmoid.configuration.lastState)
    property real lastOkMs: Number(Plasmoid.configuration.lastOkMs) || 0
    property bool online: false
    property real now: Date.now()

    // Steam's percentage (what the headset shows); the raw fuel gauge reads a bit lower
    readonly property real level: {
        if (!st) return NaN;
        const b = st.steam?.topics?.battery;
        if (b && b.level >= 0) return b.level * 100;
        return st.power?.battery?.capacity_percent ?? NaN;
    }
    readonly property bool charging: !!st && (st.power?.battery?.status === "Charging" || st.steam?.topics?.battery?.ac_state === 2)
    readonly property bool asleep: !!st && !!st.sleep?.asleep
    // Unreachable for longer than a few polls: show the last value, aged by the sleep drain
    readonly property bool stale: !online && lastOkMs > 0
    readonly property real sinceMs: asleep && st.sleep.asleep_since_ms ? st.sleep.asleep_since_ms : lastOkMs
    readonly property real shownLevel: {
        if (isNaN(level)) return NaN;
        if (!stale || charging) return level;
        const hours = (now - sinceMs) / 3600000;
        return Math.max(0, level - hours * Plasmoid.configuration.sleepDrainPerHour);
    }
    readonly property string levelText: isNaN(shownLevel) ? "–" : (stale ? "~" : "") + Math.round(shownLevel) + "%"
    readonly property var controllers: (st?.steam?.topics?.vr_devices ?? []).filter(d => d.device_class === 2)

    function parseJson(s) {
        try { return s ? JSON.parse(s) : null; } catch (e) { return null; }
    }

    function time(ms) {
        return ms ? Qt.formatTime(new Date(ms), "HH:mm") : "–";
    }

    function duration(s) {
        if (s === null || s === undefined || s < 0) return "";
        const h = Math.floor(s / 3600), m = Math.round((s % 3600) / 60);
        return h > 0 ? i18n("%1 h %2 min", h, m) : i18n("%1 min", m);
    }

    function poll() {
        const c = Plasmoid.configuration;
        if (!c.host || !c.token) return;
        const xhr = new XMLHttpRequest();
        abortTimer.xhr = xhr;
        abortTimer.restart();
        xhr.onreadystatechange = () => {
            if (xhr.readyState !== XMLHttpRequest.DONE) return;
            abortTimer.stop();
            const s = xhr.status === 200 ? parseJson(xhr.responseText) : null;
            online = !!s;
            now = Date.now();
            if (!s) return;
            const changed = !st || Math.round(level) !== Math.round(s.steam?.topics?.battery?.level * 100)
                || !!st.sleep?.asleep !== !!s.sleep?.asleep;
            st = s;
            lastOkMs = now;
            // Saved sparingly: config writes hit the disk
            if (changed || now - Number(c.lastOkMs) > 300000) {
                c.lastState = xhr.responseText;
                c.lastOkMs = String(now);
            }
        };
        xhr.open("GET", "http://" + c.host + ":" + c.port + "/api/state");
        xhr.setRequestHeader("Authorization", "Bearer " + c.token);
        xhr.send();
    }

    Timer {
        id: abortTimer
        property var xhr
        interval: 5000
        onTriggered: if (xhr) xhr.abort()
    }

    Timer {
        interval: Math.max(2, Plasmoid.configuration.pollSeconds) * 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: poll()
    }

    Plasmoid.icon: Qt.resolvedUrl("../icons/vr-headset.svg")
    Plasmoid.status: isNaN(level) ? PlasmaCore.Types.PassiveStatus : PlasmaCore.Types.ActiveStatus
    toolTipMainText: i18n("Steam Frame: %1", levelText)
    toolTipSubText: {
        if (!Plasmoid.configuration.token) return i18n("Set the address and token in the settings");
        if (stale && asleep) return i18n("Asleep since %1, estimated", time(sinceMs));
        if (stale) return i18n("Unreachable since %1", time(lastOkMs));
        if (charging) return i18n("Charging") + (st.power?.battery?.time_to_full_s ? ", " + i18n("full in %1", duration(st.power.battery.time_to_full_s)) : "");
        return i18n("On battery");
    }

    compactRepresentation: MouseArea {
        Layout.minimumWidth: row.implicitWidth
        onClicked: root.expanded = !root.expanded
        RowLayout {
            id: row
            anchors.fill: parent
            spacing: Kirigami.Units.smallSpacing
            opacity: root.stale ? 0.6 : 1
            Kirigami.Icon {
                source: Plasmoid.icon
                isMask: true
                Layout.preferredWidth: Kirigami.Units.iconSizes.small
                Layout.preferredHeight: Kirigami.Units.iconSizes.small
            }
            PlasmaComponents.Label {
                text: root.levelText
            }
            Kirigami.Icon {
                visible: root.charging && !root.stale
                source: "flash"
                isMask: true
                Layout.preferredWidth: Kirigami.Units.iconSizes.small
                Layout.preferredHeight: Kirigami.Units.iconSizes.small
            }
        }
    }

    fullRepresentation: ColumnLayout {
        Layout.minimumWidth: Kirigami.Units.gridUnit * 16
        spacing: Kirigami.Units.smallSpacing

        Kirigami.Heading {
            level: 3
            text: i18n("Steam Frame: %1", root.levelText)
        }
        PlasmaComponents.Label {
            text: root.toolTipSubText
            opacity: 0.8
        }
        PlasmaComponents.Label {
            visible: root.online && !!root.st?.power?.battery
            text: {
                const b = root.st?.power?.battery;
                if (!b) return "";
                const w = Math.abs(b.power_w ?? 0).toFixed(1);
                return root.charging ? i18n("Charging at %1 W", w) : i18n("Using %1 W", w) + (b.time_to_empty_s ? ", " + i18n("%1 left", root.duration(b.time_to_empty_s)) : "");
            }
        }
        Repeater {
            model: root.controllers
            PlasmaComponents.Label {
                required property var modelData
                text: (modelData.model || i18n("Controller")) + ": "
                    + (modelData.battery >= 0 ? Math.round(modelData.battery * 100) + "%" : "–")
                    + (modelData.charging ? " " + i18n("(charging)") : "")
                    + (modelData.connected ? "" : " · " + i18n("last seen %1", root.time(modelData.last_seen_ms)))
                opacity: modelData.connected ? 1 : 0.6
            }
        }
        PlasmaComponents.Label {
            text: i18n("Updated %1", root.time(root.lastOkMs))
            opacity: 0.6
            font: Kirigami.Theme.smallFont
        }
    }
}
