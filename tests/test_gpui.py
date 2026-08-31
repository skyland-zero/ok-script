from pathlib import Path
from unittest.mock import Mock, patch

from ok.ui.gpui import launcher


def test_resolve_binary_checks_project_root_for_relative_configuration(
    tmp_path, monkeypatch
):
    project_root = tmp_path / "project"
    project_root.mkdir()
    (project_root / "client.exe").write_bytes(b"placeholder")
    working_dir = tmp_path / "working"
    working_dir.mkdir()
    monkeypatch.chdir(working_dir)
    monkeypatch.setattr(launcher, "_project_root", lambda: project_root)

    assert launcher._resolve_binary({"gui": {"binary": "client.exe"}}) == (
        project_root / "client.exe"
    )


def test_run_gpui_forwards_explicit_debug_and_stops_backend():
    handle = Mock(url="http://127.0.0.1:34567")
    process = Mock()
    process.poll.side_effect = [None, 0, 0]
    process.returncode = 0

    with patch.object(launcher, "start_web_server", return_value=handle), \
            patch.object(launcher, "_resolve_binary", return_value=Path("client.exe")), \
            patch.object(launcher.subprocess, "Popen", return_value=process) as popen:
        assert launcher.run_gpui({"gui": {"type": "gpui"}}, debug=True) == 0

    command = popen.call_args.args[0]
    assert command == ["client.exe", "--url", handle.url, "--debug"]
    handle.stop.assert_called_once_with()
