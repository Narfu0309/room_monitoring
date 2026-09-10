# 温室度通知Bot

SHT30を使用して温湿度を計測し、Discord Webhook APIへ送信するシステムです。

## 前提条件

### 依存関係

<https://github.com/esp-rs/esp-idf-template#prerequisites> に従って下さい。

### APIキー

`./webhook_url` にWebhookのURLを記載してください。

## ビルド

`cargo build --release`
> `--debug` は `--release` より圧縮率が低いだけです。

## Windows以外でビルドする

[target-dir](.cargo\config.toml#L4) をコメントアウトする。

## 実行

`cargo run --release`

## 書き込みが失敗する場合

[baudrate](espflash.toml#L3) を 115200 まで下げる。
