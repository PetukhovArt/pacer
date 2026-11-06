#!/bin/sh
set -eu

mkdir -p "$BERTH_HOME" "$BERTH_RUNTIME_DIR" "$CLAUDE_CONFIG_DIR" "$(dirname "$GIT_CONFIG_GLOBAL")" /workspace
chmod 0700 "$BERTH_HOME" "$BERTH_RUNTIME_DIR"
rm -f "$BERTH_RUNTIME_DIR/daemon.sock" "$BERTH_RUNTIME_DIR/daemon.pid"

(
  while true; do
    berth daemon --foreground || true
    echo "berth-entrypoint: berth daemon exited, restarting" >&2
    sleep 2
  done
) &

exec berth-server "$@"
