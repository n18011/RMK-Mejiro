# TDD検証記録

この移植は、純粋なMejiro変換層をRMK依存から分離し、ホスト上でRED→GREEN→リファクタリングを確認してからARMターゲットへ接続した。

## Checkpoints

1. **RED**: 抽出前の`src/lib.rs`にMejiro契約テストを先に追加し、実装ファイルがない状態で
   `cargo test --workspace --target x86_64-unknown-linux-gnu --lib`を実行した。期待どおり、
   Mejiro実装が見つからない`E0583`で失敗した。
2. **GREEN**: `crates/mejiro-core`、`mejiro-rmk`、動詞辞書、RMKコントローラを実装後、同じテストを実行し、
   ホスト契約テストが全件成功した。
3. **差分回帰**: `tools/qmk_regression.py` が固定QMKスナップショットの略語・動詞
   テーブルを毎回比較する。変換結果は、助詞優先順位、活用の濁音化、抽象略語の句読点、
   `#`のリピート、促音持ち越し、HID操作、履歴、マクロの境界テストで固定する。
4. **リファクタリング後**: `cargo fmt --all -- --check`、ホストテスト、キーボード設定検証、
   Clippyを成功させた。`cargo llvm-cov`はCIで実行するゲートとして残している。
5. **ターゲット検証**: `cargo make uf2 --release`を実行したが、環境に`libclang`共有ライブラリが
   ないため`nrf-sdc`のBindgenで停止した。CIでは`clang`／`libclang-dev`を導入して同じゲートを
   実行する。

## 再実行コマンド

```sh
python3 tools/validate_keyboard.py
python3 tools/qmk_regression.py
cargo fmt --all -- --check
RUST_MIN_STACK=67108864 cargo test --workspace --target x86_64-unknown-linux-gnu --lib
cargo clippy --workspace --lib --target x86_64-unknown-linux-gnu -- -D warnings
cargo llvm-cov --workspace --target x86_64-unknown-linux-gnu --lib --fail-under-lines 80
cargo make uf2 --release
```
