#!/bin/sh
set -eu

mkdir -p "$VEYRA_HOME" "$PACER_RUNTIME_DIR" "$CLAUDE_CONFIG_DIR" "$(dirname "$GIT_CONFIG_GLOBAL")" /workspace
chmod 0700 "$VEYRA_HOME" "$PACER_RUNTIME_DIR"
rm -f "$PACER_RUNTIME_DIR/daemon.sock" "$PACER_RUNTIME_DIR/daemon.pid"

(
  while true; do
    pacer daemon --foreground || true
    echo "veyra-entrypoint: pacer daemon exited, restarting" >&2
    sleep 2
  done
) &

exec veyra-server "$@"
