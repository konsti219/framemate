#!/usr/bin/env bash
# Cross-builds the agent and runs it on the Frame as a transient user unit.
#
#   scripts/deploy.sh          build, copy, (re)start
#   scripts/deploy.sh logs     follow the agent's journal
#   scripts/deploy.sh stop     stop the agent
#
# Env: FRAME_HOST (default steamos@frame.local), FRAME_SSH_OPTS (extra ssh options,
# e.g. "-o ControlPath=/tmp/fm.sock" to reuse a master connection).
set -euo pipefail

HOST="${FRAME_HOST:-steamos@frame.local}"
UNIT=framemate-agent-dev
TARGET=aarch64-unknown-linux-musl
REMOTE_BIN='.local/lib/framemate/framemate-agent'
# Absolute path: `ssh` is often aliased (e.g. kitty's ssh kitten).
read -ra SSH_OPTS <<< "${FRAME_SSH_OPTS:-}"
ssh() { /usr/bin/ssh "${SSH_OPTS[@]}" "$HOST" "$@"; }

case "${1:-deploy}" in
  logs) exec /usr/bin/ssh "${SSH_OPTS[@]}" "$HOST" journalctl --user -u "$UNIT" -f -n 50 ;;
  stop) ssh systemctl --user stop "$UNIT"; exit ;;
  deploy) ;;
  *) echo "usage: $0 [deploy|logs|stop]" >&2; exit 2 ;;
esac

cd "$(dirname "$0")/.."
cargo build --release --target "$TARGET" -p framemate-agent

# The installed (Flatpak) service would hold port 7380; start it again with
# `systemctl --user start framemate-agent` when done.
ssh "mkdir -p \$(dirname $REMOTE_BIN); systemctl --user stop framemate-agent $UNIT 2>/dev/null; systemctl --user reset-failed $UNIT 2>/dev/null; true"
/usr/bin/scp -q "${SSH_OPTS[@]}" "target/$TARGET/release/framemate-agent" "$HOST:$REMOTE_BIN"
# Share the Flatpak's config dir, so the dev build uses the same API token.
ssh "systemd-run --user --unit=$UNIT --collect -p Restart=on-failure -p RestartSec=2 \
  --setenv=XDG_CONFIG_HOME=\$HOME/.var/app/dev.framemate.Agent/config \$HOME/$REMOTE_BIN >/dev/null"
sleep 1
ssh "systemctl --user --no-pager --lines=0 status $UNIT | head -3; journalctl --user -u $UNIT -n 5 --no-pager -o cat"
