"""Launch the native GPUI client against the shared local Web API."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

from ok.ui.gpui.server import WebServerHandle, start_web_server


def _project_root() -> Path:
    return Path(__file__).resolve().parents[3]


def _resolve_binary(config: dict) -> Path:
    gui_config = config.get("gui")
    configured = gui_config.get("binary") if isinstance(gui_config, dict) else None
    candidates = []
    if configured:
        candidates.append(Path(str(configured)))
    if value := os.environ.get("OK_SCRIPT_GPUI_BINARY"):
        candidates.append(Path(value))

    root = _project_root()
    crate_root = root / "ok" / "ui" / "gpui"
    candidates.extend((
        Path(sys.executable).resolve().parent / "ok-script-gpui.exe",
        crate_root / "target" / "debug" / "ok-script-gpui.exe",
        crate_root / "target" / "release" / "ok-script-gpui.exe",
        crate_root / "bin" / "ok-script-gpui.exe",
        root / "ok-script-gpui.exe",
    ))

    searched = []
    for candidate in candidates:
        candidate_paths = (candidate,) if candidate.is_absolute() else (
            Path.cwd() / candidate,
            root / candidate,
        )
        for resolved in candidate_paths:
            searched.append(resolved)
            if resolved.is_file():
                return resolved.resolve()
    searched_text = ", ".join(str(path) for path in searched)
    raise FileNotFoundError(
        "GPUI client executable was not found. Set gui.binary or "
        f"OK_SCRIPT_GPUI_BINARY. Searched: {searched_text}"
    )


def _window_args(config: dict) -> list[str]:
    gui_config = config.get("gui") if isinstance(config.get("gui"), dict) else {}
    window = gui_config.get("window_size") or config.get("window_size") or {}
    args = []
    for key, flag in (
        ("width", "--width"),
        ("height", "--height"),
        ("min_width", "--min-width"),
        ("min_height", "--min-height"),
    ):
        value = window.get(key)
        if value is not None:
            args.extend((flag, str(int(value))))
    return args


def _terminate_process(process: subprocess.Popen) -> None:
    if process.poll() is not None:
        return
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def run_gpui(config: dict, host="127.0.0.1", port=0, debug=None,
             ok_instance=None) -> int:
    """Run the GPUI client with the existing Python runtime as its backend."""
    handle: WebServerHandle | None = None
    process: subprocess.Popen | None = None
    try:
        handle = start_web_server(config, host=host, port=port, debug=debug,
                                  ok_instance=ok_instance)
        binary = _resolve_binary(config)
        command = [str(binary), "--url", handle.url, *_window_args(config)]
        if debug is True or bool(config.get("debug")):
            command.append("--debug")
        creationflags = getattr(subprocess, "CREATE_NO_WINDOW", 0)
        process = subprocess.Popen(
            command,
            cwd=str(_project_root()),
            creationflags=creationflags,
        )

        while process.poll() is None:
            if ok_instance is not None:
                if ok_instance.exit_event.wait(0.25):
                    _terminate_process(process)
                    break
            else:
                try:
                    process.wait(timeout=0.25)
                except subprocess.TimeoutExpired:
                    continue
        return int(process.returncode or 0)
    finally:
        if process is not None:
            _terminate_process(process)
        if handle is not None:
            handle.stop()
