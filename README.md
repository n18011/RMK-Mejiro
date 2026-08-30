# RMK-Mejiro

[JEEBIS27/qmk_firmware の Mejiro31](https://github.com/JEEBIS27/qmk_firmware/tree/master/keyboards%2Fjeebis%2Fmejiro31)
を、[n18011/Cygnus-M-RMK の `rmk-migration` branch](https://github.com/n18011/Cygnus-M-RMK/tree/rmk-migration)
をターゲットとしてRMK 0.9.0へ移植したファームウェアです。

## 構成

- Seeed XIAO nRF52840を使うBLE split keyboard
- Central側のPMW3610トラックボール、Peripheral側のEC11エンコーダー
- Vial対応、10レイヤー（Mejiro/Geminiベース、QWERTY、数字、機能、ポインティング、Bluetooth）
- RMKがBLE操作に予約する`User0`〜`User7`を避け、`User8`〜`User31`をMejiro入力へ割り当て
  first-up chord処理とローマ字出力をRustで実装
- [mejiro-core](https://github.com/n18011/mejiro/tree/ae395bd9bd56a7010d79fd7e94c7494928e4e97c/mejiro-core)と
  [mejiro-rmk](https://github.com/n18011/mejiro/tree/ae395bd9bd56a7010d79fd7e94c7494928e4e97c/mejiro-rmk)を
  Git依存として利用

Mejiro crate は `Cargo.toml` で `n18011/mejiro` の commit
`ae395bd9bd56a7010d79fd7e94c7494928e4e97c` に固定しています。実装の重複を避けるため、
ローカルの `crates` 配下には Mejiro crate を保持していません。

ハードウェア配線とsplit設定は[target branch](https://github.com/n18011/Cygnus-M-RMK/tree/rmk-migration)
を維持し、Mejiro31のRP2040固有配線は持ち込みません。移植元のCソースは
[upstream/qmk](upstream/qmk)に監査用リファレンスとして保存しています。現行仕様は
[docs/specification.md](docs/specification.md)、QMKとの対応表は
[docs/porting.md](docs/porting.md)を参照してください。

## ビルド

必要なもの: Rust stable、`thumbv7em-none-eabihf`、`cargo-make`、`clang`、`libclang-dev`。

```sh
rustup target add thumbv7em-none-eabihf
cargo install --force cargo-make
cargo make uf2 --release
```

環境確認は次で行えます。`cargo-make`などがrustupのbinディレクトリにある場合は、先に
`PATH="$HOME/.cargo/bin:$PATH"`を設定してください。

```sh
cargo make --no-workspace check-dev
```

生成された`central`を右側、`peripheral`を左側へ書き込みます。現在の
`keyboard.toml`は、旧ファームウェアのVial設定やBLE bondを消すために
`storage.clear_storage = true`を意図的に有効にしています。これはRMKの仕様上、起動ごとに
保存領域を消去するため、古い設定を残さない検証用の動作です。`true`のままではbondが
再起動後に残りません。`usb-debug`ビルドでは保存peerの有無にかかわらず保存アドレスを
無視してCentralが毎回自動で探索を開始するため、USBログ検証では起動後に両側を接続するだけで構いません。通常ビルドの
明示的なpeer再探索は、Central側Bluetoothレイヤーの`User7`を5秒保持してpeer探索を
開始します。起動時のlayer 0はMejiroです。左親指または右親指の`LT(1,Space)`を保持すると
QWERTYへ、右親指内側の`LT(2,Enter)`を保持するとQWERTY shiftへ移ります。Bluetoothレイヤーへ
入る`LT(8,Escape)`はQWERTYレイヤーの左端に加え、Mejiroベースの右親指外側にもあります。
保存領域を完全に消去した初回の通常ビルドでも、右側の`LT(8,Escape)`を保持してBluetooth層へ入り、
隣の`User7`を5秒保持すればpeer探索を開始できます。暗号化リンクが成立した
peerだけが保存されます。Vialロック解除は
`keyboard.toml`の設定に従います。

## ローカル書き込み

片側だけを変更した場合は、両方をビルドする必要はありません。

```sh
# 右側/Centralだけを生成して、UF2ドライブが現れるまで待って自動コピー
cargo make --no-workspace flash-uf2-central

# 左側/Peripheral
cargo make --no-workspace flash-uf2-peripheral
```

スクリプトはUF2ブートローダーのドライブを検出します。実行後に基板をダブルリセット
してブートモードへ入れると、ドライブへの手動ドラッグ&ドロップは不要です。自動検出できない
環境では、Linuxなら未マウントの`XIAO-SENSE`などのUF2ボリュームを`udisksctl`で自動マウント
します。それでも検出できない環境では`UF2_MOUNT=/path/to/mount cargo make --no-workspace
flash-uf2-central`を使えます。

SWDデバッガーを接続できる場合は、`.cargo/config.toml`のrunnerを使って書き込みと起動を
一度に行えます。

```sh
cargo install probe-rs-tools --locked
probe-rs list
cargo make --no-workspace flash-swd-central
cargo make --no-workspace flash-swd-peripheral
```

これはUSB UF2ブートローダーではなく、probe-rs対応のデバッグプローブを使う方法です。
プローブは少なくともSWDIO、SWCLK、GND、VTrefを接続し、対象基板のnRF52840が
`probe-rs list`に表示される状態にします。現在のrunnerは`nRF52840_xxAA`を対象にしています。

## USBデバッグログ

通常ビルドは`defmt`を使ったSWD/RTTログですが、RMKの`usb_log`機能を使う別プロファイルでは
USB CDC-ACMログポートを追加できます。CentralはHID/Vialと同じ複合デバイス、Peripheralは
BLE split serviceを動かしながらCDC loggerだけを公開します。これはSWDプローブなしで、
UF2を書き込んだあとに`/dev/ttyACM*`（macOSでは`/dev/cu.usbmodem*`）から起動ログを確認
するためのものです。
RMKの制約上、`defmt`とUSBログは同じビルドに混在させず、タスク側で自動的に切り替えています。

```sh
# 端末A: USB CDCログを待ち受ける
cargo make --no-workspace monitor-usb

# 端末B: USB-debugプロファイルを生成してUF2ドライブへ自動書き込み
cargo make --no-workspace flash-uf2-usb-debug-central
cargo make --no-workspace flash-uf2-usb-debug-peripheral
```

ポートを固定する場合は`USB_DEBUG_PORT=/dev/ttyACM0 cargo make --no-workspace monitor-usb`を
使います。左右を同時に接続する場合は、番号ではなく`/dev/serial/by-id/`配下のリンクを
指定すると取り違えません。新しいPeripheral USB-debug版では、UF2ブートローダーの
`cdc_acm`列挙だけでなく、アプリ起動後にも`Cygnus-M`とは別のCDC loggerポートが現れます。
PeripheralのUSB serialは`vial:f64c2b3c:peripheral`、CentralはRMKが生成する右手基板固有の
serialです。Peripheralを書き換えた後に`ls -l /dev/serial/by-id/`を実行すると、左右を
別リンクとして選択できます。
SWDでUSB-debugプロファイルを書き込む場合は、`flash-swd-usb-debug-central`または
`flash-swd-usb-debug-peripheral`を使えます。通常の`flash-swd-central`／`peripheral`は
`defmt` RTTログをprobe-rsへ出すプロファイルです。

`usb-debug`ではRMKのUSB/BLE状態に加えて、Mejiroの受信イベントと確定した
`StrokeResult`も`DEBUG`ログへ出すため、実機で「どの仮想キー列が届き、何へ変換されたか」を
確認できます。アプリ側からArduinoの`Serial.print`相当の診断行を出す場合は、
`mejiro_rmk::usb_serial_println!("value = {:?}", value)`を使えます。このUSB loggerは
各レコードの末尾にCRLFを付けるため、出力は行単位です。通常ビルドでは同マクロはno-opになります。
Mejiroキーイベントでは、`mejiro action: user=N, pressed=true`のようにRMK標準のUserキー番号と
押下状態を確認できます。Vial上の物理配置と変換エンジンを分けて検証できます。

## 開発・検証

```sh
python3 tools/validate_keyboard.py
python3 tools/qmk_regression.py
python3 tools/qmk_dynamic_regression.py
cargo fmt --all -- --check
RUST_MIN_STACK=67108864 cargo test --workspace --target x86_64-unknown-linux-gnu --lib
RUST_MIN_STACK=67108864 cargo clippy --workspace --lib --target x86_64-unknown-linux-gnu -- -D warnings
RUST_MIN_STACK=67108864 cargo test --no-default-features --features usb-debug --workspace --target x86_64-unknown-linux-gnu --lib
RUST_MIN_STACK=67108864 cargo clippy --no-default-features --features usb-debug --workspace --lib --target x86_64-unknown-linux-gnu -- -D warnings
```

Mejiroの変換ロジックを外部リポジトリの clone から確認する場合は、次のコマンドを実行します。
`/path/to/mejiro` は `n18011/mejiro` の clone 先に置き換えてください。

```sh
RUST_MIN_STACK=67108864 cargo test --manifest-path /path/to/mejiro/mejiro-core/Cargo.toml --target x86_64-unknown-linux-gnu --lib
RUST_MIN_STACK=67108864 cargo test --manifest-path /path/to/mejiro/mejiro-rmk/Cargo.toml --target x86_64-unknown-linux-gnu --lib
```

CI の行カバレッジ検査は、同じ commit の2 crateを一時 workspace として実行します。

QMKの固定表だけでなく実行結果まで比較する場合は、`cc`が利用可能な環境で次を実行します。
約2.9万件の略語・動詞・かな・音節・助詞ストロークを一時プローブへ流し、QMKの`#`リピート
経路も含めて比較します。

```sh
cargo make --no-workspace qmk-dynamic-regression
```

SWDまたはUF2用のARMビルドでは、`nrf-sdc`のBindgenがlibclangを必要とします。Debian/Ubuntuでは
`clang-18`と`libclang-18-dev`を入れ、必要なら`LIBCLANG_PATH=/usr/lib/llvm-18/lib`を設定してください。

CIは上記のホストTDD検証に加えて、80%以上の行カバレッジ、central/peripheralのARMビルド、
UF2/HEX/ELF成果物の保存を行います。`v*`タグでは成果物をGitHub Releaseへ公開します。
`RUST_MIN_STACK`はRMKの大きなマクロ展開を安定してコンパイルするための設定です。
