# Plasma widget

A Plasma 6 panel widget showing the Steam Frame's battery (the percentage Steam shows) with a charging
indicator, and in its popup the power draw, time to full or empty, and the controllers.

It polls the agent's `GET /api/state` (default every 10 s). When the headset is unreachable, usually because
it is asleep, the widget keeps showing the last value, faded and prefixed with `~`, reduced by an
estimated drain per hour (configurable, default 1 %/h). The agent marks the state asleep about a second
before the headset suspends, so the widget can tell sleep from other outages. The last state is kept in
the widget's config, so it survives a Plasma restart.

Install by copying `dev.framemate.headset` to `~/.local/share/plasma/plasmoids/` (or
`kpackagetool6 -t Plasma/Applet -i dev.framemate.headset`), then add "Steam Frame" to a panel and set the
headset's address and the token (`framemate-agent token` on the headset) in its settings.
