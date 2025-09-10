"""Bootstrap main application (tray + window + server)."""

from __future__ import annotations

import os
import sys
import signal
import psutil
from pathlib import Path
from PySide6.QtWidgets import QApplication, QMessageBox
from PySide6.QtGui import QFont
from PySide6.QtCore import QTimer

# Use the native PyQt UI
from .ui.main_window import NativeMainWindow as MainWindow
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
    current_pid = os.getpid()

    # Check if lock file exists and process is still running
    if lock_path.exists():
        try:
            old_pid = int(lock_path.read_text().strip())
            if old_pid != current_pid and psutil.pid_exists(old_pid):
                try:
                    old_proc = psutil.Process(old_pid)
                    cmdline_str = " ".join(old_proc.cmdline())
                    if "asrpro" in cmdline_str or "run.py" in cmdline_str:
                        # Ask user what to do with existing instance
                        print(f"\nFound existing asrpro instance (PID: {old_pid})")
                        print("Do you want to kill the existing instance? (Y/n): ", end="", flush=True)
                        
                        try:
                            choice = input().strip().lower()
                            if choice == "" or choice == "y" or choice == "yes":
                                print(f"Terminating existing asrpro instance (PID: {old_pid})")
                                old_proc.terminate()
                                try:
                                    old_proc.wait(timeout=5)
                                except psutil.TimeoutExpired:
                                    print("Force killing existing instance...")
                                    old_proc.kill()
                            else:
                                print("Keeping existing instance. Exiting...")
                                sys.exit(0)
                        except (EOFError, KeyboardInterrupt):
                            print("\nKeeping existing instance. Exiting...")
                            sys.exit(0)
                except (psutil.NoSuchProcess, psutil.TimeoutExpired):
                    # Process no longer exists, remove stale lock file
                    lock_path.unlink(missing_ok=True)
        except (ValueError, OSError):
            # Invalid lock file, remove it
            lock_path.unlink(missing_ok=True)

    # Kill any remaining instances (cleanup)
    killed = kill_existing_instance()
    if killed:
        import time
        time.sleep(1)  # Give processes time to clean up

    # Write our PID to lock file
    try:
        lock_path.write_text(str(current_pid))
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

    # Apply a consistent system font baseline
    try:
        from .ui.styles.dark_theme import Fonts
        app.setFont(QFont(Fonts.get_system_font(), Fonts.BASE_SIZE))
    except Exception:
        pass

    # Native PyQt UI - no WebEngine needed

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

    # Show settings on startup (open main window to General page)
    try:
        # Ensure the General settings page is visible
        if hasattr(window, "sidebar"):
            window.sidebar.set_active_section("general")
        if hasattr(window, "content_area"):
            window.content_area.show_page("general")
        window.show()
        window.raise_()
        window.activateWindow()
    except Exception as e:
        print(f"[Bootstrap] Failed to show settings on startup: {e}")

    # Start server if enabled in config
    server_thread = None
    if config.is_server_enabled():
        server_thread = ServerThread(
            window.model_manager, port=config.get_server_port()
        )
        server_thread.start()

    # Window is already shown; continue to event loop
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
