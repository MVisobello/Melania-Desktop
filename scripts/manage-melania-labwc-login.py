#!/usr/bin/env python3
"""Explicit, reversible Melania-labwc login registration.

This is deliberately NOT an automatic installer or an update to XFCE/Cinnamon.
The system-wide entry invokes a root-owned generic launcher; each account
independently opts in with a private registration that points to its own build.
Tests invoke pure functions with a temporary filesystem root, never /usr.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile

CHECKOUT = Path(__file__).resolve().parents[1]
LAUNCH_SOURCE = CHECKOUT / "scripts/melania-labwc-login-launch"
ENTRY_PATH = Path("/usr/share/wayland-sessions/org.melaniadesktop.Labwc.desktop")
LAUNCH_PATH = Path("/usr/local/libexec/melania-labwc-launch")
USER_FILE = "labwc-login-root"
USER_MAGIC = "MELANIA_LABWC_USER_ROOT_V1"
ENTRY_MAGIC = "X-Melania-Managed-By=Melania-Login-Installer-V1"
LAUNCH_MAGIC = "# MELANIA_LOGIN_WRAPPER_V1"

ENTRY = (
    "[Desktop Entry]\n"
    "Type=Application\nVersion=1.0\n"
    "Name=Melania Desktop (Labwc)\n"
    "Comment=Independent, reversible Melania Wayland desktop session\n"
    f"Exec={LAUNCH_PATH}\n"
    "TryExec=/usr/bin/labwc\n"
    "DesktopNames=Melania\n"
    "X-Melania-Backend=labwc\n"
    "X-Melania-RustCompositorReady=false\n"
    f"{ENTRY_MAGIC}\n"
).encode("utf-8")


def refuse(message: str) -> None:
    raise ValueError(message)


def _system_path(root: Path, absolute: Path) -> Path:
    if not absolute.is_absolute():
        refuse("system destinations must be absolute")
    return root / absolute.relative_to("/")


def system_targets(root: Path, source: Path = LAUNCH_SOURCE) -> dict[Path, tuple[bytes, int, bytes]]:
    launch = source.read_bytes()
    if not launch.startswith(b"#!/usr/bin/env bash\n" + LAUNCH_MAGIC.encode() + b"\n"):
        refuse("the root-owned launcher lacks the expected Melania marker")
    if b"\0" in launch or len(launch) > 16_384:
        refuse("invalid launcher content")
    return {
        _system_path(root, LAUNCH_PATH): (launch, 0o755, LAUNCH_MAGIC.encode()),
        _system_path(root, ENTRY_PATH): (ENTRY, 0o644, ENTRY_MAGIC.encode()),
    }


def _check_parent(path: Path, root: Path) -> None:
    if not root.is_absolute():
        refuse("test/system filesystem root must be absolute")
    current = path.parent
    while current != root:
        if current.is_symlink():
            refuse(f"symbolic link in installation path: {current}")
        if current.exists() and not current.is_dir():
            refuse(f"installation parent is not a directory: {current}")
        if current == current.parent:
            refuse("system destination escapes allowed root")
        current = current.parent
    if root.is_symlink() or not root.is_dir():
        refuse("system root is not an ordinary directory")


def _managed_or_missing(path: Path, marker: bytes) -> None:
    if path.is_symlink():
        refuse(f"refusing symbolic link: {path}")
    try:
        info = path.lstat()
    except FileNotFoundError:
        return
    if not stat.S_ISREG(info.st_mode):
        refuse(f"refusing non-regular destination: {path}")
    # Only previously installed, clearly marked Melania-owned files can
    # be upgraded or removed. Never replace unrelated display-manager data.
    if marker not in path.read_bytes()[:16_384]:
        refuse(f"existing file is not managed by Melania: {path}")


def system_install(root: Path, source: Path = LAUNCH_SOURCE) -> list[Path]:
    targets = system_targets(root, source)
    # Preflight ALL destinations before the first modification.
    for path, (_, _, marker) in targets.items():
        _check_parent(path, root)
        _managed_or_missing(path, marker)
    for path, (content, mode, _) in targets.items():
        path.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
        if path.exists() and path.read_bytes() == content and stat.S_IMODE(path.stat().st_mode) == mode:
            continue
        fd, temp = tempfile.mkstemp(prefix=".melania-session-", dir=path.parent)
        try:
            with os.fdopen(fd, "wb") as handle:
                handle.write(content)
                handle.flush()
                os.fchmod(handle.fileno(), mode)
                os.fsync(handle.fileno())
            os.replace(temp, path)
        finally:
            if os.path.exists(temp):
                os.unlink(temp)
    return list(targets)


def system_uninstall(root: Path, source: Path = LAUNCH_SOURCE) -> list[Path]:
    targets = system_targets(root, source)
    # Fail atomically on any foreign or symlinked destination.
    for path, (_, _, marker) in targets.items():
        _check_parent(path, root)
        _managed_or_missing(path, marker)
    removed: list[Path] = []
    for path in targets:
        if path.is_file():
            path.unlink()
            removed.append(path)
    return removed


def user_config_dir(config_root: Path) -> Path:
    if not config_root.is_absolute() or config_root.is_symlink():
        refuse("user XDG config home must be absolute and not a symlink")
    folder = config_root / "melania"
    if folder.is_symlink():
        refuse("Melania configuration directory must not be a symlink")
    if folder.exists():
        info = folder.stat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.geteuid() or stat.S_IMODE(info.st_mode) & 0o077:
            refuse("Melania configuration directory must be owned and mode 0700")
    return folder


def _check_user_file(path: Path) -> bool:
    if path.is_symlink():
        refuse("refusing a symlinked registration")
    if not path.exists():
        return False
    info = path.stat()
    if not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid() or stat.S_IMODE(info.st_mode) & 0o077:
        refuse("registration file must be owned and mode 0600")
    contents = path.read_text(encoding="utf-8")
    if not contents.startswith(USER_MAGIC + "\n"):
        refuse("existing registration is not managed by Melania")
    return True


def register_user(config_root: Path, checkout: Path, *, verify: bool = True) -> Path:
    if os.geteuid() == 0 and verify:
        refuse("register the account while logged in as that user, not with sudo")
    checkout = checkout.resolve(strict=True)
    launcher = checkout / "scripts/melania-labwc-session"
    if not launcher.is_file() or launcher.is_symlink():
        refuse("Melania checkout has no regular session launcher")
    if "\n" in str(checkout) or "\r" in str(checkout):
        refuse("invalid checkout path")
    if verify:
        staged = checkout / "target/melania-labwc-session/share/applications/org.melaniadesktop.Files.desktop"
        if not staged.is_file():
            refuse("first build Melania and run scripts/melania-labwc-session --stage")
        check = subprocess.run(["bash", str(launcher), "--check"],
                               capture_output=True, text=True, timeout=30, check=False)
        if check.returncode != 0:
            refuse("Melania standalone preflight failed: " + check.stderr.strip()[:500])
    folder = user_config_dir(config_root)
    folder.mkdir(parents=True, mode=0o700, exist_ok=True)
    path = folder / USER_FILE
    _check_user_file(path)
    value = f"{USER_MAGIC}\n{checkout}\n".encode("utf-8")
    if path.exists() and path.read_bytes() == value:
        return path
    fd, temp = tempfile.mkstemp(prefix=".melania-login-", dir=folder)
    try:
        with os.fdopen(fd, "wb") as handle:
            handle.write(value)
            os.fchmod(handle.fileno(), 0o600)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temp, path)
    finally:
        if os.path.exists(temp):
            os.unlink(temp)
    return path


def unregister_user(config_root: Path) -> bool:
    folder = user_config_dir(config_root)
    path = folder / USER_FILE
    if not _check_user_file(path):
        return False
    path.unlink()
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--register-user", action="store_true")
    action.add_argument("--unregister-user", action="store_true")
    action.add_argument("--install-system", action="store_true")
    action.add_argument("--uninstall-system", action="store_true")
    action.add_argument("--status", action="store_true")
    args = parser.parse_args()
    config_root = Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config")))
    try:
        if args.register_user:
            path = register_user(config_root, CHECKOUT)
            print(f"MELANIA_LOGIN_USER_REGISTERED={path}")
        elif args.unregister_user:
            print(f"MELANIA_LOGIN_USER_UNREGISTERED={unregister_user(config_root)}")
        elif args.install_system:
            if os.geteuid() != 0:
                refuse("--install-system requires an explicit administrator action (sudo)")
            for path in system_install(Path("/")):
                print(f"MELANIA_LOGIN_SYSTEM_INSTALLED={path}")
        elif args.uninstall_system:
            if os.geteuid() != 0:
                refuse("--uninstall-system requires an explicit administrator action (sudo)")
            for path in system_uninstall(Path("/")):
                print(f"MELANIA_LOGIN_SYSTEM_REMOVED={path}")
        else:
            system = all(path.is_file() and not path.is_symlink() and marker in path.read_bytes()[:16_384]
                         for path, (_, _, marker) in system_targets(Path("/")).items())
            registered = _check_user_file(user_config_dir(config_root) / USER_FILE)
            print(f"MELANIA_LOGIN_SYSTEM_REGISTERED={str(system).lower()}")
            print(f"MELANIA_LOGIN_USER_REGISTERED={str(registered).lower()}")
        return 0
    except (ValueError, OSError, subprocess.TimeoutExpired) as error:
        print(f"MELANIA_LOGIN_BLOCKED={error}", file=sys.stderr)
        return 78


if __name__ == "__main__":
    raise SystemExit(main())
