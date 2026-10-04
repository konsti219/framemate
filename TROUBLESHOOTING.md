# Troubleshooting

If you ran into any issues installing or using FrameMate, check out this file for possible fixes. If the issue persists please open an issue here on GitHub. 

The more information you can give me the better, this page will also describe how to access more info for troubleshooting. 

## Common issues

Before going any further, please ensure that you are on an up to date version of SteamOS. 
Also make sure you followed the installation guide fully.

Run `flatpak run --user dev.framemate.Agent check` on the frame (ssh / console in desktop mode) for a detailed report of what might be the issue.

## Connection issues

Make sure of the following:

- Both the frame and the phone running the app need to be in the same (local) network.
- There is no firewall or router settings blocking communication between app and frame.
- Port 7380 needs to be open on the frame, this is the case by default, if you installed or configured a firewall you'll need to open that port.
- Check whether you can reach the (debug) web interface, you can access it by visiting http://frame.local:7380/healthz or http://<frame-ip>:7380/healthz in your browser.
- mDNS used for resolving frame.local might be unreliable in some cases, use the plain ip from the frame instead
- Android 17 (SDK 37) blocks local network access by default for apps that target it. FrameMate declares `ACCESS_LOCAL_NETWORK`, if it still won't connect grant the permission under Settings > Apps > FrameMate > Permissions > Local network (it is part of the Nearby devices group, you may have to open that submenu). If you deny it, the connection will fail silently with a timeout.
 
 Note: Some guest or mesh wifi networks may isolate devices by default, make sure that isn't the issue before proceeding.

## Installation issues

If you encounter an error during installation please send me the logs and the commands you ran in a github issue.
Should the app say "Wrong token", get it with `flatpak run --user dev.framemate.Agent token`. Note that reinstalling keeps the token.

### Installing from Desktop Mode

If `install-service` says "systemd isn't reachable from this terminal": Desktop Mode on the Frame is a nested desktop without access to your user's systemd, so
`install-service` can't start the agent from there. The agent is still installed and starts with
the next restart of the Frame. To start it right away, run the command it prints:

```sh
env XDG_RUNTIME_DIR=/run/user/$(id -u) DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus \
  flatpak run --user dev.framemate.Agent install-service
```

Installing over SSH doesn't have this problem.

## Other issues

You encountered a different issue or think you found a bug, please let me know!
Include steps to reproduce, the output of the agents `check` command and the logs (`journalctl --user -n 200 _COMM=framemate-agent`).
