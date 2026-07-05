#!/usr/bin/env python3
"""Drive the TUI viewer in a virtual PTY and print a plain-text snapshot of
the rendered terminal screen. Useful for automated/non-interactive testing
of the `to_markdown_mcp tui <path>` viewer.

Usage:
    python3 capture_tui.py <file-or-vault-path> [key_presses] [rows] [cols]

Requires the `pyte` terminal-emulator library:
    python3 -m venv /tmp/tui-test-venv
    /tmp/tui-test-venv/bin/pip install --quiet pyte
    /tmp/tui-test-venv/bin/python3 capture_tui.py output.md

Gotcha: keys are sent ONE AT A TIME with a short delay between them, not as
a rapid burst. A burst sent immediately after spawning the process can land
in the PTY's line-buffered ("cooked") mode before the app's
enable_raw_mode() call takes effect, so keystrokes appear to get silently
dropped. This is a test-harness race, not an app bug — the fix is a warm-up
delay after spawn and pacing between keys.
"""
import fcntl
import os
import pty
import select
import struct
import subprocess
import sys
import termios
import time
from pathlib import Path

try:
    import pyte
except ImportError:
    print("Missing dependency: pip install pyte (ideally in a venv)", file=sys.stderr)
    sys.exit(1)

REPO_ROOT = Path(__file__).resolve().parent.parent
BINARY = REPO_ROOT / "target" / "debug" / "to_markdown_mcp"


def run(target_path: str, key_presses: str = "jjjjjjjjjj", rows: int = 55, cols: int = 150) -> str:
    if not BINARY.exists():
        raise FileNotFoundError(f"Binary not found at {BINARY} — run `cargo build` first.")

    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))

    env = os.environ.copy()
    env["TERM"] = "xterm-256color"
    proc = subprocess.Popen(
        [str(BINARY), "tui", target_path],
        stdin=slave,
        stdout=slave,
        stderr=slave,
        env=env,
        close_fds=True,
    )
    os.close(slave)

    screen = pyte.Screen(cols, rows)
    stream = pyte.Stream(screen)

    def read_available(timeout: float = 0.5) -> None:
        end = time.time() + timeout
        while time.time() < end:
            ready, _, _ = select.select([master], [], [], 0.2)
            if master in ready:
                try:
                    data = os.read(master, 65536)
                except OSError:
                    break
                stream.feed(data.decode(errors="ignore"))
            else:
                time.sleep(0.05)

    try:
        # Warm-up: let the app draw its first frame and enable raw mode
        # before we start sending keys.
        read_available(1.5)

        # Pace keys one at a time — see module docstring gotcha.
        for key in key_presses:
            os.write(master, key.encode())
            read_available(0.15)

        snapshot = "\n".join(screen.display)

        os.write(master, b"q")
        read_available(1.0)
    finally:
        try:
            proc.wait(timeout=5)
        except Exception:
            proc.kill()

    return snapshot


def main() -> None:
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    target_path = sys.argv[1]
    key_presses = sys.argv[2] if len(sys.argv) > 2 else "jjjjjjjjjj"
    rows = int(sys.argv[3]) if len(sys.argv) > 3 else 55
    cols = int(sys.argv[4]) if len(sys.argv) > 4 else 150

    snapshot = run(target_path, key_presses, rows, cols)
    print(snapshot)


if __name__ == "__main__":
    main()
