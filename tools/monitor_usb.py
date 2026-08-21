#!/usr/bin/env python3
"""Print the RMK USB CDC-ACM debug log without external Python packages."""

from __future__ import annotations

import argparse
import glob
import os
import select
import sys
import time


PORT_PATTERNS = (
    "/dev/ttyACM*",
    "/dev/ttyUSB*",
    "/dev/cu.usbmodem*",
    "/dev/cu.usbserial*",
)


def find_ports() -> list[str]:
    ports: list[str] = []
    for pattern in PORT_PATTERNS:
        ports.extend(glob.glob(pattern))
    return sorted(set(ports))


def configure_port(fd: int, baudrate: int) -> None:
    """Configure a conventional serial port when termios is available."""

    try:
        import termios

        speed = getattr(termios, f"B{baudrate}", termios.B115200)
        attrs = termios.tcgetattr(fd)
        attrs[0] = 0
        attrs[1] = 0
        attrs[2] = termios.CS8 | termios.CREAD | termios.CLOCAL
        attrs[3] = 0
        attrs[4] = speed
        attrs[5] = speed
        attrs[6][termios.VMIN] = 0
        attrs[6][termios.VTIME] = 1
        termios.tcsetattr(fd, termios.TCSANOW, attrs)
    except (ImportError, OSError):
        # USB CDC ignores the baud rate on most boards; reading still works.
        pass


def monitor(port: str, baudrate: int) -> None:
    flags = os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK
    # udev's stable by-id links can be readable while opening the link path
    # itself is denied by a restrictive /dev/serial directory ACL. Resolve the
    # link first; this also keeps the user-facing stable identifier unchanged.
    device = os.path.realpath(port)
    fd = os.open(device, flags)
    try:
        configure_port(fd, baudrate)
        if device == port:
            connection = port
        else:
            connection = f"{port} -> {device}"
        print(f"connected to {connection}; waiting for RMK USB logs (Ctrl-C to stop)", file=sys.stderr)
        while True:
            readable, _, _ = select.select([fd], [], [], 1.0)
            if not readable:
                continue
            try:
                data = os.read(fd, 4096)
            except BlockingIOError:
                continue
            if not data:
                return
            sys.stdout.buffer.write(data)
            sys.stdout.buffer.flush()
    finally:
        os.close(fd)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--port",
        default=os.environ.get("USB_DEBUG_PORT"),
        help="CDC port; defaults to USB_DEBUG_PORT or automatic /dev/ttyACM* detection",
    )
    parser.add_argument("--baudrate", type=int, default=115200)
    args = parser.parse_args()

    selected = args.port
    print("waiting for RMK USB CDC port...", file=sys.stderr)
    try:
        while True:
            if selected is None:
                ports = find_ports()
                selected = ports[0] if ports else None
            if selected is not None:
                try:
                    monitor(selected, args.baudrate)
                except OSError as error:
                    print(f"{selected}: {error}; waiting for reconnect", file=sys.stderr)
                    if args.port:
                        selected = args.port
                    else:
                        selected = None
                    time.sleep(0.5)
            else:
                time.sleep(0.5)
    except KeyboardInterrupt:
        print("stopped", file=sys.stderr)
        return 0


if __name__ == "__main__":
    sys.exit(main())
