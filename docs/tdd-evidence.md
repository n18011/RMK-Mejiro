# TDD検証記録

この移植は、純粋なMejiro変換層をRMK依存から分離し、ホスト上でRED→GREEN→リファクタリングを確認してからARMターゲットへ接続した。

## Checkpoints

1. **RED**: 抽出前の`src/lib.rs`にMejiro契約テストを先に追加し、実装ファイルがない状態で
   `cargo test --workspace --target x86_64-unknown-linux-gnu --lib`を実行した。期待どおり、
   Mejiro実装が見つからない`E0583`で失敗した。
2. **GREEN**: `crates/mejiro-core`、`mejiro-rmk`、動詞辞書、RMKコントローラを実装後、同じテストを実行し、
   ホスト契約テストが全件成功した。
3. **差分回帰**: `tools/qmk_regression.py` が固定QMKスナップショットの略語・動詞
   テーブルを毎回比較する。さらに`cargo make --no-workspace qmk-dynamic-regression`で
   QMK C実装とRust実装へ29,003件を流し、助詞優先順位、活用の濁音化、抽象略語の句読点、
   `#`のリピート、促音持ち越し、かな変換を実行結果で照合する。
4. **リファクタリング後**: 重複押下・順序外解放でfirst-up状態が壊れない契約テスト、Mejiroの
   略語・抽象略語・動詞助詞組合せ・かな表の網羅テスト、Mejiroをlayer 0へ固定するレイヤー契約、
   Cygnus-Mの`Kb`配置、Vialの物理配置と
   UF2コピー補助のテストを追加した。`cargo fmt --all -- --check`、ホストテスト、キーボード設定検証、
   Clippyを成功させた。`usb-debug` featureでもホストテストとClippyを成功させ、USB CDC loggerの
   feature競合がないことを確認した。`cargo llvm-cov`はCIで実行するゲートとして残している。
5. **ターゲット検証**: Rustup側の`cargo`と`RUST_MIN_STACK=67108864`を使い、ARM向け
   `usb-debug`のcentral/peripheralビルドを完了させた。`cargo make --no-workspace check-dev`で
   `thumbv7em-none-eabihf`、libclang、UF2ツール、probe-rsの検出も確認する。
6. **保存領域**: `storage.clear_storage=true`は旧Vial設定とBLE bondを一掃するための意図的な
   検証設定であり、現在の運用方針として維持する。`clear_layout=false`は別途維持し、物理配置の
   初期値とVialレイアウトの扱いを混同しない。

## 再実行コマンド

```sh
python3 tools/validate_keyboard.py
python3 -m unittest tools/test_validate_keyboard.py tools/test_flash_uf2.py
python3 tools/qmk_regression.py
cargo make --no-workspace qmk-dynamic-regression
cargo fmt --all -- --check
RUST_MIN_STACK=67108864 cargo test --workspace --target x86_64-unknown-linux-gnu --lib
RUST_MIN_STACK=67108864 cargo test --no-default-features --features usb-debug --workspace --target x86_64-unknown-linux-gnu --lib
RUST_MIN_STACK=67108864 cargo clippy --workspace --lib --target x86_64-unknown-linux-gnu -- -D warnings
RUST_MIN_STACK=67108864 cargo clippy --no-default-features --features usb-debug --workspace --lib --target x86_64-unknown-linux-gnu -- -D warnings
cargo llvm-cov --workspace --target x86_64-unknown-linux-gnu --lib --fail-under-lines 80
RUST_MIN_STACK=67108864 cargo test -p mejiro-core --target x86_64-unknown-linux-gnu --lib
cargo make --no-workspace flash-uf2-central
cargo make --no-workspace flash-uf2-usb-debug-central
cargo make --no-workspace flash-swd-central
```
