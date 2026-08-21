#!/usr/bin/env python3
"""Compare the Rust engine with the pinned QMK engine on a generated corpus.

The cheaper ``qmk_regression.py`` check compares copied tables.  This check
also executes both implementations, so ordering and post-processing bugs in
abbreviations, verbs, particles, kana conversion, and sokuon handling cannot
hide behind matching static data.
"""

from __future__ import annotations

from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
QMK_ROOT = ROOT / "upstream/qmk/keyboards/jeebis/mejiro31"
TARGET = "x86_64-unknown-linux-gnu"


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


def verb_strokes(source: str) -> list[str]:
    body = array_body(source, "static const verb_entry_t verb_dict")
    return re.findall(r'\{"([^"\\]*)",\s*"([^"\\]*)",\s*\'[^\']\',', body)


def qmk_probe_source() -> str:
    return f'''#include <stdio.h>
#include <string.h>
#include "qmk_harness_stub.h"
#include "mejiro_transform.c"
#include "mejiro_commands.c"
#include "mejiro_abbreviations.c"
#include "mejiro_verb.c"

static void reset_qmk_state(void) {{
    strcpy(last_vowel_stroke, "A");
    prev_joshi[0] = '\\0';
    pending_tsu = false;
}}

static void emit_transform(const char *stroke, bool repeat) {{
    mejiro_result_t result = mejiro_transform(stroke);
    if (!result.success) {{
        printf("T\\t0\\t\\n");
        return;
    }}
    if (repeat) {{
        char repeated[256] = {{0}};
        strcpy(repeated, result.output);
        strcat(repeated, result.output);
        printf("T\\t1\\t%s\\n", repeated);
    }} else {{
        printf("T\\t1\\t%s\\n", result.output);
    }}
}}

int main(void) {{
    char line[512];
    while (fgets(line, sizeof(line), stdin) != NULL) {{
        line[strcspn(line, "\\r\\n")] = '\\0';
        if (strncmp(line, "K:", 2) == 0) {{
            char output[256] = {{0}};
            kana_to_roma(line + 2, output, sizeof(output));
            printf("K\\t%s\\n", output);
            continue;
        }}
        if (strncmp(line, "T:", 2) != 0) {{
            return 3;
        }}
        const char *stroke = line + 2;
        if (strcmp(stroke, "STKNYIAUntk#-STKNYIAUntk*") == 0) {{
            printf("T\\tN\\t\\n");
            continue;
        }}

        reset_qmk_state();
        bool has_hash = strchr(stroke, '#') != NULL;
        char raw_stroke[256] = {{0}};
        strncpy(raw_stroke, stroke, sizeof(raw_stroke) - 1);
        char *asterisk = strchr(raw_stroke, '*');
        if (asterisk != NULL) {{
            *asterisk = '\\0';
        }}
        bool is_user_abbreviation =
            has_hash && mejiro_user_abbreviation(raw_stroke, true).success;
        if (has_hash && !is_user_abbreviation) {{
            char normalized[256] = {{0}};
            for (const char *p = stroke; *p != '\\0'; p++) {{
                if (*p != '#') {{
                    size_t len = strlen(normalized);
                    normalized[len] = *p;
                    normalized[len + 1] = '\\0';
                }}
            }}
            emit_transform(normalized, true);
        }} else {{
            emit_transform(stroke, false);
        }}
    }}
    return 0;
}}
'''


def qmk_stub_source() -> str:
    return '''#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define KC_BSPC 0x2a
#define KC_DEL 0x4c
#define KC_DOWN 0x51
#define KC_END 0x4d
#define KC_ENTER 0x28
#define KC_ESC 0x29
#define KC_HOME 0x4a
#define KC_LEFT 0x50
#define KC_LNG1 0x90
#define KC_LNG2 0x91
#define KC_RIGHT 0x4f
#define KC_SPACE 0x2c
#define KC_TAB 0x2b
#define KC_UP 0x52

#define LSFT(keycode) ((uint16_t)((keycode) | 0x0100u))
#define LCTL(keycode) ((uint16_t)((keycode) | 0x0200u))
'''


def rust_probe_manifest() -> str:
    return f'''[package]
name = "mejiro-dynamic-probe"
version = "0.0.0"
edition = "2021"

[dependencies]
mejiro-core = {{ path = "{(ROOT / "crates/mejiro-core").as_posix()}" }}
'''


def rust_probe_source() -> str:
    return '''use std::io::{self, BufRead};

use mejiro_core::mejiro::{kana_to_romaji, transform, StrokeResult};

fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.expect("stdin");
        if let Some(input) = line.strip_prefix("K:") {
            match kana_to_romaji(input) {
                Ok(output) => println!("K\\t{output}"),
                Err(error) => println!("KERR\\t{error:?}"),
            }
            continue;
        }

        let stroke = line.strip_prefix("T:").expect("probe input prefix");
        match transform(stroke) {
            StrokeResult::Text { text, .. } => println!("T\\t1\\t{text}"),
            StrokeResult::Unsupported => println!("T\\t0\\t"),
            StrokeResult::Noop => println!("T\\tN\\t"),
            other => println!("T\\tN\\t{other:?}"),
        }
    }
}
'''


def build_c_probe(directory: Path) -> Path:
    source = directory / "qmk_probe.c"
    source.write_text(qmk_probe_source(), encoding="utf-8")
    (directory / "qmk_harness_stub.h").write_text(qmk_stub_source(), encoding="utf-8")
    binary = directory / "qmk-probe"
    subprocess.run(
        [
            "cc",
            "-std=c11",
            "-O2",
            '-DQMK_KEYBOARD_H="qmk_harness_stub.h"',
            "-I",
            str(directory),
            "-I",
            str(QMK_ROOT),
            str(source),
            "-o",
            str(binary),
        ],
        cwd=ROOT,
        check=True,
    )
    return binary


def build_rust_probe(directory: Path) -> Path:
    directory.mkdir(parents=True)
    (directory / "Cargo.toml").write_text(rust_probe_manifest(), encoding="utf-8")
    source_dir = directory / "src"
    source_dir.mkdir()
    (source_dir / "main.rs").write_text(rust_probe_source(), encoding="utf-8")
    return directory


def make_corpus(transform_source: str) -> list[str]:
    corpus: list[str] = []
    seen: set[str] = set()

    def add(prefix: str, value: str) -> None:
        item = f"{prefix}:{value}"
        if item not in seen:
            seen.add(item)
            corpus.append(item)

    abbreviation_source = (QMK_ROOT / "mejiro_abbreviations.c").read_text(encoding="utf-8")
    for stroke, _kana in pairs(abbreviation_source, "user_abbreviations[] ="):
        add("T", stroke)
    for stroke, _kana in pairs(abbreviation_source, "abstract_abbreviations[] ="):
        add("T", stroke)

    left = pairs(abbreviation_source, "abstract_left[] =")
    right = pairs(abbreviation_source, "abstract_right[] =")
    for left_stroke, _left_kana in left:
        for right_stroke, _right_kana in right:
            add("T", f"{left_stroke}-{right_stroke}*")

    verb_source = (QMK_ROOT / "mejiro_verb.c").read_text(encoding="utf-8")
    particles = ["", "n", "t", "k", "tk", "nt", "nk", "ntk"]
    for stroke, _stem in verb_strokes(verb_source):
        left_stroke, right_stroke = stroke.split("-", 1)
        for left_particle in particles:
            for right_particle in particles:
                add(
                    "T",
                    f"{left_stroke}{left_particle}-{right_stroke}{right_particle}*",
                )

    consonants = ["", "S", "T", "K", "N", "ST", "SK", "TK", "SN", "TN", "KN", "TKN", "STK", "STN", "SKN", "STKN"]
    vowels = ["A", "I", "U", "IA", "AU", "YA", "YU", "YAU"]
    sounds = [consonant + vowel for consonant in consonants for vowel in vowels]
    for sound in sounds:
        add("T", sound)
        for particle in particles:
            add("T", f"{sound}{particle}-")
    for left_sound in sounds:
        for right_sound in sounds:
            add("T", f"{left_sound}-{right_sound}")

    for stroke in [
        "STNtk-Atk",
        "STNtk-KAtk",
        "AUn-YAUntk*",
        "AUn-YAUt*",
        "AUk-AUtk",
        "#A-STU*",
        "#NIAntk-SAn*",
        "STKNYIAUntk#-STKNYIAUntk*",
    ]:
        add("T", stroke)

    kana_source = transform_source
    for kana, _romaji in pairs(kana_source, "kana_roma_table[]"):
        add("K", kana)
    for kana in ["っ", "っあ", "っか", "っー", "abcあ", "あ🙂い"]:
        add("K", kana)
    return corpus


def run_probe(command: list[str], corpus: list[str], env: dict[str, str] | None = None) -> list[str]:
    result = subprocess.run(
        command,
        input="\n".join(corpus) + "\n",
        text=True,
        capture_output=True,
        env=env,
    )
    if result.returncode != 0:
        raise SystemExit(
            f"probe failed ({result.returncode}): {command[0]}\n{result.stderr}"
        )
    return result.stdout.splitlines()


def main() -> int:
    if shutil.which("cc") is None:
        raise SystemExit("cc is required for the dynamic QMK regression")

    transform_source = (QMK_ROOT / "mejiro_transform.c").read_text(encoding="utf-8")
    corpus = make_corpus(transform_source)
    with tempfile.TemporaryDirectory(prefix="rmk-mejiro-qmk-") as temp:
        temp_dir = Path(temp)
        c_binary = build_c_probe(temp_dir)
        rust_project = build_rust_probe(temp_dir / "rust-probe")
        c_output = run_probe([str(c_binary)], corpus)
        rust_output = run_probe(
            [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                str(rust_project / "Cargo.toml"),
                "--target",
                TARGET,
            ],
            corpus,
            env={
                **dict(__import__("os").environ),
                "CARGO_NET_OFFLINE": "true",
                "RUST_MIN_STACK": "67108864",
            },
        )

    if len(c_output) != len(corpus) or len(rust_output) != len(corpus):
        raise SystemExit(
            f"probe output length mismatch: corpus={len(corpus)} qmk={len(c_output)} rust={len(rust_output)}"
        )

    mismatches: list[str] = []
    intentional_differences = 0
    for item, qmk, rust in zip(corpus, c_output, rust_output):
        if item.startswith("K:"):
            expected = qmk.split("\t", 1)[1] if "\t" in qmk else qmk
            actual = rust.split("\t", 1)[1] if "\t" in rust else rust
            if expected != actual:
                if item == "K:っー" and expected == "--" and actual == "xtu-":
                    intentional_differences += 1
                    continue
                mismatches.append(f"{item}: QMK={qmk!r} Rust={rust!r}")
            continue

        qmk_parts = qmk.split("\t", 2)
        rust_parts = rust.split("\t", 2)
        if len(qmk_parts) != 3 or len(rust_parts) != 3:
            mismatches.append(f"{item}: malformed QMK={qmk!r} Rust={rust!r}")
            continue
        if qmk_parts[1:] != rust_parts[1:]:
            mismatches.append(f"{item}: QMK={qmk!r} Rust={rust!r}")

    if mismatches:
        print("QMK dynamic regression FAILED", file=sys.stderr)
        print("\n".join(mismatches[:20]), file=sys.stderr)
        print(f"additional mismatches: {max(0, len(mismatches) - 20)}", file=sys.stderr)
        return 1

    print(
        "QMK dynamic regression OK: "
        f"cases={len(corpus)}, transform={sum(item.startswith('T:') for item in corpus)}, "
        f"kana={sum(item.startswith('K:') for item in corpus)}, "
        f"intentional_qmk_differences={intentional_differences}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
