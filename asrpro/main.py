"""Bootstrap main application (tray + window + server)."""

from __future__ import annotations

import os
import sys
import signal
import psutil
from pathlib import Path
from PySide6.QtWidgets import QApplication, QMessageBox
from PySide6.QtCore import QTimer

# Use the new WebEngine-based UI mirroring temp/index.html
from .ui.web_main_window import MainWindow
from .ui.tray import build_tray
from .server import ServerThread
from .config import config


def kill_existing_instance():
    """Kill any existing asrpro instance."""
    current_pid = os.getpid()
    killed_any = False

    for proc in psutil.process_iter(["pid", "name", "cmdline"]):
        try:
            if proc.info["pid"] == current_pid:
                continue

            # Check if it's a Python process running asrpro
            if (
                proc.info["name"]
                and "python" in proc.info["name"].lower()
                and proc.info["cmdline"]
                and any("asrpro" in str(arg) for arg in proc.info["cmdline"])
            ):
                print(f"Killing existing asrpro instance (PID: {proc.info['pid']})")
                proc.kill()
                killed_any = True
        except (psutil.NoSuchProcess, psutil.AccessDenied, psutil.ZombieProcess):
            pass

    return killed_any


def ensure_single_instance():
    """Ensure only one instance of asrpro is running."""
    lock_path = Path.home() / ".asrpro.lock"

    # Check if lock file exists and process is still running
    if lock_path.exists():
        try:
            old_pid = int(lock_path.read_text().strip())
            if psutil.pid_exists(old_pid):
                try:
                    old_proc = psutil.Process(old_pid)
                    if any("asrpro" in str(arg) for arg in old_proc.cmdline()):
                        print(f"Terminating existing asrpro instance (PID: {old_pid})")
                        old_proc.terminate()
                        old_proc.wait(timeout=5)
                except (psutil.NoSuchProcess, psutil.TimeoutExpired):
                    pass
        except (ValueError, OSError):
            pass

    # Kill any remaining instances
    kill_existing_instance()

    # Write our PID to lock file
    try:
        lock_path.write_text(str(os.getpid()))
    except Exception as e:
        print(f"Warning: Could not create lock file: {e}")

    return lock_path


def launch():  # pragma: no cover
    # Ensure single instance
    lock_path = ensure_single_instance()

    app = QApplication(sys.argv)
    app.setOrganizationName("asrpro")
    app.setApplicationName("asrpro")
    app.setQuitOnLastWindowClosed(False)

    # Initialize WebEngine (if available) early to avoid blank screens
    try:
        import PySide6.QtWebEngineCore  # noqa: F401
    except Exception:
        pass

    # Allow Ctrl+C (SIGINT) to terminate the Qt event loop on all platforms.
    signal.signal(signal.SIGINT, lambda *_: app.quit())
    # Heartbeat timer so Python processes pending signals while in Qt loop.
    _heartbeat = QTimer()
    _heartbeat.start(250)
    _heartbeat.timeout.connect(lambda: None)

    window = MainWindow()
    tray = build_tray(window)
    window.set_tray_icon(tray)  # Store tray reference for theme updates
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
