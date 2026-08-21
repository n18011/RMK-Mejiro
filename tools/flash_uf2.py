#!/usr/bin/env python3
"""Copy a UF2 image to an nRF52 UF2 bootloader mount.

The board still needs to be placed in bootloader mode (usually by a double
reset). Once it is mounted, this script finds it and performs the copy so a
local build does not require manual file-manager work.
"""

from __future__ import annotations

import argparse
import errno
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import time


MOUNT_MARKERS = ("INFO_UF2.TXT", "INDEX.HTM", "CURRENT.UF2")
UF2_LABEL_MARKERS = ("UF2", "XIAO", "NRF")


def is_uf2_mount(path: Path) -> bool:
    if not path.is_dir():
        return False
    return any((path / marker).exists() for marker in MOUNT_MARKERS)


def candidate_mounts() -> list[Path]:
    candidates: list[Path] = []
    configured = os.environ.get("UF2_MOUNT")
    if configured:
        candidates.append(Path(configured).expanduser())

    username = os.environ.get("USER") or os.environ.get("LOGNAME")
    if username:
        candidates.extend((Path("/media") / username, Path("/run/media") / username))
    candidates.append(Path("/mnt"))

    mounts: list[Path] = []
    seen: set[Path] = set()
    for root in candidates:
        if not root.exists():
            continue
        entries = [root]
        try:
            entries.extend(root.iterdir())
        except OSError:
            continue
        for entry in entries:
            resolved = entry.resolve()
            if resolved in seen:
                continue
            seen.add(resolved)
            if is_uf2_mount(resolved):
                mounts.append(resolved)
    return mounts


def parse_lsblk_pairs(output: str) -> list[dict[str, str]]:
    """Parse lsblk's shell-compatible --pairs output."""

    rows: list[dict[str, str]] = []
    for line in output.splitlines():
        try:
            fields = dict(
                item.split("=", 1) for item in shlex.split(line) if "=" in item
            )
        except ValueError:
            continue
        if fields:
            rows.append(fields)
    return rows


def unmounted_uf2_devices() -> list[str]:
    """Find likely UF2 volumes that udisks knows about but has not mounted."""

    if shutil.which("lsblk") is None:
        return []
    try:
        result = subprocess.run(
            [
                "lsblk",
                "--paths",
                "--pairs",
                "--output",
                "PATH,FSTYPE,LABEL,MOUNTPOINTS",
            ],
            capture_output=True,
            text=True,
            timeout=2,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return []
    if result.returncode != 0:
        return []

    devices: list[str] = []
    for row in parse_lsblk_pairs(result.stdout):
        filesystem = row.get("FSTYPE", "").lower()
        label = row.get("LABEL", "").upper()
        mountpoint = row.get("MOUNTPOINTS", "")
        if (
            row.get("PATH")
            and filesystem in {"vfat", "fat", "msdos"}
            and not mountpoint
            and any(marker in label for marker in UF2_LABEL_MARKERS)
        ):
            devices.append(row["PATH"])
    return devices


def auto_mount_uf2_devices() -> None:
    """Ask udisks to mount detected UF2 volumes without requiring sudo."""

    if shutil.which("udisksctl") is None:
        return
    for device in unmounted_uf2_devices():
        try:
            subprocess.run(
                ["udisksctl", "mount", "--block-device", device],
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired):
            continue


def wait_for_mount(timeout: float, interval: float) -> Path | None:
    deadline = time.monotonic() + timeout
    next_mount_attempt = 0.0
    while time.monotonic() <= deadline:
        mounts = candidate_mounts()
        if mounts:
            return mounts[0]
        if time.monotonic() >= next_mount_attempt:
            auto_mount_uf2_devices()
            next_mount_attempt = time.monotonic() + 1.0
        time.sleep(interval)
    return None


def copy_uf2(source: Path, mount: Path) -> Path:
    destination = mount / source.name
    expected_size = source.stat().st_size
    bytes_written = 0

    try:
        with source.open("rb") as input_file, destination.open("wb", buffering=0) as output_file:
            while chunk := input_file.read(1024 * 1024):
                pending = memoryview(chunk)
                while pending:
                    count = output_file.write(pending)
                    if not count:
                        raise OSError(errno.EIO, "UF2 bootloader accepted no data")
                    bytes_written += count
                    pending = pending[count:]
            output_file.flush()
            os.fsync(output_file.fileno())
    except OSError as error:
        # UF2 bootloaders commonly reboot and remove the MSC volume as soon as
        # the complete image has been accepted. In that case fsync()/close()
        # can report EIO even though the firmware is already being flashed.
        if error.errno in {errno.EIO, errno.ENODEV, errno.ENOENT} and bytes_written == expected_size:
            return destination
        raise
    return destination


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("uf2", type=Path, help="UF2 image to copy")
    parser.add_argument(
        "--mount",
        type=Path,
        help="explicit UF2 mount point (also accepted through UF2_MOUNT)",
    )
    parser.add_argument("--timeout", type=float, default=60.0)
    parser.add_argument("--interval", type=float, default=0.25)
    args = parser.parse_args()

    source = args.uf2.resolve()
    if not source.is_file():
        print(f"UF2 image not found: {source}", file=sys.stderr)
        return 2

    mount = args.mount.expanduser().resolve() if args.mount else None
    if mount is not None:
        if not mount.is_dir():
            print(f"UF2 mount is not a directory: {mount}", file=sys.stderr)
            return 2
    else:
        print(
            "Waiting for UF2 bootloader mount; double-reset the board if it is not mounted...",
            flush=True,
        )
        mount = wait_for_mount(args.timeout, args.interval)
        if mount is None:
            print(
                "No UF2 mount found. Put the board in bootloader mode or set UF2_MOUNT.",
                file=sys.stderr,
            )
            return 1

    try:
        destination = copy_uf2(source, mount)
    except OSError as error:
        print(
            f"UF2 copy failed: {error}. The bootloader may have disconnected "
            "before the complete image was accepted; retry in bootloader mode.",
            file=sys.stderr,
        )
        return 1
    print(f"Copied {source.name} to {destination}; the board should reboot shortly.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
