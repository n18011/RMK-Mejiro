# Mejiro31移植メモ

## 対応関係

| QMK source | RMK-Mejiro |
| --- | --- |
| `mejiro_fifo.c` first-up chord FIFO | `MejiroSession` |
| `mejiro_transform.c` | `src/mejiro.rs` |
| `mejiro_commands.c` | `KeyAction` とコマンド分岐 |
| `mejiro_abbreviations.c` | 略語テーブル |
| `mejiro_verb.c` | 特殊動詞・活用拡張ポイント |
| `jis_transform.h` | RMKのHIDキーコード境界で吸収 |

移植先のハードウェアは、ユーザー指定の
[`n18011/Cygnus-M-RMK` `rmk-migration` branch](https://github.com/n18011/Cygnus-M-RMK/tree/rmk-migration)
です。したがって、RP2040のMejiro31単体配線を複製せず、Cygnus-MのnRF52840 BLE split、
PMW3610、EC11、Vial構成へMejiro入力層を載せています。

## キーイベント境界

RMKの標準キーマップには`Kb0`〜`Kb23`を割り当て、RMK本体の標準キー処理が解決した
イベントをvendored RMKの`MEJIRO_EVENT_CHANNEL`へ複製します。`MejiroController`はその
イベントだけを消費して、first-up確定、ローマ字変換、キーコード出力を担当します。

この境界を設けることで、Mejiroの純粋ロジックはホスト上でTDDでき、BLE splitや
トラックボールのドライバとは独立して検証できます。

## 検証

```sh
RUST_MIN_STACK=67108864 cargo fmt --all -- --check
RUST_MIN_STACK=67108864 cargo test --target x86_64-unknown-linux-gnu --lib
cargo clippy --lib --target x86_64-unknown-linux-gnu -- -D warnings
cargo llvm-cov --target x86_64-unknown-linux-gnu --lib --fail-under-lines 80
cargo make uf2 --release
```

`nrf-sdc`のBindgenには`clang`と`libclang-dev`が必要です。CIはホストTDD、Clippy、
カバレッジ、キーボード設定検証、central/peripheralのファームウェアビルドを実行します。
