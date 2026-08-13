# TDD検証記録

この移植は、純粋なMejiro変換層をRMK依存から分離し、ホスト上でRED→GREEN→リファクタリングを確認してからARMターゲットへ接続した。

## Checkpoints

1. **RED**: `src/lib.rs`にMejiro契約テストを先に追加し、実装ファイルがない状態で
   `cargo test --target x86_64-unknown-linux-gnu --lib`を実行した。期待どおり、
   `src/mejiro.rs`が見つからない`E0583`で失敗した。
2. **GREEN**: `src/mejiro.rs`、動詞辞書、RMKコントローラを実装後、同じテストを実行し、
   最終的に16テスト全件成功した。
3. **リファクタリング後**: `cargo fmt --all -- --check`、Clippyの`-D warnings`、
   `cargo llvm-cov --lib --fail-under-lines 80`を実行し、行カバレッジ84.04%を確認した。
4. **ターゲット検証**: central/peripheralを`thumbv7em-none-eabihf`向けにreleaseビルドし、
   nRF52840用UF2、HEX、ELFの生成と`file`による形式確認を完了した。

## 再実行コマンド

```sh
python3 tools/validate_keyboard.py
cargo fmt --all -- --check
RUST_MIN_STACK=67108864 cargo test --target x86_64-unknown-linux-gnu --lib
cargo clippy --lib --target x86_64-unknown-linux-gnu -- -D warnings
cargo llvm-cov --target x86_64-unknown-linux-gnu --lib --fail-under-lines 80
cargo make uf2 --release
```
