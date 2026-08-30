#!/usr/bin/env python3
"""Compare the checked-in Rust dictionaries with the pinned QMK snapshot.

This is intentionally a standard-library-only check.  It does not build the
QMK firmware; it verifies the static data that the Rust implementation claims
to port, so a dictionary edit cannot silently drift from the source snapshot.
"""

import json
import os
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
MEJIRO_GIT = "https://github.com/n18011/mejiro"
MEJIRO_REV = "ae395bd9bd56a7010d79fd7e94c7494928e4e97c"
EXPECTED_SNAPSHOT = "c0f986d9608d377a0ed4ade345a8b4e9300f37c8"


def configured_path(name: str, default: Path) -> Path:
    value = os.environ.get(name)
    if value is None:
        return default
    path = Path(value).expanduser()
    return path if path.is_absolute() else ROOT / path


def mejiro_core_root() -> Path:
    configured = os.environ.get("MEJIRO_CORE_ROOT")
    if configured is not None:
        return configured_path("MEJIRO_CORE_ROOT", ROOT)

    metadata = subprocess.run(
        [
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--manifest-path",
            str(ROOT / "Cargo.toml"),
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    source = f"git+{MEJIRO_GIT}?rev={MEJIRO_REV}#{MEJIRO_REV}"
    packages = json.loads(metadata.stdout)["packages"]
    for package in packages:
        if package["name"] == "mejiro-core" and package["source"] == source:
            return Path(package["manifest_path"]).parent
    raise SystemExit(f"could not resolve mejiro-core from {source}")


CORE_ROOT = mejiro_core_root()
QMK_ROOT = configured_path(
    "MEJIRO_QMK_ROOT", ROOT / "upstream/qmk/keyboards/jeebis/mejiro31"
)
QMK_README = configured_path("MEJIRO_QMK_README", ROOT / "upstream/qmk/README.md")
RUST_TRANSFORM = CORE_ROOT / "src/mejiro_transform.rs"
RUST_VERBS = CORE_ROOT / "src/mejiro_verbs.rs"


def array_body(source: str, marker: str) -> str:
    start = source.find(marker)
    if start < 0:
        raise ValueError(f"missing table marker: {marker}")
    opening = re.search(r"=\s*&?[\[{]", source[start:])
    if opening is None:
        raise ValueError(f"missing table opening: {marker}")
    body_start = start + opening.end() - 1
    ends = [
        index
        for index in (source.find("];", body_start), source.find("};", body_start))
        if index >= 0
    ]
    if not ends:
        raise ValueError(f"unterminated table: {marker}")
    return source[start : min(ends)]


def pairs(source: str, marker: str) -> list[tuple[str, str]]:
    return re.findall(
        r'[\{\(]"([^"\\]*(?:\\.[^"\\]*)*)",\s*"([^"\\]*(?:\\.[^"\\]*)*)"[\}\)]',
        array_body(source, marker),
    )


def strings(source: str, marker: str) -> list[str]:
    body = re.sub(r"//[^\n]*", "", array_body(source, marker))
    return re.findall(r'"([^"\\]*(?:\\.[^"\\]*)*)"', body)


def verbs(source: str, marker: str, rust: bool = False) -> list[tuple[str, str, str, str]]:
    body = array_body(source, marker)
    if rust:
        return re.findall(
            r'VerbEntry\s*\{\s*stroke:\s*"([^"]*)",\s*stem:\s*"([^"]*)",\s*row:\s*\'([^\'])\',\s*kind:\s*VerbType::(\w+)',
            body,
            re.S,
        )
    return re.findall(
        r'\{"([^"\\]*)",\s*"([^"\\]*)",\s*\'([^\'])\',\s*(VERB_TYPE_\w+)\}',
        body,
    )


def compare(name: str, expected: list, actual: list) -> None:
    if expected != actual:
        missing = [entry for entry in expected if entry not in actual]
        extra = [entry for entry in actual if entry not in expected]
        raise SystemExit(
            f"{name} mismatch: expected={len(expected)} actual={len(actual)} "
            f"missing={missing[:3]} extra={extra[:3]}"
        )


def main() -> int:
    qmk_abbreviations = (QMK_ROOT / "mejiro_abbreviations.c").read_text(encoding="utf-8")
    qmk_transform = (QMK_ROOT / "mejiro_transform.c").read_text(encoding="utf-8")
    qmk_verbs = (QMK_ROOT / "mejiro_verb.c").read_text(encoding="utf-8")
    rust_transform = RUST_TRANSFORM.read_text(encoding="utf-8")
    rust_verbs = RUST_VERBS.read_text(encoding="utf-8")

    readme = QMK_README.read_text(encoding="utf-8")
    snapshot = re.search(r"Snapshot:\s*`([^`]+)`", readme)
    if snapshot is None or snapshot.group(1) != EXPECTED_SNAPSHOT:
        raise SystemExit("QMK reference snapshot metadata changed")

    table_pairs = [
        ("user abbreviations", "user_abbreviations[] =", "const USER_ABBREVIATIONS"),
        ("abstract abbreviations", "abstract_abbreviations[] =", "const ABSTRACT_ABBREVIATIONS"),
        ("abstract left", "abstract_left[] =", "const ABSTRACT_LEFT"),
        ("abstract right", "abstract_right[] =", "const ABSTRACT_RIGHT"),
    ]
    for name, qmk_marker, rust_marker in table_pairs:
        compare(name, pairs(qmk_abbreviations, qmk_marker), pairs(rust_transform, rust_marker))

    compare(
        "kana romaji table",
        pairs(qmk_transform, "kana_roma_table[]"),
        pairs(rust_transform, "const KANA_ROMAJI"),
    )
    compare(
        "kana matrix",
        strings(qmk_transform, "static const char *kana_table[][8]"),
        strings(rust_transform, "const KANA:"),
    )

    qmk_verb_entries = verbs(qmk_verbs, "static const verb_entry_t verb_dict")
    rust_verb_entries = verbs(rust_verbs, "pub(crate) const VERB_DICTIONARY", rust=True)
    kind_map = {
        "VERB_TYPE_GODAN": "Godan",
        "VERB_TYPE_KAMI": "Kami",
        "VERB_TYPE_SIMO": "Simo",
        "VERB_TYPE_KAHEN": "Kahen",
        "VERB_TYPE_SPECIAL": "Special",
    }
    expected_verbs = [(stroke, stem, row, kind_map[kind]) for stroke, stem, row, kind in qmk_verb_entries]
    intentional_rust_entries = {
        ("I-K", "", "x", "Special"),
        ("A-", "", "x", "Special"),
        ("K-", "", "x", "Kahen"),
    }
    actual_verbs = [entry for entry in rust_verb_entries if entry not in intentional_rust_entries]
    unexpected = [entry for entry in rust_verb_entries if entry not in expected_verbs and entry not in intentional_rust_entries]
    if unexpected:
        raise SystemExit(f"unexpected Rust verb entries: {unexpected[:3]}")
    compare("verb dictionary", expected_verbs, actual_verbs)

    print(
        "QMK static regression OK: "
        f"snapshot={EXPECTED_SNAPSHOT}, abbreviations={sum(len(pairs(qmk_abbreviations, q)) for _, q, _ in table_pairs)}, "
        f"verbs={len(expected_verbs)}, kana={len(pairs(qmk_transform, 'kana_roma_table[]'))}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
