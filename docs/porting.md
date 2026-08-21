# Mejiro31移植メモ

現行実装の機能仕様は [`specification.md`](specification.md)、この文書は QMK 版との対応理由と
差分監査を扱います。

## 対応関係

| QMK source | RMK-Mejiro |
| --- | --- |
| `mejiro_fifo.c` first-up chord FIFO | `MejiroSession` |
| `mejiro_transform.c` | `crates/mejiro-core/src/mejiro_transform.rs` |
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

## crate境界

第1段階として、変換・セッション・出力契約・辞書・契約テストを
`crates/mejiro-core`へ抽出しました。このcrateは`heapless`だけに依存する`no_std`ライブラリで、
RMKのイベントチャネルやボード設定を参照しません。ルートcrateは互換re-exportを提供し、
既存の`MejiroController`から同じAPIを使える状態を維持しています。

RMK固有の`MejiroController`は`crates/mejiro-rmk`アダプターcrateへ分離しました。
`keyboard.toml`、BLE/Vial、nRF52840の依存はボードアプリ側に残しています。
アダプターが有効化するRMK機能は`controller`と`mejiro`だけで、storage・Vial・splitは
利用側のボードcrateが選択します。
入力コードは既定では`Kb0`〜`Kb23`ですが、別のキーマップでは
`MejiroController::with_keymap(MejiroKeyMap::new([...]))`で`Kb0`〜`Kb31`から選んだ任意の24キーへ
差し替えられます（RMKの`mejiro`イベント境界が仮想`Kb`キーを通知するためです）。

## 差分監査と判定

比較基準は`upstream/qmk/README.md`に固定したQMKスナップショット
`c0f986d9608d377a0ed4ade345a8b4e9300f37c8`です。変換表（かな、略語、動詞辞書）と
変換順序を機械的に照合する`tools/qmk_regression.py`をCIで実行します。現在の
ハーネスはかな154件、略語117件・動詞181件の静的テーブルを比較し、QMK側に意図的に存在しない
Rust固有の3エントリも明示的に記録します。さらに`tools/qmk_dynamic_regression.py`は
QMK C実装とRust実装へ同じ29,003ケースを流し、実行時の出力も比較します。

| 項目 | QMK版 | RMK版 | 判定 |
| --- | --- | --- | --- |
| ハードウェア | Mejiro31 / RP2040固有の配線 | Cygnus-M / nRF52840 BLE splitの`Kb0`〜`Kb23` | 意図した適応 |
| HID出力 | `send_string`と任意のJIS記号変換 | RMKの`from_ascii`によるUS-HID ASCII出力 | 意図した適応。JISモードは未提供 |
| 変換失敗時 | GeminiのSTNキーをパススルー | `Unsupported`を出力せず破棄 | 意図した適応。RMK側にGemini raw HID境界がないため |
| 全押しキャンセル | 変換失敗で無出力 | `StrokeResult::Noop`で明示 | 等価 |
| `A-`／`K-` | QMKの動詞特殊処理が無印にも適用される | 無印は基本音`a`／`ka`、`*`付きだけ動詞 | 意図した衝突回避。無印の基本母音を守る |
| 促音＋長音符 `っー` | `--`（QMKの非アルファベット先頭処理） | `xtu-` | 意図した修正。促音を長音符へ重ねず、入力を失わない |
| 履歴の文字数 | 変換結果は`kana_length`中心 | 実際に送るASCII長（`{#Left}`を除外） | 意図した適応。ASCII HIDでBackspaceを一致させる |
| `reset()` | QMKの`mejiro_reset_state()`は履歴を保持 | Rustのテスト用セッションリセットは履歴も消去 | ファームウェアのモード遷移からは未使用。API差分として明記 |
| マクロ容量 | 1キーあたり512バイト | 1キーあたり128バイト。超過時は`Truncated`として記録し、部分的なマクロを再生しない | RMK側の固定バッファ設計による制限。長大マクロはQMKと同一ではない |
| `{#Left}`のマクロ記録 | QMKは展開済み文字列を保存 | RMKはトークンを保存し、リプレイ時にもカーソル移動を再現 | RMK側で論理操作を保持する設計差 |

上表の「意図した適応」以外は、QMKの変換結果・first-up確定・履歴更新・マクロ制御を
Rust側で再現しています。特に`#`のリピート、例外かなの優先順位、上一段／補助動詞の
推論、`tk`の促音持ち越しは回帰テストの対象で、辞書テーブルは固定スナップショットと
機械比較します。動的比較の現在の差分は上表の`っー` 1件だけです。

## 検証

```sh
RUST_MIN_STACK=67108864 cargo fmt --all -- --check
RUST_MIN_STACK=67108864 cargo test --workspace --target x86_64-unknown-linux-gnu --lib
cargo clippy --workspace --lib --target x86_64-unknown-linux-gnu -- -D warnings
cargo make --no-workspace qmk-dynamic-regression
cargo llvm-cov --workspace --target x86_64-unknown-linux-gnu --lib --fail-under-lines 80
cargo make uf2 --release
```

`nrf-sdc`のBindgenには`clang`と`libclang-dev`が必要です。CIはホストTDD、Clippy、
カバレッジ、`keyboard.toml`検証、QMK静的テーブル回帰、central/peripheralの
ファームウェアビルドを実行します。
