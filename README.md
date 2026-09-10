# 温室度通知Bot

ESP32でSHT30を使用して温湿度を計測し、指定の時間にDiscord Webhook APIへ送信するシステムです。

## 前提条件

### 依存関係

<https://github.com/esp-rs/esp-idf-template#prerequisites> に従って下さい。

### APIキー

`./webhook_url` にWebhookのURLを記載してください。

> [!CAUTION]
> **現状平文で保存している状態です。[.gitignore](.gitignore) や `webhook_url`、ビルド成果物などを厳重に管理してください。**

### 回路

SHT30でのみ動作済みです。
初期値は次の通りです

- SDA=21番ピン
- SCL=22番ピン
- I2Cアドレス=0x44

設定は [main.rs](src/main.rs) にハードコーディングしてあります。

### 計測及び通知時刻

[main.rs](src/main.rs) の `TIME_OF_MEASUREMENT` を設定してください。

### ビルドの設定

#### Windows

デフォルトでは Windows 向けに設定されています。

ビルドの作業ディレクトリは `D:\tmp` となっています。必要があれば [target-dir](.cargo\config.toml#L4) を変更してください。

#### Windows以外

[target-dir](.cargo\config.toml#L4) をコメントアウトする。

もしくは任意の場所に変更する。

## ビルド

> [!NOTE]
> Windows以外ではデフォルトの設定でビルドできません。
> [こちら](#windows以外) を参照してください。

`cargo build --release`
> `--debug` は `--release` より圧縮率が低いだけです。

## 実行

`cargo run --release`

## 書き込みが失敗する場合

[baudrate](espflash.toml#L3) を 115200 まで下げる。
