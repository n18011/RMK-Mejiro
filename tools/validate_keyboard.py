#!/usr/bin/env python3
"""Validate the static RMK keyboard contract without external dependencies."""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "keyboard.toml"
REFERENCE = ROOT / "upstream/qmk/keyboards/jeebis/mejiro31"


def layer_rows(text: str) -> list[list[str]]:
    rows: list[list[str]] = []
    for raw in text.strip().splitlines():
        line = raw.strip()
        if line:
            row: list[str] = []
            token: list[str] = []
            depth = 0
            for char in line:
                if char == "(":
                    depth += 1
                elif char == ")":
                    depth -= 1
                if char.isspace() and depth == 0:
                    if token:
                        row.append("".join(token))
                        token.clear()
                else:
                    token.append(char)
            if token:
                row.append("".join(token))
            rows.append(row)
    return rows


def main() -> int:
    config = CONFIG.read_text(encoding="utf-8")
    if "layers = 10" not in config:
        raise SystemExit("keyboard.toml must define ten layers")
    if not REFERENCE.is_dir():
        raise SystemExit("upstream QMK reference is missing")

    blocks = re.findall(r'\[\[layer\]\]\s*\nname = "([^"]+)"\s*\nkeys = """(.*?)"""', config, re.S)
    if len(blocks) != 10:
        raise SystemExit(f"expected 10 layer blocks, found {len(blocks)}")

    expected = [12, 14, 14, 11]
    names = []
    for name, body in blocks:
        names.append(name)
        rows = layer_rows(body)
        counts = [len(row) for row in rows]
        if counts != expected:
            raise SystemExit(f"layer {name!r} has row widths {counts}, expected {expected}")

    if names[:3] != ["base", "mejiro_gemini", "qwerty_shift"]:
        raise SystemExit(f"unexpected first layers: {names[:3]}")
    print(f"keyboard contract OK: {', '.join(names)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
