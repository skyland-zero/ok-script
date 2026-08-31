"""Own the local Web API process used by the native GPUI shell."""

from __future__ import annotations

import logging
import socket
import threading
import time
from dataclasses import dataclass

from ok.ui.web.app import create_web_app
from ok.ui.web.requirements import check_web_requirements
from ok.util.logger import Logger


logger = Logger.get_logger("web_server")


@dataclass
class WebServerHandle:
    """A running local Web API owned by the GPUI shell."""

    server: object
    server_socket: socket.socket
    thread: threading.Thread
    host: str
    port: int
    url: str
    _stop_lock: threading.Lock
    _stopped: bool = False

    def wait_started(self, timeout: float = 15.0) -> None:
        """Wait until Uvicorn has completed startup or raise its failure."""
        deadline = time.monotonic() + timeout
        while not getattr(self.server, "started", False):
            if not self.thread.is_alive():
                error = getattr(self.server, "_ok_start_error", None)
                if error is not None:
                    raise RuntimeError("Web API failed to start") from error
                raise RuntimeError("Web API stopped before startup completed")
            if time.monotonic() >= deadline:
                raise TimeoutError(f"Web API did not start within {timeout:.1f}s")
            time.sleep(0.01)

    def stop(self, timeout: float = 10.0) -> None:
        """Stop the server and release its listening socket exactly once."""
        with self._stop_lock:
            if self._stopped:
                return
            self._stopped = True
            setattr(self.server, "should_exit", True)
            if self.thread.is_alive():
                self.thread.join(timeout=timeout)
            try:
                self.server_socket.close()
            except OSError:
                pass


class _OkServerLogHandler(logging.Handler):
    """Route ASGI server records through ok-script's logger."""

    _WEBSOCKET_FRAME_PREFIXES = (
        "> TEXT", "< TEXT", "> BINARY", "< BINARY",
        "> PING", "< PING", "> PONG", "< PONG",
        "> CLOSE", "< CLOSE",
    )

    def __init__(self):
        super().__init__(logging.DEBUG)
        self.logger = Logger.get_logger("web_server")

    def emit(self, record):
        message = self.format(record)
        if (record.levelno <= logging.DEBUG
                and message.lstrip().startswith(self._WEBSOCKET_FRAME_PREFIXES)):
            return
        if record.levelno <= logging.INFO:
            self.logger.debug(message)
        elif record.levelno <= logging.WARNING:
            self.logger.warning(message)
        elif record.levelno <= logging.ERROR:
            self.logger.error(message)
        else:
            self.logger.critical(message)


def _configure_server_logging():
    handler = _OkServerLogHandler()
    for name in ("uvicorn", "uvicorn.error", "uvicorn.access", "websockets.server"):
        server_logger = logging.getLogger(name)
        server_logger.handlers = [handler]
        server_logger.setLevel(
            logging.INFO if name == "websockets.server" else logging.DEBUG
        )
        server_logger.propagate = False


def start_web_server(config, host="127.0.0.1", port=0, debug=None,
                     ok_instance=None) -> WebServerHandle:
    """Start the local FastAPI service without opening a browser window."""
    uvicorn = check_web_requirements()

    web_config = dict(config)
    web_config["use_gui"] = False
    if debug is not None:
        web_config["debug"] = debug

    server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server = None
    thread = None
    try:
        server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server_socket.bind((host, port))
        server_socket.listen(2048)
        selected_port = server_socket.getsockname()[1]

        web_app = create_web_app(web_config, ok_instance=ok_instance)
        uvicorn_config = uvicorn.Config(
            web_app,
            host=host,
            port=selected_port,
            log_config=None,
        )
        _configure_server_logging()
        server = uvicorn.Server(uvicorn_config)

        def serve() -> None:
            try:
                server.run(sockets=[server_socket])
            except BaseException as exc:
                setattr(server, "_ok_start_error", exc)
                logger.exception("ok script GPUI web API thread failed")

        thread = threading.Thread(target=serve, name="gpui-web-server", daemon=True)
        handle = WebServerHandle(
            server=server,
            server_socket=server_socket,
            thread=thread,
            host=host,
            port=selected_port,
            url=f"http://{host}:{selected_port}",
            _stop_lock=threading.Lock(),
        )
        thread.start()
        handle.wait_started()
        logger.info(f"ok script GPUI web API started: {handle.url}")
        return handle
    except BaseException:
        if server is not None:
            setattr(server, "should_exit", True)
        if thread is not None and thread.is_alive():
            thread.join(timeout=10.0)
        try:
            server_socket.close()
        except OSError:
            pass
        raise
