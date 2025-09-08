"""Bootstrap main application (tray + window + server)."""

from __future__ import annotations

import os
import sys
import signal
from pathlib import Path
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QTimer

from .ui.main_window import MainWindow
from .ui.tray import build_tray
from .server import ServerThread
from .config import config


def launch():  # pragma: no cover
    lock_path = Path.home() / ".asrpro.lock"
    if not lock_path.exists():
        try:
            lock_path.write_text(str(os.getpid()))
        except Exception:
            pass

    app = QApplication(sys.argv)
    app.setOrganizationName("asrpro")
    app.setApplicationName("asrpro")
    app.setQuitOnLastWindowClosed(False)

    # Allow Ctrl+C (SIGINT) to terminate the Qt event loop on all platforms.
    signal.signal(signal.SIGINT, lambda *_: app.quit())
    # Heartbeat timer so Python processes pending signals while in Qt loop.
    _heartbeat = QTimer()
    _heartbeat.start(250)
    _heartbeat.timeout.connect(lambda: None)

    window = MainWindow()
    tray = build_tray(window)
    tray.show()

    # Start server if enabled in config
    server_thread = None
    if config.is_server_enabled():
        server_thread = ServerThread(
            window.model_manager, port=config.get_server_port()
        )
        server_thread.start()

    window.hide()
    exit_code = 0
    try:
        exit_code = app.exec()
    except KeyboardInterrupt:
        exit_code = 0

    # Cleanup
    try:
        if server_thread:
            server_thread.shutdown()
    except Exception:
        pass
    try:
        lock_path.unlink(missing_ok=True)
    except Exception:
        pass
    sys.exit(exit_code)


__all__ = ["launch"]
