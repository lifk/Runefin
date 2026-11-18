#!/usr/bin/env python3
"""Install Runefin and its Steam tile on a SteamOS / Steam Machine host.

This is a host-side installer. A Flatpak has no post-install hook and cannot write Steam's
`shortcuts.vdf`, so integration belongs here:

  * write a wrapper that survives Steam's LD_LIBRARY_PATH/LD_PRELOAD launch environment
  * write a host .desktop launcher
  * ask the running Steam client to add that launcher
  * read the shortcut's appid back and install portrait/header/hero/logo/icon art
  * --uninstall removes the wrapper, launcher, art and our record; the Steam entry itself is
    removed from Steam's UI (we never rewrite shortcuts.vdf while Steam owns it)

Usage:
  python3 scripts/steam-install.py [--dry-run] [--no-install] [--no-steam]
  python3 scripts/steam-install.py --uninstall

It never runs as root, never edits /usr or /etc, and never touches another shortcut.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path
from urllib.parse import quote

APP_ID = "io.github.lifk.Runefin"
APP_NAME = "Runefin"
DEFAULT_REF = "https://lifk.github.io/Runefin/runefin.flatpakref"
ART_BASE = "https://raw.githubusercontent.com/lifk/Runefin/main/packaging/steam/art"
SENTINEL_NAME = "addnonsteamgamefile"

DATA_HOME = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share")
RUNEFIN_DIR = DATA_HOME / "runefin"
WRAPPER = RUNEFIN_DIR / "runefin-steam.sh"
LAUNCHER = DATA_HOME / "applications" / "runefin-steam.desktop"
RECORD = RUNEFIN_DIR / "steam.json"
DECKY_PLUGIN = Path.home() / "homebrew" / "plugins" / "Runefin"
DECKY_BASE = "https://raw.githubusercontent.com/lifk/Runefin/main/packaging/decky/runefin"
DECKY_FILES = ("plugin.json", "main.py", "package.json", "dist/index.js")


def say(message: str) -> None:
    print(message, flush=True)


def run(command: list[str], dry: bool, timeout: int = 120) -> int:
    printable = " ".join(command)
    if dry:
        say(f"would run: {printable}")
        return 0
    say(f"run: {printable}")
    return subprocess.run(command, timeout=timeout).returncode


def flatpak_installed() -> bool:
    if shutil.which("flatpak") is None:
        return False
    return (
        subprocess.run(
            ["flatpak", "info", APP_ID],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        ).returncode
        == 0
    )


def install_flatpak(ref: str, dry: bool, skip: bool) -> bool:
    if skip:
        return True
    if flatpak_installed():
        say(f"Flatpak {APP_ID} is already installed.")
        return True
    if shutil.which("flatpak") is None:
        say("flatpak is not installed. Install it first, or use --no-install.")
        return False
    say("Flatpak is not installed; installing the signed repository reference.")
    if run(["flatpak", "install", "--user", "-y", ref], dry) != 0:
        say("Could not install the Flatpak. Run the command above, then rerun this script.")
        return False
    return True


def steam_roots() -> list[Path]:
    home = Path.home()
    return [
        home / ".local/share/Steam",
        home / ".steam/steam",
        home / ".var/app/com.valvesoftware.Steam/.local/share/Steam",
        home / ".var/app/com.valvesoftware.Steam/.steam/steam",
    ]


def find_steam() -> tuple[Path, str] | None:
    """The Steam root and the command that talks to the running client."""
    for root in steam_roots():
        if not root.is_dir():
            continue
        inside_flatpak = ".var/app/com.valvesoftware.Steam" in str(root)
        command = "flatpak" if inside_flatpak else "steam"
        if inside_flatpak:
            if shutil.which("flatpak") is None:
                continue
        elif shutil.which("steam") is None:
            continue
        return root, command
    return None


def account_from_loginusers(root: Path) -> str | None:
    login = root / "config/loginusers.vdf"
    if not login.is_file():
        return None
    try:
        text = login.read_text(errors="replace")
    except OSError:
        return None
    # loginusers.vdf is text VDF. Prefer the account with MostRecent "1"; otherwise take the
    # first id.
    blocks = re.findall(r'"(\d+)"\s*\{(.*?)\n\s*\}', text, re.S)
    for steam_id, body in blocks:
        if re.search(r'"MostRecent"\s*"1"', body):
            return steam_id
    return blocks[0][0] if blocks else None


def active_account(root: Path) -> Path | None:
    userdata = root / "userdata"
    if not userdata.is_dir():
        return None
    wanted = account_from_loginusers(root)
    if wanted:
        candidate = userdata / wanted
        if candidate.is_dir():
            return candidate
    candidates = [
        child
        for child in userdata.iterdir()
        if child.is_dir() and (child / "config/shortcuts.vdf").is_file()
    ]
    if not candidates:
        return None
    return max(candidates, key=lambda child: (child / "config/shortcuts.vdf").stat().st_mtime)


def steam_command(root: Path) -> list[str]:
    if ".var/app/com.valvesoftware.Steam" in str(root):
        return ["flatpak", "run", "com.valvesoftware.Steam"]
    return ["steam"]


def steam_running(root: Path) -> bool:
    pattern = "com.valvesoftware.Steam" if ".var/app" in str(root) else "steamwebhelper"
    try:
        result = subprocess.run(
            ["pgrep", "-f", pattern],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return result.returncode == 0
    except OSError:
        return False


def c_string(data: bytes, offset: int) -> tuple[str, int]:
    end = data.index(b"\x00", offset)
    return data[offset:end].decode("utf-8", "replace"), end + 1


def parse_vdf_map(data: bytes, offset: int) -> tuple[dict[str, object], int]:
    result: dict[str, object] = {}
    while offset < len(data) and data[offset] != 0x08:
        kind = data[offset]
        offset += 1
        if kind == 0x00:
            key, offset = c_string(data, offset)
            value, offset = parse_vdf_map(data, offset)
            result[key] = value
        elif kind == 0x01:
            key, offset = c_string(data, offset)
            value, offset = c_string(data, offset)
            result[key] = value
        elif kind == 0x02:
            key, offset = c_string(data, offset)
            if offset + 4 > len(data):
                raise ValueError("truncated int")
            result[key] = int.from_bytes(data[offset : offset + 4], "little", signed=True)
            offset += 4
        elif kind == 0x07:
            key, offset = c_string(data, offset)
            if offset + 8 > len(data):
                raise ValueError("truncated int64")
            result[key] = int.from_bytes(data[offset : offset + 8], "little", signed=True)
            offset += 8
        else:
            raise ValueError(f"unknown VDF type {kind:#x}")
    if offset >= len(data):
        raise ValueError("unterminated VDF map")
    return result, offset + 1


def read_shortcuts(path: Path) -> dict[str, object]:
    data = path.read_bytes()
    if not data or data[0] != 0x00:
        raise ValueError("not a binary VDF file")
    key, offset = c_string(data, 1)
    if key != "shortcuts":
        raise ValueError("not a shortcuts.vdf file")
    shortcuts, _ = parse_vdf_map(data, offset)
    return shortcuts


def find_shortcut(shortcuts: dict[str, object], wrapper: Path) -> tuple[str, dict] | None:
    for key, value in shortcuts.items():
        if not isinstance(value, dict):
            continue
        name = str(value.get("appname", value.get("AppName", "")))
        exe = str(value.get("Exe", value.get("exe", "")))
        options = str(value.get("LaunchOptions", ""))
        contains_wrapper = str(wrapper) in exe or str(wrapper) in options
        if contains_wrapper:
            return key, value
        if name.casefold() == APP_NAME.casefold() and (
            "runefin" in (exe + options).lower() or APP_ID in (exe + options)
        ):
            return key, value
    return None


def wrapper_text() -> str:
    log = Path.home() / ".var/app" / APP_ID / "launch.log"
    flatpak = shutil.which("flatpak") or "/usr/bin/flatpak"
    return f"""#!/bin/sh
# Runefin's Steam launcher. Steam can leak its runtime's LD_LIBRARY_PATH/LD_PRELOAD into
# `flatpak run`; a wrapper that clears them is the durable launch path.
unset LD_LIBRARY_PATH LD_PRELOAD
mkdir -p "$(dirname '{log}')"
{{
  printf '%s Runefin launch\\n' "$(date -Is)"
  exec {flatpak} run --branch=stable --arch=x86_64 {APP_ID} --big-screen "$@"
}} >>'{log}' 2>&1
"""


def launcher_text() -> str:
    return f"""[Desktop Entry]
Name={APP_NAME}
Comment=Jellyfin on the big screen
Exec="{WRAPPER}"
Icon={APP_ID}
Terminal=false
Type=Application
Categories=AudioVideo;Video;Player;TV;
Keywords=jellyfin;media;player;tv;big screen;
StartupNotify=true
StartupWMClass=runefin
"""


def write_files(dry: bool) -> None:
    if dry:
        say(f"would write {WRAPPER}")
        say(f"would write {LAUNCHER}")
        return
    WRAPPER.parent.mkdir(parents=True, exist_ok=True)
    WRAPPER.write_text(wrapper_text())
    WRAPPER.chmod(0o755)
    LAUNCHER.parent.mkdir(parents=True, exist_ok=True)
    LAUNCHER.write_text(launcher_text())
    LAUNCHER.chmod(0o644)


def add_to_steam(root: Path, wrapper: Path, dry: bool) -> Path | None:
    account = active_account(root)
    if account is None:
        say("Could not find a Steam userdata account. Start Steam once, then rerun.")
        return None
    shortcuts_path = account / "config/shortcuts.vdf"
    if shortcuts_path.is_file():
        try:
            existing = find_shortcut(read_shortcuts(shortcuts_path), wrapper)
        except (OSError, ValueError) as error:
            say(f"warning: could not read existing shortcuts.vdf: {error}")
            existing = None
        if existing:
            say(f"Runefin is already in Steam (appid {existing[1].get('appid')}).")
            return shortcuts_path
    if not steam_running(root):
        say("Steam is not running. Start Steam (Desktop Mode is fine), then rerun this script.")
        return None

    sentinel = Path(tempfile.gettempdir()) / SENTINEL_NAME
    if dry:
        say(f"would touch {sentinel}")
        say(f"would run: {' '.join(steam_command(root))} steam://addnonsteamgame/{quote(str(LAUNCHER), safe='')}")
        return shortcuts_path
    sentinel.write_bytes(b"")
    os.chmod(sentinel, 0o600)
    command = steam_command(root) + [
        f"steam://addnonsteamgame/{quote(str(LAUNCHER), safe='')}"
    ]
    say(f"run: {' '.join(command)}")
    try:
        subprocess.run(command, timeout=20)
    except subprocess.TimeoutExpired:
        say("Steam did not return; checking the library for the entry anyway.")

    # Steam writes the shortcut when it handles the URL. Give it a moment, then read the id.
    deadline = time.time() + 20
    while time.time() < deadline:
        if shortcuts_path.is_file():
            try:
                found = find_shortcut(read_shortcuts(shortcuts_path), wrapper)
            except (OSError, ValueError):
                found = None
            if found:
                say(f"Steam added Runefin (appid {found[1].get('appid')}).")
                return shortcuts_path
        time.sleep(0.5)
    say("Steam has not written the shortcut yet. Restart Steam and rerun this script.")
    return shortcuts_path


def art_sources(explicit: Path | None, dry: bool) -> Path | None:
    if explicit:
        return explicit if explicit.is_dir() else None
    local = Path(__file__).resolve().parent.parent / "packaging/steam/art"
    if local.is_dir():
        return local
    target = Path(tempfile.mkdtemp(prefix="runefin-art-"))
    for name in ("portrait.png", "header.jpg", "hero.jpg", "logo.png", "icon.png"):
        url = f"{ART_BASE}/{name}"
        if dry:
            say(f"would download {url}")
            continue
        try:
            urllib.request.urlretrieve(url, target / name)
        except OSError as error:
            say(f"warning: could not download {name}: {error}")
    return target


def install_art(shortcuts_path: Path, wrapper: Path, art: Path | None, dry: bool) -> list[Path]:
    if art is None:
        return []
    try:
        found = find_shortcut(read_shortcuts(shortcuts_path), wrapper)
    except (OSError, ValueError):
        found = None
    if not found:
        say("Could not find Runefin in shortcuts.vdf, so artwork was not installed.")
        return []
    appid = str(found[1].get("appid", ""))
    if not appid:
        say("Runefin's shortcut has no appid; artwork was not installed.")
        return []
    grid = shortcuts_path.parent / "grid"
    mapping = {
        "portrait.png": f"{appid}p.png",
        "header.jpg": f"{appid}.jpg",
        "hero.jpg": f"{appid}_hero.jpg",
        "logo.png": f"{appid}_logo.png",
        "icon.png": f"{appid}_icon.png",
    }
    written = []
    if dry:
        for target in mapping.values():
            say(f"would copy art to {grid / target}")
        return []
    grid.mkdir(parents=True, exist_ok=True)
    for source, target in mapping.items():
        src = art / source
        if not src.is_file():
            continue
        destination = grid / target
        shutil.copyfile(src, destination)
        written.append(destination)
    say(f"installed {len(written)} artwork files for appid {appid}.")
    return written


def install_decky_plugin(dry: bool) -> Path | None:
    """Copy the optional QAM plugin into an existing Decky install.

    Decky is not a dependency and is never installed here; an absent `~/homebrew/plugins` is
    simply skipped. The plugin is a separate, unsupported extra: no root flag, no route patches.
    """
    plugins = DECKY_PLUGIN.parent
    if not plugins.is_dir():
        say("Decky is not installed; skipping the optional QAM plugin.")
        return None
    local = Path(__file__).resolve().parent.parent / "packaging" / "decky" / "runefin"
    if dry:
        say(f"would install the Decky plugin in {DECKY_PLUGIN}")
        return DECKY_PLUGIN
    try:
        for name in DECKY_FILES:
            destination = DECKY_PLUGIN / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            if local.is_dir():
                shutil.copyfile(local / name, destination)
            else:
                urllib.request.urlretrieve(f"{DECKY_BASE}/{name}", destination)
    except OSError as error:
        shutil.rmtree(DECKY_PLUGIN, ignore_errors=True)
        say(f"warning: could not install the Decky plugin: {error}")
        return None
    say(f"installed the optional Decky QAM plugin in {DECKY_PLUGIN}; restart Decky to load it")
    return DECKY_PLUGIN


def write_record(appid: str | None, artworks: list[Path], decky: Path | None, dry: bool) -> None:
    if dry:
        say(f"would write {RECORD}")
        return
    RECORD.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "appid": appid,
        "wrapper": str(WRAPPER),
        "launcher": str(LAUNCHER),
        "artwork": [str(path) for path in artworks],
    }
    if decky is not None:
        payload["decky_plugin"] = str(decky)
    RECORD.write_text(json.dumps(payload, indent=2) + "\n")


def uninstall(dry: bool) -> int:
    removed = []
    record = {}
    if RECORD.is_file():
        try:
            record = json.loads(RECORD.read_text())
        except (OSError, ValueError):
            record = {}
    for path in record.get("artwork", []):
        path = Path(path)
        if path.is_file():
            if dry:
                say(f"would remove {path}")
            else:
                path.unlink()
                removed.append(path)
    for path in (WRAPPER, LAUNCHER):
        if path.is_file():
            if dry:
                say(f"would remove {path}")
            else:
                path.unlink()
                removed.append(path)
    decky = record.get("decky_plugin")
    if decky:
        decky_path = Path(decky)
        plugins = (Path.home() / "homebrew" / "plugins").resolve()
        if decky_path.is_dir() and plugins in decky_path.resolve().parents:
            if dry:
                say(f"would remove {decky_path}")
            else:
                shutil.rmtree(decky_path)
                removed.append(decky_path)
    if RECORD.is_file():
        if dry:
            say(f"would remove {RECORD}")
        else:
            RECORD.unlink()
    say("Remove the Runefin entry itself in Steam: Library → right-click Runefin → Manage → Remove non-Steam game.")
    say(f"removed {len(removed)} installed file(s).")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="print changes without making them")
    parser.add_argument("--uninstall", action="store_true", help="remove wrapper, launcher and art")
    parser.add_argument("--no-install", action="store_true", help="do not install the Flatpak")
    parser.add_argument("--no-steam", action="store_true", help="write the wrapper and launcher only")
    parser.add_argument("--decky", action="store_true", help="also install the optional Decky QAM plugin when Decky is present")
    parser.add_argument("--art-dir", type=Path, help="use this artwork directory")
    parser.add_argument("--ref", default=DEFAULT_REF, help="Flatpak .flatpakref to install")
    args = parser.parse_args()

    if os.geteuid() == 0:
        say("Do not run this as root; Runefin is a per-user Flatpak and Steam is per-user.")
        return 1
    if args.uninstall:
        return uninstall(args.dry_run)

    if not install_flatpak(args.ref, args.dry_run, args.no_install):
        return 1
    write_files(args.dry_run)
    decky = install_decky_plugin(args.dry_run) if args.decky else None

    if args.no_steam:
        if decky is not None:
            write_record(None, [], decky, args.dry_run)
        say("Wrapper and launcher written. Add it in Steam yourself, or rerun without --no-steam.")
        return 0

    found = find_steam()
    if found is None:
        say("Steam was not found. Run with --no-steam, or install Steam and rerun.")
        return 1
    root, _ = found
    say(f"Steam: {root}")
    shortcuts = add_to_steam(root, WRAPPER, args.dry_run)
    if shortcuts is None:
        return 1
    art = art_sources(args.art_dir, args.dry_run)
    artworks = install_art(shortcuts, WRAPPER, art, args.dry_run)
    try:
        found_shortcut = find_shortcut(read_shortcuts(shortcuts), WRAPPER)
        appid = str(found_shortcut[1].get("appid")) if found_shortcut else None
    except (OSError, ValueError):
        appid = None
    write_record(appid, artworks, decky, args.dry_run)
    say("Done. Restart Steam if the artwork or name does not appear immediately.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
