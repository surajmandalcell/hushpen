#!/usr/bin/env python3
"""Simple launcher with lightweight hot-reload.

Behavior:
- Spawns the GUI app as a child process ("python -m asrpro").
- Press 'r' in this terminal to restart the child (quick dev reload).
- Press 'q' (or Ctrl+C) to quit the launcher.

Notes:
- Keeps stdout/stderr of the child attached so logs stream through.
- Uses cross-platform input handling (msvcrt on Windows; select on POSIX).
"""

from __future__ import annotations

import os
import sys
import time
import subprocess
import threading


def _input_loop(on_key):
    """Background key listener that calls on_key(char)."""
    try:
        if os.name == "nt":
            # Windows: use msvcrt for single key reads
            import msvcrt

            while True:
                if msvcrt.kbhit():
                    ch = msvcrt.getwch()
                    if ch:
                        on_key(ch.lower())
                time.sleep(0.05)
        else:
            # Mac/Unix: Use cbreak mode instead of raw mode to preserve log formatting
            import tty, termios, select

            fd = sys.stdin.fileno()
            old_settings = termios.tcgetattr(fd)
            try:
                # Use cbreak instead of raw - this preserves output formatting
                # while still allowing single-char input
                tty.setcbreak(fd)

                while True:
                    # Use select to check for input without blocking
                    if select.select([sys.stdin], [], [], 0.1)[0]:
                        ch = sys.stdin.read(1)
                        if ch:
                            if ch == '\x03':  # Ctrl+C
                                on_key('\u0003')
                                break
                            elif ch in ('\x1b', '\r', '\n'):  # Escape, Enter, newline - ignore
                                continue
                            else:
                                on_key(ch.lower())
            finally:
                termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)
    except Exception as e:
        # Final fallback: blocking line input
        print(f"[launcher] Using line-based input (press 'r' then Enter). Reason: {e}")
        try:
            for line in sys.stdin:
                if line:
                    on_key(line.strip().lower()[:1])
        except KeyboardInterrupt:
            on_key('\u0003')


def spawn_child():
    """Spawn the GUI app as a module so imports/env match the current interpreter."""
    # Redirect child's stdin to devnull so it doesn't compete for terminal input
    return subprocess.Popen(
        [sys.executable, "-m", "asrpro"],
        stdin=subprocess.DEVNULL,
        # Keep stdout/stderr inherited so we see logs
    )


def main():
    print("asrpro dev launcher: 'r' = restart, 'q' = quit")

    child = spawn_child()
    should_exit = False
    restart_requested = False

    def on_key(ch: str):
        nonlocal restart_requested, should_exit
        if ch == "r":
            print("\n[launcher] Restart requested.")
            restart_requested = True
            # Terminate child; main loop will respawn
            try:
                child.terminate()
            except Exception:
                pass
        elif ch in ("q", "\u0003"):  # q or Ctrl+C
            print("\n[launcher] Quit requested.")
            should_exit = True
            try:
                child.terminate()
            except Exception:
                pass

    # Start background input thread
    t = threading.Thread(target=_input_loop, args=(on_key,), daemon=True)
    t.start()

    try:
        while True:
            ret = child.wait()
            if should_exit:
                break
            if restart_requested:
                restart_requested = False
                print("[launcher] Restarting child...")
                child = spawn_child()
                continue
            # Child exited without restart request; propagate code
            sys.exit(ret)
    except KeyboardInterrupt:
        pass
    finally:
        try:
            child.terminate()
        except Exception:
            pass


if __name__ == "__main__":
    # Keep previous behavior if someone imports run.py and calls main
    main()
