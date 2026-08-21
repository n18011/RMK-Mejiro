# RMK-Mejiro 現行仕様書

最終確認日: 2026-08-21

この文書は、RMK-Mejiro の現在の実装を利用者・保守者向けに定義する仕様書です。
セッション状態の正本は [`crates/mejiro-core/src/mejiro.rs`](../crates/mejiro-core/src/mejiro.rs)、変換表と変換処理の正本は
[`crates/mejiro-core/src/mejiro_transform.rs`](../crates/mejiro-core/src/mejiro_transform.rs)、出力境界の正本は
[`crates/mejiro-core/src/mejiro_output.rs`](../crates/mejiro-core/src/mejiro_output.rs)、RMK と HID の接続の正本は
 [`crates/mejiro-rmk/src/lib.rs`](../crates/mejiro-rmk/src/lib.rs)、キーボードの物理構成と
レイヤーの正本は [`keyboard.toml`](../keyboard.toml) です。

QMK 版との対応理由・意図した差分は [`porting.md`](porting.md)、検証結果は
[`tdd-evidence.md`](tdd-evidence.md) に分離して記録します。

## 1. 目的と適用範囲

RMK-Mejiro は、Cygnus-M の Mejiro/Gemini 入力を first-up chord として解釈し、
ローマ字またはキー操作へ変換してホストへ送るファームウェアです。

対象は次の構成です。

- RMK 0.8.2
- Seeed XIAO nRF52840 を使った BLE split keyboard
- [`n18011/Cygnus-M-RMK`](https://github.com/n18011/Cygnus-M-RMK) の `rmk-migration` を
  ハードウェア基盤とする Cygnus-M
- Central 側の PMW3610 トラックボール、Peripheral 側の EC11 エンコーダー
- Vial 対応、レイヤー構成は `keyboard.toml` で宣言

Mejiro31 の RP2040 固有配線や QMK の実行環境は対象外です。QMK ソースは
`upstream/qmk` に比較用リファレンスとして保存されています。

## 2. 実行経路

```text
keyboard.toml の Kb0..Kb23
        ↓
RMK が解決したキーイベント
        ↓  MEJIRO_EVENT_CHANNEL
MejiroController（Central）
        ↓
MejiroSession → StrokeResult
        ↓
KEYBOARD_REPORT_CHANNEL → HID レポート
```

RMK のキーマップ処理は通常のキー処理に加えて、`Kb0`〜`Kb23` の押下・解放を
`MEJIRO_EVENT_CHANNEL` へ発行します。`MejiroController` はこのイベントだけを
Mejiro 入力として消費します。Peripheral には Mejiro コントローラを配置しません。

出力は RMK の `from_ascii` とキーコードを使った US-HID 境界で生成されます。したがって、
変換結果は日本語かなそのものではなく、ホスト側で解釈される ASCII ローマ字です。

BLE split は右側をCentral、左側をPeripheralとして動作します。ホスト側のBluetooth接続を
持つのは右側のCentralだけで、左側は右側へ入力を転送するsplit peerです。公開サービスUUID
だけでは通常ビルドのpeerを自動登録しません。通常ビルドでは未登録時に右側のBluetoothレイヤーで
`User9` を5秒保持して一度だけ探索を開始し、接続後に暗号化リンクが成立した場合だけアドレスを
永続化します。`usb-debug` ビルドでは、保存peerの有無にかかわらず保存アドレスを無視し、
検証用に探索を自動開始します。
登録済みpeerの directed advertising がタイムアウトしても、別の公開advertiserへ自動的に
フォールバックしません。QWERTYレイヤーの `LT(8,Escape)` は `(0,6,L)` の左手側にあります。
Mejiroベースには同じ `LT(8,Escape)` を `(3,11,R)` に複製し、右手だけでもBluetoothレイヤーへ
入れるようにしています。Bluetoothレイヤーへ入った後、隣の `(3,12,R)` にある `User9` を
5秒保持するとpeer探索を開始できます。古い
Vial設定やBLE bondを残さないため、現在も
`keyboard.toml` の `storage.clear_storage = true` を意図的に使っています。RMKでは起動ごとに
保存領域を消去するため、通常ビルドでBLE接続を検証する場合は起動ごとにpeer探索を開始してください。
`usb-debug` ではこの探索を自動化しています。
`clear_layout` は `false` のままにし、Vialレイアウトは
ファームウェアの初期値として別途検証します。

## 3. Mejiro キー仕様

### 3.1 物理キー番号

`Kb` 番号と論理ラベルは次のとおりです。ラベルの順序が chord ID の正規化順序になります。

| Kb | 左手ラベル | Kb | 右手ラベル |
| ---: | --- | ---: | --- |
| 0 | `#` | 12 | `S` |
| 1 | `S` | 13 | `T` |
| 2 | `T` | 14 | `K` |
| 3 | `K` | 15 | `N` |
| 4 | `N` | 16 | `Y` |
| 5 | `Y` | 17 | `I` |
| 6 | `I` | 18 | `A` |
| 7 | `A` | 19 | `U` |
| 8 | `U` | 20 | `n` |
| 9 | `n` | 21 | `t` |
| 10 | `t` | 22 | `k` |
| 11 | `k` | 23 | `*` |

同一 chord 内の重複押下はビット集合として一度だけ扱います。chord ID は左手を
固定順で連結し、左右のどちらかが存在する場合に `-` を挟み、右手を固定順で連結します。
例えば左手 `S` と `A`、右手 `T` は `SA-T`、右手だけの `A` は `-A`、左手だけの
`A` は `A-` になります。

### 3.2 first-up 確定

ファームウェアは `MejiroSession::new(true)` で動作します。

1. 新しいキーが押されると、現在の chord に追加します。
2. chord に含まれるキーのうち最初のキーが解放された時点で chord を確定します。
3. まだ押されているキーは、確定直後の次の chord の種として残します。
4. 解放イベントだけでは新しいキーが追加されないため、同じ chord を二重に確定しません。

このため、例えば `K` と `A` を押して `K` を先に離すと、その時点で `KA` が一度だけ
確定し、`A` の解放では出力しません。純粋ロジックの `MejiroSession::new(false)` は
比較・検証用に提供され、全キー解放時に確定します。

## 4. StrokeResult と出力契約

1回の chord 確定は、次のいずれかの結果になります。

| 結果 | 意味 | HID 出力 |
| --- | --- | --- |
| `Text` | ASCII テキストと内部かな長を返す | 各 ASCII 文字を tap |
| `Key` | 単一の RMK キー操作 | 指定キーを1回 tap |
| `RepeatKey` | 単一キー操作のリピート | 指定キーを2回 tap |
| `Repeat` | 履歴の直前テキストを再出力 | 直前テキストを1回送信 |
| `Undo` | 直前履歴を取り消す | 履歴長分の Backspace |
| `Noop` | 有効な制御だが出力不要 | 何もしない |
| `Unsupported` | RMK 側で表現できない入力 | 破棄 |
| `Truncated` | 固定容量を超えた変換・マクロ | 部分結果を送らず破棄 |

`Text` の容量は 128 bytes です。`kana_length` は変換結果のかな長を保持する内部メタデータ
であり、HID 送信および履歴の Backspace 数は実際に送信される ASCII 長を基準にします。

### 4.1 基本変換

子音は 16 種、母音は次の 8 種を使います。

```text
子音: "", S, T, K, N, ST, SK, TK, SN, TN, KN, TKN, STK, STN, SKN, STKN
母音: A, I, U, IA, AU, YA, YU, YAU
```

基本例:

| chord ID | 出力 |
| --- | --- |
| `A` | `a` |
| `KA` | `ka` |
| `KYA` | `kya` |
| `STKNU` | `vu` |
| `SKYI` | `wyi` |
| `TNYA` | `thi` |

例外かな、英語・小拗音系の二重母音、助詞、ユーザー略語、抽象略語、動詞活用は
QMK の変換表・変換順序を Rust の固定テーブルとして移植しています。`*` は主に
ユーザー略語・動詞形の指定に使われます。

### 4.2 変換の優先順位と状態

変換は概ね次の優先順位で評価します。

1. 全キー押下のキャンセル chord
2. 編集・移動・記号などの固定コマンド
3. `#` によるリピート（明示的な `#...*` ユーザー略語を除く）
4. ユーザー略語
5. 抽象略語
6. 動詞・補助動詞の活用
7. 通常の音・助詞変換

セッションは次の状態を chord 間で保持します。

- 直前の母音: 母音を省略した次の子音の補完に使用
- 促音保留: `tk` で終わる入力の `っ` を次の first-up chord の先頭へ持ち越し
- 直前助詞: 助詞の `のの` など QMK 互換の補正に使用

`A-` と `K-` は無印では基本音 `a` と `ka` を返します。動詞として解釈する場合は
`A-*`、`K-*` を使います。これは無印の基本母音と動詞特殊処理の衝突を避けるための
RMK 側の設計です。

## 5. コマンド仕様

### 5.1 編集・移動・特殊キー

代表的な固定コマンドは以下のとおりです。

| chord ID | 操作 |
| --- | --- |
| `-U` | Undo |
| `-AU` | Backspace |
| `-IU` | Delete |
| `-S` / `-A` / `-N` / `-Y` / `-K` | Escape / Left / Down / Up / Right |
| `-I` / `-T` | Home / End |
| `-An`, `-Nn`, `-Yn`, `-Kn`, `-In`, `-Tn` | Shift + Left/Down/Up/Right/Home/End |
| `-n` / `n-` / `n-n` | Enter / Space / Tab |
| `#n-n` / `#-nk` | Shift + Enter / Ctrl + Enter |
| `#-t` / `#-k` | Language1 / Language2 |
| `#-S` | Escape を2回 |

記号コマンドは ASCII 記号を返します。括弧・引用符などの一部は
`{#Left}` トークンを含むテキストとして返されます。

### 5.2 `#` リピート

`#` は、ユーザー略語として明示登録された `#...*` を除き、通常の chord の結果を
2回分にします。テキストは連結し、キー操作は `RepeatKey` として同じキーを2回送ります。

全キー押下のキャンセル chord
`STKNYIAUntk#-STKNYIAUntk*` は `Noop` です。

### 5.3 マクロ

マクロ名は `n`、`t`、`k`、`nt`、`nk`、`tk`、`ntk` の7種類です。

| 操作例 | 意味 |
| --- | --- |
| `#n-*` | `n` マクロの記録開始・停止（トグル） |
| `#-*` | 最後に開始したマクロの記録を停止 |
| `#n-` | `n` マクロを再生 |

記録中の `Text` と `Repeat` の結果をマクロへ追加します。制御 chord 自体は記録・
出力しません。各マクロは 128 bytes 固定です。容量を超えた場合は `Truncated` として
記録し、そのマクロを部分状態のまま再生しません。マクロ内の
`{#Left}` はトークンのまま保存し、再生時にも ASCII 送信と Left tap を再現します。

## 6. 履歴と取り消し

- 履歴は最大 20 エントリで、上限を超えると最古のエントリを破棄します。
- 空でない `Text` は1エントリとして保存します。
- 履歴長は Unicode かな長ではなく、`{#Left}` を除いた実送信 ASCII bytes 数です。
- `Repeat` は直前エントリを複製してから再送します。
- Backspace は通常、直前エントリの長さを1減らし、0になればエントリを削除します。
- 直前の操作が Space の場合、Backspace は空白マーカーだけを解除します。
- Delete は保留中の促音を解除しますが、通常の文字履歴は減らしません。
- Undo は直前エントリを削除し、その保存長の Backspace を送ります。履歴が空の場合は
  QMK 互換の既定値として2回送ります。
- `reset()` は押下状態、変換状態、履歴、マクロ記録状態を初期化します。保存済みマクロの
  値は保持します。現在のファームウェアのモード遷移から `reset()` は呼び出していません。

## 7. HID 境界と制約

- テキストは RMK の `from_ascii` で US-HID キーへ変換します。
- `{#Left}` は ASCII 文字列として送らず、直前までのテキスト、Left tap、残りのテキストに
  分解して送信します。
- 変換できない ASCII は HID レポートへ追加しません。
- JIS 配列固有の直接出力は提供しません。
- RMK 側に Gemini の raw HID パススルー境界がないため、`Unsupported` は無出力です。
- `Text` とマクロの固定容量は 128 bytes です。超過時は `Truncated` として部分出力を
  破棄します。QMK 版の1キー 512 bytes マクロとは異なります。

## 8. キーボード構成

`keyboard.toml` がレイヤー、matrix_map、split の寸法と `Kb0`〜`Kb23` の配置の正本です。
`tools/validate_keyboard.py` はこのファイルと `vial.json` を読み取り、レイヤー数・各行の幅・
`matrix_map` の重複・Mejiroキーの一意性とCygnus-M上の配置・split矩形の境界・Vialの物理配置と
カスタムキーコード順序を検証します。layer 0 の `base` がMejiro/Gemini入力を持ち、そこだけが24個の `Kb` キーを持ちます。通常キー・機能キー・
マウス・数字・Bluetooth操作と共存します。物理行列の `(0,5)` はCygnus-M基板に存在しない
意図的な空きスロットです。

レイヤーの順序は、RMKの起動時デフォルトがlayer 0であることを前提に固定しています。

| index | name | 役割 |
| ---: | --- | --- |
| 0 | `base` | Mejiro/Gemini。左右親指の`LT(1,Space)`でQWERTYへ入り、右外側の`LT(8,Escape)`でBluetoothへ入る |
| 1 | `qwerty` | 通常QWERTY。`LT(2,Enter)`でQWERTY shift、Bluetooth・数字操作もここから入る |
| 2 | `qwerty_shift` | QWERTY shift |
| 3–9 | `function`〜`number_shift` | 既存の機能・ポインティング・数字・Bluetooth補助レイヤー |

Mejiroレイヤーの見た目は、Vialの行順で次のとおりです。

```text
Kb0 Kb1 Kb2 Kb3 Kb4 Kb5       Kb12 Kb13 Kb14 Kb15 Kb16 Kb17
_   Kb6 Kb7 Kb8 Kb9 Kb10 Kb11 Kb18 Kb19 Kb20 Kb21 Kb22 Kb23 _
_   _   _   _   _   _   _       _   _   _   _   _   _   _
_   _   _   _   _   LT(1,Space) _   LT(1,Space) LT(2,Enter) LT(8,Escape) _
```

このうち右手の先頭キー `Kb18` は、物理matrixでは `(3,7,R)` に割り当てられています。
左手の先頭空きは `(1,6,L)` です。見た目の行番号とmatrix行番号を混同しないよう、実機ログの
`mejiro key: KbN -> LogicalName` と併用します。

## 9. CI/CD と検証条件

`.github/workflows/build.yml` は push、pull request、手動実行で動作します。

### Host ジョブ

- `python3 tools/validate_keyboard.py`
- `python3 tools/qmk_regression.py`
- `cargo fmt --all -- --check`
- `cargo test --workspace --target x86_64-unknown-linux-gnu --lib`
- `cargo test --no-default-features --features usb-debug --workspace --target x86_64-unknown-linux-gnu --lib`
- Clippy を `-D warnings` で実行
- `cargo llvm-cov` で行カバレッジ 80%以上を要求

### Firmware ジョブ

Host ジョブ成功後、`clang` と `libclang-dev` を導入し、
`thumbv7em-none-eabihf` 向けに Central / Peripheral をビルドします。各 ELF から
UF2 と HEX を生成し、ELF/UF2/HEX を GitHub Actions artifact として保存します。
`v*` タグでは同じ成果物を GitHub Release に公開します。

仕様変更時は、少なくとも次を更新・実行します。

1. 本仕様書の影響する契約を更新
2. `crates/mejiro-core/src/lib.rs` または `crates/mejiro-rmk/src/lib.rs` のホスト契約テストを追加・修正
3. キーボード設定を変更した場合は `tools/validate_keyboard.py` を実行
4. Host 検証と Firmware 検証を CI で完了

## 10. ローカル書き込みとデバッグ

`Makefile.toml` はログ経路の異なる2つのファームウェアプロファイルを持ちます。

| プロファイル | 書き込み | ログ | 用途 |
| --- | --- | --- | --- |
| `swd-debug` | `flash-swd-central` / `flash-swd-peripheral` または通常UF2 | probe-rs RTT / defmt | SWDプローブで起動ログを確認 |
| `usb-debug` | `flash-uf2-usb-debug-*` または `flash-swd-usb-debug-*` | USB CDC-ACM / log | SWDプローブなしでUSB起動ログを確認 |

RMKは`defmt`と`log`を同時に有効化できないため、`usb-debug`タスクは必ず
`--no-default-features --features usb-debug`でビルドします。CentralのCDCログはHID/Vial
複合デバイスの追加インターフェース、PeripheralのCDCログはBLE split serviceと並行する
専用loggerデバイスとして公開されます。`tools/monitor_usb.py`がLinux/macOSのCDCデバイス
再接続を待ち受けます。USBデバッグを使う前に`cargo make --no-workspace check-dev`を実行し、
ARMビルドに必要な`libclang`の有無も確認します。`usb-debug`時はMejiroの受信イベントと
`StrokeResult`も`DEBUG`ログへ出します。
Peripheral loggerのUSB serialは`vial:f64c2b3c:peripheral`、CentralはRMKが生成する
右手基板固有のserialです。

現在の検証実績と、ローカル環境で未導入だったゲートの記録は
[`tdd-evidence.md`](tdd-evidence.md) を参照してください。
