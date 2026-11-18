"""Runefin's Decky companion: launch and MPRIS control from the Quick Access Menu.

No root flag, no Steam route patches, no filesystem writes. It reads the host installer's wrapper
path, launches the existing Steam tile through Steam's own APIs, and drives playback through the
MPRIS service Runefin already exposes.
"""

from __future__ import annotations

import asyncio
import os
import re
import shutil
from pathlib import Path

import decky  # provided by Decky Loader

MPRIS_DEST = "org.mpris.MediaPlayer2.runefin"
MPRIS_PATH = "/org/mpris/MediaPlayer2"
PLAYER = "org.mpris.MediaPlayer2.Player"
ROOT = "org.mpris.MediaPlayer2"
GDBUS = shutil.which("gdbus") or "/usr/bin/gdbus"


async def gdbus(*args: str, timeout: float = 2.0) -> tuple[int, str]:
    """One gdbus call, without blocking the plugin's event loop.

    Decky's loader is a system service, so the plugin process may inherit root's environment
    rather than the desktop session's. Point GDBus at the user's session bus explicitly.
    """
    try:
        env = os.environ.copy()
        uid = os.getuid()
        if uid != 0:
            runtime = env.get("XDG_RUNTIME_DIR") or f"/run/user/{uid}"
            env["XDG_RUNTIME_DIR"] = runtime
            env["DBUS_SESSION_BUS_ADDRESS"] = f"unix:path={runtime}/bus"
        process = await asyncio.create_subprocess_exec(
            GDBUS,
            *args,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
            env=env,
        )
        stdout, _ = await asyncio.wait_for(process.communicate(), timeout)
        return process.returncode or 0, stdout.decode("utf-8", "replace")
    except (FileNotFoundError, asyncio.TimeoutError, OSError):
        return 1, ""


async def call_player(method: str) -> bool:
    code, _ = await gdbus(
        "call",
        "--session",
        "--dest",
        MPRIS_DEST,
        "--object-path",
        MPRIS_PATH,
        "--method",
        f"{PLAYER}.{method}",
    )
    return code == 0


class Plugin:
    async def _main(self) -> None:
        decky.logger.info("Runefin companion ready")

    async def _unload(self) -> None:
        pass

    async def installation(self) -> dict:
        """What the host installer wrote, so the panel can explain a missing tile."""
        wrapper = Path.home() / ".local" / "share" / "runefin" / "runefin-steam.sh"
        return {
            "installed": wrapper.is_file(),
            "wrapper": str(wrapper),
            "flatpak": shutil.which("flatpak") is not None,
        }

    async def status(self) -> dict:
        """Whether Runefin is on the bus, and its MPRIS PlaybackStatus."""
        code, out = await gdbus(
            "call",
            "--session",
            "--dest",
            MPRIS_DEST,
            "--object-path",
            MPRIS_PATH,
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            PLAYER,
            "PlaybackStatus",
        )
        if code != 0:
            return {"running": False, "playback": ""}
        match = re.search(r"<'([^']*)'>", out)
        return {"running": True, "playback": match.group(1) if match else "Playing"}

    async def command(self, action: str) -> bool:
        """Transport, raise or quit. Only these names reach the bus."""
        if action == "quit":
            code, _ = await gdbus(
                "call",
                "--session",
                "--dest",
                MPRIS_DEST,
                "--object-path",
                MPRIS_PATH,
                "--method",
                f"{ROOT}.Quit",
            )
            return code == 0
        if action == "raise":
            code, _ = await gdbus(
                "call",
                "--session",
                "--dest",
                MPRIS_DEST,
                "--object-path",
                MPRIS_PATH,
                "--method",
                f"{ROOT}.Raise",
            )
            return code == 0
        methods = {
            "playpause": "PlayPause",
            "play": "Play",
            "pause": "Pause",
            "stop": "Stop",
            "next": "Next",
            "previous": "Previous",
        }
        method = methods.get(action)
        return bool(method) and await call_player(method)
