#!/usr/bin/env bash
# Real GIO/GVfs SFTP provider smoke, isolated to a GitHub runner loopback.
set -euo pipefail
[[ "$CI" == true && "$GITHUB_ACTIONS" == true ]] || {
  echo "FAIL: isolated GitHub Actions runner required" >&2
  exit 1
}
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
TEMP="$(mktemp -d)"
OLD_HOME="$HOME"
SSHD_PID=''
AGENT_PID=''
# The SSH config is isolated under a temporary HOME, but Rustup/Cargo must
# retain the toolchain installed by dtolnay/rust-toolchain on the CI runner.
export RUSTUP_HOME="$OLD_HOME/.rustup"
export CARGO_HOME="$OLD_HOME/.cargo"
cleanup() {
  set +e
  if [[ -n "$SSHD_PID" ]]; then sudo kill "$SSHD_PID" 2>/dev/null || true; fi
  if [[ -n "$AGENT_PID" ]]; then kill "$AGENT_PID" 2>/dev/null || true; fi
  HOME="$OLD_HOME" rm -rf -- "$TEMP"
}
trap cleanup EXIT

mkdir -p "$TEMP/home/.ssh" "$TEMP/fixture/target"
chmod 700 "$TEMP/home/.ssh"
printf 'melania-sftp-sample\n' > "$TEMP/fixture/melania-sftp-fixture.txt"
[[ "$(wc -c < "$TEMP/fixture/melania-sftp-fixture.txt")" == 20 ]] || exit 1
ssh-keygen -q -t ed25519 -N '' -f "$TEMP/client-key"
ssh-keygen -q -t ed25519 -N '' -f "$TEMP/host-key"
cp "$TEMP/client-key.pub" "$TEMP/authorized_keys"
chmod 600 "$TEMP/authorized_keys"
PORT="$(python3 - <<'PY'
import socket
with socket.socket() as sock:
    sock.bind(('127.0.0.1',0))
    print(sock.getsockname()[1])
PY
)"
cat > "$TEMP/sshd.conf" <<EOF
Port $PORT
ListenAddress 127.0.0.1
HostKey $TEMP/host-key
PidFile $TEMP/sshd.pid
AuthorizedKeysFile $TEMP/authorized_keys
Subsystem sftp internal-sftp
PasswordAuthentication no
KbdInteractiveAuthentication no
PubkeyAuthentication yes
StrictModes no
UsePAM yes
PermitRootLogin no
AllowUsers $(id -un)
LogLevel DEBUG3
EOF
sudo mkdir -p /run/sshd
sudo /usr/sbin/sshd -D -e -f "$TEMP/sshd.conf" > "$TEMP/sshd.log" 2>&1 &
SSHD_PID=$!
export HOME="$TEMP/home"
cat > "$HOME/.ssh/config" <<EOF
Host 127.0.0.1
  User $(id -un)
  Port $PORT
  IdentityFile $TEMP/client-key
  StrictHostKeyChecking no
  UserKnownHostsFile /dev/null
  BatchMode yes
EOF
chmod 600 "$HOME/.ssh/config"
eval "$(ssh-agent -s -a "$TEMP/ssh-agent.sock")" >/dev/null
AGENT_PID="$SSH_AGENT_PID"
ssh-add "$TEMP/client-key" >/dev/null
READY=0
for _ in $(seq 1 35); do
  if timeout 3 ssh -F "$HOME/.ssh/config" 127.0.0.1 'true' >/dev/null 2>&1; then
    READY=1
    break
  fi
  sleep .2
done
if [[ "$READY" != 1 ]]; then
  echo 'FAIL: loopback sshd did not accept the temporary key' >&2
  ssh -vv -F "$HOME/.ssh/config" 127.0.0.1 'true' 2>&1 |
    grep -Ei 'Offering public key|Authentication|Permission denied|Connection|refused|key type|host key' |
    tail -24 >&2 || true
  tail -60 "$TEMP/sshd.log" >&2
  exit 1
fi
export GIO_USE_VFS=gvfs
export MELANIA_TEST_SFTP_URI="sftp://$(id -un)@127.0.0.1:$PORT$TEMP/fixture"
echo "PASS: isolated SFTP transport with temporary SSH public-key authentication"
dbus-run-session -- bash -c '
  set -euo pipefail
  timeout 40 gio mount "$MELANIA_TEST_SFTP_URI"
  timeout 20 gio list "$MELANIA_TEST_SFTP_URI" | grep -qx "melania-sftp-fixture.txt"
  timeout 150 cargo test -p melania-files --bin melania-files \
      live_gvfs_sftp_loopback_fixture -- --ignored --nocapture
  gio mount -u "$MELANIA_TEST_SFTP_URI" >/dev/null 2>&1 || true
'
echo 'PASS: authenticated GVfs SFTP listing, metadata, copy and no-overwrite verified'
