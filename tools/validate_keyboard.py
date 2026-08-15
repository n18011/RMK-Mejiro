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


def section(text: str, name: str) -> str:
    match = re.search(
        rf"(?ms)^\[{{1,2}}{re.escape(name)}\]{{1,2}}\s*\n(.*?)(?=^\[|\Z)",
        text,
    )
    if match is None:
        raise SystemExit(f"missing [{name}] section")
    return match.group(1)


def integer(section_text: str, key: str) -> int:
    match = re.search(rf"(?m)^\s*{re.escape(key)}\s*=\s*(\d+)\s*$", section_text)
    if match is None:
        raise SystemExit(f"missing integer {key!r}")
    return int(match.group(1))


def matrix_map(config: str) -> list[list[str]]:
    match = re.search(r'(?ms)^\s*matrix_map\s*=\s*"""(.*?)"""', config)
    if match is None:
        raise SystemExit("layout matrix_map is missing")
    return layer_rows(match.group(1))


def validate_mejiro_key_ids(key_ids: list[int]) -> None:
    if (
        len(key_ids) != 24
        or len(set(key_ids)) != 24
        or any(key_id > 31 for key_id in key_ids)
    ):
        raise ValueError(
            "mejiro_gemini must contain 24 unique keys from Kb0..Kb31: "
            f"{key_ids}"
        )


def main() -> int:
    config = CONFIG.read_text(encoding="utf-8")
    if not REFERENCE.is_dir():
        raise SystemExit("upstream QMK reference is missing")

    layout = section(config, "layout")
    layer_count = integer(layout, "layers")
    logical_rows = integer(layout, "rows")
    logical_cols = integer(layout, "cols")
    map_rows = matrix_map(layout)
    if len(map_rows) != logical_rows:
        raise SystemExit(f"matrix_map has {len(map_rows)} rows, expected {logical_rows}")

    for row_index, row in enumerate(map_rows):
        for token in row:
            match = re.fullmatch(r"\((\d+),(\d+),([LR])\)", token)
            if match is None:
                raise SystemExit(f"invalid matrix_map token at row {row_index}: {token!r}")
            mapped_row, mapped_col = (int(value) for value in match.groups()[:2])
            if mapped_row >= logical_rows or mapped_col >= logical_cols:
                raise SystemExit(f"matrix_map coordinate outside layout: {token!r}")

    blocks = re.findall(r'\[\[layer\]\]\s*\nname = "([^"]+)"\s*\nkeys = """(.*?)"""', config, re.S)
    if len(blocks) != layer_count:
        raise SystemExit(f"layout declares {layer_count} layers, found {len(blocks)} blocks")

    names = []
    for name, body in blocks:
        names.append(name)
        rows = layer_rows(body)
        counts = [len(row) for row in rows]
        expected = [len(row) for row in map_rows]
        if counts != expected:
            raise SystemExit(f"layer {name!r} has row widths {counts}, expected matrix_map widths {expected}")

    if len(set(names)) != len(names):
        raise SystemExit("layer names must be unique")
    if "mejiro_gemini" not in names:
        raise SystemExit("mejiro_gemini layer is missing")

    for name, body in blocks:
        key_ids = [int(value) for value in re.findall(r"\bKb(\d+)\b", body)]
        if name == "mejiro_gemini":
            try:
                validate_mejiro_key_ids(key_ids)
            except ValueError as error:
                raise SystemExit(str(error)) from error
        elif key_ids:
            raise SystemExit(f"Mejiro Kb keys leaked into layer {name!r}: {key_ids}")

    split = section(config, "split")
    if not re.search(r'(?m)^\s*connection\s*=\s*"ble"\s*$', split):
        raise SystemExit("split connection must be BLE")
    for name in ("split.central", "split.peripheral"):
        split_section = section(config, name)
        split_rows = integer(split_section, "rows")
        split_cols = integer(split_section, "cols")
        row_offset = integer(split_section, "row_offset")
        col_offset = integer(split_section, "col_offset")
        if row_offset + split_rows > logical_rows or col_offset + split_cols > logical_cols:
            raise SystemExit(f"{name} matrix exceeds logical layout bounds")

    print(f"keyboard contract OK: layers={layer_count}, matrix_widths={[len(row) for row in map_rows]}, names={', '.join(names)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
