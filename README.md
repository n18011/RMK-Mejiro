# RMK-Mejiro

[JEEBIS27/qmk_firmware の Mejiro31](https://github.com/JEEBIS27/qmk_firmware/tree/master/keyboards%2Fjeebis%2Fmejiro31)
を、[n18011/Cygnus-M-RMK の `rmk-migration` branch](https://github.com/n18011/Cygnus-M-RMK/tree/rmk-migration)
をターゲットとしてRMK 0.8.2へ移植したファームウェアです。

## 構成

- Seeed XIAO nRF52840を使うBLE split keyboard
- Central側のPMW3610トラックボール、Peripheral側のEC11エンコーダー
- Vial対応、10レイヤー（QWERTY、Mejiro/Gemini、数字、機能、ポインティング、Bluetooth）
- `Kb0`〜`Kb23`をMejiro入力へ割り当て、first-up chord処理とローマ字出力をRustで実装

ハードウェア配線とsplit設定は[target branch](https://github.com/n18011/Cygnus-M-RMK/tree/rmk-migration)
を維持し、Mejiro31のRP2040固有配線は持ち込みません。移植元のCソースは
[upstream/qmk](upstream/qmk)に監査用リファレンスとして保存しています。対応表は
[docs/porting.md](docs/porting.md)を参照してください。

## ビルド

必要なもの: Rust stable、`thumbv7em-none-eabihf`、`cargo-make`、`clang`、`libclang-dev`。

```sh
rustup target add thumbv7em-none-eabihf
cargo install --force cargo-make
cargo make uf2 --release
```

生成された`central`を右側、`peripheral`を左側へ書き込みます。nRF52840 BLEの初回
ペアリングやVialロック解除は`keyboard.toml`の設定に従います。

## 開発・検証

```sh
python3 tools/validate_keyboard.py
cargo fmt --all -- --check
RUST_MIN_STACK=67108864 cargo test --target x86_64-unknown-linux-gnu --lib
cargo clippy --lib --target x86_64-unknown-linux-gnu -- -D warnings
cargo llvm-cov --target x86_64-unknown-linux-gnu --lib --fail-under-lines 80
```

CIは上記のホストTDD検証に加えて、80%以上の行カバレッジ、central/peripheralのARMビルド、
UF2/HEX/ELF成果物の保存を行います。`v*`タグでは成果物をGitHub Releaseへ公開します。
`RUST_MIN_STACK`はRMKの大きなマクロ展開を安定してコンパイルするための設定です。
