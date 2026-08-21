#!/usr/bin/env python3
"""Check the local toolchain used by RMK-Mejiro firmware development."""

from __future__ import annotations

import argparse
import glob
import os
import shutil
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
TARGET = "thumbv7em-none-eabihf"


def find_tool(name: str) -> str | None:
    """Find a command, including the usual rustup cargo bin directory."""

    found = shutil.which(name)
    if found:
        return found

    rustup_bin = Path.home() / ".cargo" / "bin" / name
    if rustup_bin.is_file() and os.access(rustup_bin, os.X_OK):
        return str(rustup_bin)
    return None


def run(command: list[str], timeout: float = 10.0) -> tuple[int, str]:
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        return 1, str(error)

    output = (completed.stdout + completed.stderr).strip()
    return completed.returncode, output


def report(kind: str, label: str, detail: str) -> None:
    print(f"[{kind}] {label}: {detail}")


def first_line(text: str) -> str:
    return text.splitlines()[0] if text else "no output"


def check_command(name: str, required: bool, must_be_on_path: bool = False) -> str | None:
    path = find_tool(name)
    if path is None:
        report("ERROR" if required else "WARN", name, "not found")
        return None

    code, output = run([path, "--version"])
    version = first_line(output) if code == 0 else "version query failed"
    if shutil.which(name) is None:
        kind = "ERROR" if required and must_be_on_path else "WARN"
        report(kind, name, f"{path} ({version}); add {Path(path).parent} to PATH")
        if must_be_on_path:
            return None
    else:
        report("OK", name, f"{path} ({version})")
    return path


def check_rustup(rustup: str | None) -> bool:
    if rustup is None:
        return False

    code, output = run([rustup, "target", "list", "--installed"])
    if code != 0:
        report("ERROR", "rustup target", first_line(output))
        return False
    if TARGET not in output.split():
        report("ERROR", "Rust target", f"{TARGET} is not installed")
        return False
    report("OK", "Rust target", TARGET)

    code, output = run([rustup, "component", "list", "--installed"])
    llvm_tools_installed = any(
        line.startswith(("llvm-tools-preview", "llvm-tools-"))
        for line in output.splitlines()
    )
    if code != 0 or not llvm_tools_installed:
        report("ERROR", "Rust component", "llvm-tools-preview is not installed")
        return False
    report("OK", "Rust component", "llvm-tools-preview")
    return True


def check_libclang() -> bool:
    search_roots: list[Path] = []
    configured = os.environ.get("LIBCLANG_PATH")
    if configured:
        search_roots.extend(Path(item) for item in configured.split(os.pathsep) if item)
    search_roots.extend(Path(path) for path in glob.glob("/usr/lib/llvm-*/lib"))
    search_roots.extend(
        [
            Path("/usr/lib/x86_64-linux-gnu"),
            Path("/usr/lib/aarch64-linux-gnu"),
            Path("/usr/local/lib"),
        ]
    )

    seen: set[Path] = set()
    for root in search_roots:
        if root in seen or not root.is_dir():
            continue
        seen.add(root)
        if any(root.glob("libclang.so*")) or any(root.glob("libclang-*.so*")):
            report("OK", "libclang", str(root))
            return True

    report("ERROR", "libclang", "install clang-18 and libclang-18-dev")
    return False


def check_optional_cargo_tools(cargo: str | None) -> None:
    if cargo is None:
        return
    for subcommand in ("objcopy", "hex-to-uf2"):
        code, output = run([cargo, subcommand, "--help"])
        if code == 0:
            report("OK", f"cargo {subcommand}", "available")
        else:
            report("INFO", f"cargo {subcommand}", "cargo-make installs it on first firmware build")


def check_probe(probe_rs: str | None) -> None:
    if probe_rs is None:
        return
    code, output = run([probe_rs, "list"])
    if code != 0:
        report("WARN", "SWD probe", first_line(output))
    elif "No debug probes" in output:
        report("WARN", "SWD probe", "probe-rs is installed, but no probe is connected")
    else:
        report("OK", "SWD probe", "probe-rs detected a debug probe")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()

    errors = 0
    if check_command("python3", required=True) is None:
        errors += 1
    cargo = check_command("cargo", required=True, must_be_on_path=True)
    rustup = check_command("rustup", required=True, must_be_on_path=True)
    cargo_make = check_command("cargo-make", required=True, must_be_on_path=True)
    flip_link = check_command("flip-link", required=True, must_be_on_path=True)
    clang = check_command("clang-18", required=False) or check_command("clang", required=True)
    probe_rs = check_command("probe-rs", required=False)

    if cargo is None or rustup is None or cargo_make is None or flip_link is None:
        errors += 1
    if not check_rustup(rustup):
        errors += 1
    if clang is None or not check_libclang():
        errors += 1
    check_optional_cargo_tools(cargo)
    check_probe(probe_rs)

    if errors:
        print("Development environment is incomplete; fix ERROR items before ARM/UF2 builds.")
        return 1
    print("Development environment is ready for host tests and ARM firmware builds.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
