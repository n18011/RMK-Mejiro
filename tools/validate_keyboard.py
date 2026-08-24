#!/usr/bin/env python3
"""Validate the static RMK keyboard contract without external dependencies."""

from pathlib import Path
import json
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "keyboard.toml"
REFERENCE = ROOT / "upstream/qmk/keyboards/jeebis/mejiro31"
EXPECTED_VIAL_CUSTOM_KEYCODES = (
    [f"BT{index}" for index in range(3)]
    + ["NEXT_BT", "PREV_BT", "CLR_BT", "SWITCH", "CLR_PEER"]
    + [f"MEJIRO_KB{index}" for index in range(24)]
)
EXPECTED_VIAL_GEOMETRY = (
    (
        (("y", 0.37),),
        (("y", -0.25),),
        (("y", -0.12),),
        (("y", 0.12),),
        (("y", 0.12),),
        (("x", 3),),
        (("y", -0.12),),
        (("y", -0.12),),
        (("y", 0.12),),
        (("y", 0.25),),
    ),
    (
        (("y", -0.25),),
        (("y", -0.12),),
        (("y", 0.12),),
        (("y", 0.12),),
        (("y", 0.13),),
        (("x", 1),),
        (("y", -0.13),),
        (("y", -0.12),),
        (("y", -0.12),),
        (("y", 0.12),),
        (("y", 0.25),),
    ),
    (
        (("y", -0.25),),
        (("y", -0.12),),
        (("y", 0.12),),
        (("y", 0.12),),
        (("y", 0.13),),
        (("x", 1),),
        (("y", -0.13),),
        (("y", -0.12),),
        (("y", -0.12),),
        (("y", 0.12),),
        (("y", 0.25),),
    ),
    ((("y", -0.25),), (("y", -0.12),), (("x", 0.25), ("y", 0.27))),
    ((("r", 10), ("rx", 4.27), ("ry", 3.25), ("x", 1)),),
    ((("r", 20), ("rx", 5.40), ("ry", 3.42), ("x", 1)),),
    ((("r", -20), ("rx", 6.50), ("ry", 3.42), ("x", 1)),),
    ((("r", -10), ("rx", 7.63), ("ry", 3.60), ("x", 1)),),
    ((("r", 0), ("rx", 0), ("ry", 0), ("x", 13), ("y", 3.37)),),
)

EXPECTED_MEJIRO_GRID = (
    ("User8", "User9", "User10", "User11", "User12", "User13", "User20", "User21", "User22", "User23", "User24", "User25"),
    ("_", "User14", "User15", "User16", "User17", "User18", "User19", "User26", "User27", "User28", "User29", "User30", "User31", "_"),
    ("_", "_", "_", "_", "_", "_", "_", "_", "_", "_", "_", "_", "_", "_"),
    ("_", "_", "_", "_", "_", "LT(1,Space)", "_", "LT(1,Space)", "LT(2,Enter)", "LT(8,Escape)", "_"),
)
EXPECTED_LAYER_NAMES = (
    "base",
    "qwerty",
    "qwerty_shift",
    "function",
    "arrow",
    "mouse",
    "scroll",
    "numeric",
    "bluetooth",
    "number_shift",
)
EXPECTED_MEJIRO_COORDINATES = {
    0: (0, 6, "L"),
    1: (0, 0, "L"),
    2: (0, 1, "L"),
    3: (0, 2, "L"),
    4: (0, 3, "L"),
    5: (0, 4, "L"),
    6: (1, 0, "L"),
    7: (1, 1, "L"),
    8: (1, 2, "L"),
    9: (1, 3, "L"),
    10: (1, 4, "L"),
    11: (1, 5, "L"),
    12: (0, 7, "R"),
    13: (0, 8, "R"),
    14: (0, 9, "R"),
    15: (0, 10, "R"),
    16: (0, 11, "R"),
    17: (0, 12, "R"),
    18: (3, 7, "R"),
    19: (1, 7, "R"),
    20: (1, 8, "R"),
    21: (1, 9, "R"),
    22: (1, 10, "R"),
    23: (1, 11, "R"),
}
MEJIRO_USER_START = 8


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
    match = re.search(r'(?ms)^\s*map\s*=\s*"""(.*?)"""', config)
    if match is None:
        raise SystemExit("layout map is missing")
    return layer_rows(match.group(1))


def parse_matrix_coordinates(
    map_rows: list[list[str]], logical_rows: int, logical_cols: int
) -> list[tuple[int, int, str]]:
    coordinates: list[tuple[int, int, str]] = []
    seen: set[tuple[int, int]] = set()
    for visual_row, row in enumerate(map_rows):
        for visual_col, token in enumerate(row):
            match = re.fullmatch(r"\((\d+),(\d+),([LR])\)", token)
            if match is None:
                raise ValueError(
                    f"invalid layout.map token at row {visual_row}, column {visual_col}: {token!r}"
                )
            mapped_row, mapped_col = (int(value) for value in match.groups()[:2])
            if mapped_row >= logical_rows or mapped_col >= logical_cols:
                raise ValueError(f"layout.map coordinate outside layout: {token!r}")
            coordinate = (mapped_row, mapped_col)
            if coordinate in seen:
                raise ValueError(f"layout.map coordinate is duplicated: {token!r}")
            seen.add(coordinate)
            coordinates.append((mapped_row, mapped_col, match.group(3)))
    return coordinates


def validate_hand_coordinates(
    coordinates: list[tuple[int, int, str]],
    windows: dict[str, tuple[int, int, int, int]],
) -> None:
    for row, col, hand in coordinates:
        row_offset, row_count, col_offset, col_count = windows[hand]
        if not (
            row_offset <= row < row_offset + row_count
            and col_offset <= col < col_offset + col_count
        ):
            raise ValueError(f"layout.map {hand} coordinate is outside its split matrix: ({row},{col})")


def vial_keymap_coordinates(vial: dict) -> list[tuple[int, int]]:
    keymap = vial.get("layouts", {}).get("keymap")
    if not isinstance(keymap, list):
        raise ValueError("vial.json layouts.keymap is missing")

    coordinates: list[tuple[int, int]] = []
    for row_index, row in enumerate(keymap):
        if not isinstance(row, list):
            raise ValueError(f"vial.json keymap row {row_index} is not an array")
        for item in row:
            if isinstance(item, dict):
                continue
            if not isinstance(item, str):
                raise ValueError(f"vial.json keymap item has an invalid type: {item!r}")
            match = re.fullmatch(r"(\d+),(\d+)", item)
            if match is None:
                raise ValueError(f"vial.json keymap item is not a matrix coordinate: {item!r}")
            coordinates.append(tuple(int(value) for value in match.groups()))
    return coordinates


def vial_geometry(vial: dict) -> tuple[tuple[tuple[str, int | float], ...], ...]:
    keymap = vial["layouts"]["keymap"]
    return tuple(
        tuple(
            tuple(sorted(item.items()))
            for item in row
            if isinstance(item, dict)
        )
        for row in keymap
    )


def validate_vial_contract(vial_path: Path, expected_coordinates: list[tuple[int, int]]) -> None:
    try:
        vial = json.loads(vial_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"unable to read vial.json: {error}") from error

    if vial.get("name") != "Cygnus-M":
        raise ValueError("vial.json name must be Cygnus-M")
    if vial.get("matrix") != {"rows": 4, "cols": 13}:
        raise ValueError(f"vial.json matrix does not match the firmware: {vial.get('matrix')!r}")

    custom_keycodes = vial.get("customKeycodes")
    actual_names = [entry.get("name") for entry in custom_keycodes or []]
    if actual_names != EXPECTED_VIAL_CUSTOM_KEYCODES:
        raise ValueError(
            "vial.json custom keycode order does not match keyboard.toml/RMK: "
            f"{actual_names!r}"
        )

    keymap = vial["layouts"]["keymap"]
    if len(keymap) != 9 or not any(
        isinstance(item, dict) and "r" in item
        for row in keymap
        for item in row
    ):
        raise ValueError("vial.json must retain the physical Cygnus-M thumb-key geometry")
    if vial_geometry(vial) != EXPECTED_VIAL_GEOMETRY:
        raise ValueError("vial.json physical geometry differs from the Cygnus-M contract")

    actual_coordinates = vial_keymap_coordinates(vial)
    if actual_coordinates != expected_coordinates:
        raise ValueError(
            "vial.json keymap coordinates differ from keyboard.toml layout.map: "
            f"{actual_coordinates!r} != {expected_coordinates!r}"
        )


def validate_mejiro_key_ids(key_ids: list[int]) -> None:
    if (
        len(key_ids) != 24
        or len(set(key_ids)) != 24
        or any(key_id > 31 for key_id in key_ids)
    ):
        raise ValueError(
            "base Mejiro must contain 24 unique keys from User0..User31: "
            f"{key_ids}"
        )


def validate_mejiro_layout(
    rows: list[list[str]], map_rows: list[list[str]]
) -> None:
    if tuple(tuple(row) for row in rows) != EXPECTED_MEJIRO_GRID:
        raise ValueError(
            "base Mejiro User placement differs from the Cygnus-M contract: "
            f"{rows!r}"
        )

    for visual_row, row in enumerate(rows):
        for visual_col, token in enumerate(row):
            match = re.fullmatch(r"User(\d+)", token)
            if match is None:
                continue
            coordinate = re.fullmatch(
                r"\((\d+),(\d+),([LR])\)", map_rows[visual_row][visual_col]
            )
            if coordinate is None:
                raise ValueError(
                    f"invalid layout.map token for {token}: "
                    f"{map_rows[visual_row][visual_col]!r}"
                )
            actual = (
                int(coordinate.group(1)),
                int(coordinate.group(2)),
                coordinate.group(3),
            )
            user_key = int(match.group(1))
            logical_index = user_key - MEJIRO_USER_START
            if not 0 <= logical_index < 24:
                raise ValueError(f"{token} is outside the default Mejiro User8..User31 range")
            expected = EXPECTED_MEJIRO_COORDINATES[logical_index]
            if actual != expected:
                raise ValueError(
                    f"{token} is mapped to {actual}, expected {expected}"
                )


def main() -> int:
    config = CONFIG.read_text(encoding="utf-8")
    if not REFERENCE.is_dir():
        raise SystemExit("upstream QMK reference is missing")

    layout = section(config, "layout")
    keymap = section(config, "keymap")
    layer_count = integer(keymap, "layers")
    logical_rows = integer(layout, "rows")
    logical_cols = integer(layout, "cols")
    map_rows = matrix_map(layout)
    if len(map_rows) != logical_rows:
        raise SystemExit(f"layout map has {len(map_rows)} rows, expected {logical_rows}")

    try:
        coordinates = parse_matrix_coordinates(map_rows, logical_rows, logical_cols)
    except ValueError as error:
        raise SystemExit(str(error)) from error

    blocks = re.findall(r'\[\[keymap\.layer\]\]\s*\nname = "([^"]+)"\s*\nkeys = """(.*?)"""', config, re.S)
    if len(blocks) != layer_count:
        raise SystemExit(f"layout declares {layer_count} layers, found {len(blocks)} blocks")

    names = []
    for name, body in blocks:
        names.append(name)
        rows = layer_rows(body)
        counts = [len(row) for row in rows]
        expected = [len(row) for row in map_rows]
        if counts != expected:
            raise SystemExit(f"layer {name!r} has row widths {counts}, expected layout map widths {expected}")

    if len(set(names)) != len(names):
        raise SystemExit("layer names must be unique")
    if tuple(names) != EXPECTED_LAYER_NAMES:
        raise SystemExit(
            "layer order must keep Mejiro as the default base layer: "
            f"{names!r} != {list(EXPECTED_LAYER_NAMES)!r}"
        )

    base_body = next(body for name, body in blocks if name == "base")
    try:
        validate_mejiro_layout(layer_rows(base_body), map_rows)
    except ValueError as error:
        raise SystemExit(str(error)) from error

    layer_indices = {name: index for index, name in enumerate(names)}
    for required_layer in ("qwerty", "qwerty_shift", "bluetooth"):
        layer_number = layer_indices.get(required_layer)
        if layer_number is None or not re.search(rf"\bLT\({layer_number},", base_body):
            raise SystemExit(f"base layer has no layer-tap entry for {required_layer!r}")

    qwerty_body = next(body for name, body in blocks if name == "qwerty")
    for required_layer in ("qwerty_shift", "numeric", "bluetooth", "number_shift"):
        layer_number = layer_indices.get(required_layer)
        if layer_number is None or not re.search(rf"\bLT\({layer_number},", qwerty_body):
            raise SystemExit(f"qwerty layer has no layer-tap entry for {required_layer!r}")

    mouse_body = next(body for name, body in blocks if name == "mouse")
    for required_layer in ("scroll", "numeric"):
        layer_number = layer_indices.get(required_layer)
        if layer_number is None or not re.search(rf"\bLT\({layer_number},Space\)", mouse_body):
            raise SystemExit(f"mouse layer has no layer-tap entry for {required_layer!r}")

    for name, body in blocks:
        key_ids = [int(value) for value in re.findall(r"\bUser(\d+)\b", body)]
        if name == "base":
            try:
                validate_mejiro_key_ids(key_ids)
            except ValueError as error:
                raise SystemExit(str(error)) from error
        elif key_ids and (
            name != "bluetooth" or any(key_id >= MEJIRO_USER_START for key_id in key_ids)
        ):
            raise SystemExit(f"Mejiro User keys leaked into layer {name!r}: {key_ids}")

    split = section(config, "split")
    if not re.search(r'(?m)^\s*connection\s*=\s*"ble"\s*$', split):
        raise SystemExit("split connection must be BLE")
    split_windows: dict[str, tuple[int, int, int, int]] = {}
    for name, hand in (("split.central", "R"), ("split.peripheral", "L")):
        split_section = section(config, name)
        split_rows = integer(split_section, "rows")
        split_cols = integer(split_section, "cols")
        row_offset = integer(split_section, "row_offset")
        col_offset = integer(split_section, "col_offset")
        if row_offset + split_rows > logical_rows or col_offset + split_cols > logical_cols:
            raise SystemExit(f"{name} matrix exceeds logical layout bounds")
        split_windows[hand] = (row_offset, split_rows, col_offset, split_cols)

    try:
        validate_hand_coordinates(coordinates, split_windows)
        validate_vial_contract(
            ROOT / "vial.json", [(row, col) for row, col, _hand in coordinates]
        )
    except ValueError as error:
        raise SystemExit(str(error)) from error

    all_slots = {(row, col) for row in range(logical_rows) for col in range(logical_cols)}
    mapped_slots = {(row, col) for row, col, _hand in coordinates}
    unused_slots = sorted(all_slots - mapped_slots)
    storage = section(config, "storage")
    if re.search(r"(?m)^\s*clear_layout\s*=\s*true\s*$", storage):
        raise SystemExit("storage.clear_layout must remain false to preserve Vial layout state")
    clear_storage = re.search(
        r"(?m)^\s*clear_storage\s*=\s*(true|false)(?:\s*#.*)?$", storage
    )
    if clear_storage is None:
        raise SystemExit("storage.clear_storage must be declared explicitly")
    print(
        "keyboard contract OK: "
        f"layers={layer_count}, matrix_widths={[len(row) for row in map_rows]}, "
        f"mapped_keys={len(coordinates)}, unused_slots={unused_slots}, "
        "mejiro_layout=cygnus-m, "
        f"names={', '.join(names)}, clear_storage={clear_storage.group(1)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
