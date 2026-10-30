#!/usr/bin/env bash
# D-Bus smoke test uses an isolated temporary session bus; never overrides
# org.freedesktop.Notifications on the user's main desktop session.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [[ "${1:-}" == "--inside" ]]; then
    export XDG_RUNTIME_DIR="$MELANIA_SMOKE_DIR/runtime"
    target/debug/melania-notifyd >"$MELANIA_SMOKE_DIR/daemon.log" 2>&1 &
    pid=$!
    trap 'kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true' EXIT
    for i in $(seq 1 40); do
        if busctl --user status org.freedesktop.Notifications >/dev/null 2>&1; then break; fi
        kill -0 "$pid" || { cat "$MELANIA_SMOKE_DIR/daemon.log" >&2; exit 1; }
        sleep .15
    done
    busctl --user status org.freedesktop.Notifications >/dev/null
    notify-send -a MelaniaSmoke "Melania test" "Notification body"
    for i in $(seq 1 20); do
        if python3 - <<'PY' >/dev/null 2>&1
import json,os
p=os.path.join(os.environ['XDG_RUNTIME_DIR'],'melania','notifications.json')
with open(p) as f:
    data=json.load(f)
assert any(n['summary']=='Melania test' and n['app']=='MelaniaSmoke' for n in data['notifications'])
PY
        then
            count="$(target/debug/melania-notifyctl unread)"
            [[ "$count" == "1" ]] || { echo "FAIL: expected one unread notification, got $count" >&2; exit 1; }
            target/debug/melania-notifyctl mark-read
            [[ "$(target/debug/melania-notifyctl unread)" == "0" ]] || { echo "FAIL: mark read did not persist" >&2; exit 1; }
            id="$(python3 - <<'PYINNER'
import json, os
p=os.path.join(os.environ['XDG_RUNTIME_DIR'],'melania','notifications.json')
with open(p, encoding='utf-8') as f: data=json.load(f)
assert data['notifications'][0]['unread'] is False
print(data['notifications'][0]['id'])
PYINNER
)"
            target/debug/melania-notifyctl close "$id"
            for attempt in $(seq 1 20); do
                if python3 - <<'PYCLOSE' >/dev/null 2>&1
import json, os
p=os.path.join(os.environ['XDG_RUNTIME_DIR'],'melania','notifications.json')
with open(p, encoding='utf-8') as f: data=json.load(f)
assert not data['notifications']
PYCLOSE
                then echo 'PASS: D-Bus reception, read acknowledgment, close and persisted snapshot'; exit 0; fi
                sleep .15
            done
            echo 'FAIL: close did not update the snapshot' >&2
            exit 1
        fi
        sleep .15
    done
    cat "$MELANIA_SMOKE_DIR/daemon.log" >&2
    echo 'FAIL: notification missing from snapshot' >&2
    exit 1
fi
for binary in dbus-run-session busctl notify-send python3; do
    command -v "$binary" >/dev/null || { echo "BLOCKED: missing $binary" >&2; exit 3; }
done
[[ -x target/debug/melania-notifyd ]] && [[ -x target/debug/melania-notifyctl ]] || { echo 'BLOCKED: cargo build -p melania-notifyd -p melania-notifyctl' >&2; exit 3; }
WORK="$(mktemp -d -t melania-notify-XXXXXX)"
trap 'rm -rf "$WORK"' EXIT
mkdir -m 700 "$WORK/runtime"
export MELANIA_SMOKE_DIR="$WORK"
dbus-run-session -- "$0" --inside
