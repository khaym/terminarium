<p align="center">
  <img src="assets/logo.svg" width="480"
       alt="terminarium — 8bit ピクセルフォントの名前の横を左向きに泳ぐ大きな魚">
</p>

<p align="center">
  <em>コーディングエージェントが働くあいだ、ターミナルのペインで育っていく小さな海。</em>
</p>

<p align="center">
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-005f87" alt="License: MIT OR Apache-2.0"></a>
</p>

<p align="center">
  <a href="#インストール">インストール</a> &bull;
  <a href="#遊び方">遊び方</a> &bull;
  <a href="#ターミナル設定">ターミナル設定</a> &bull;
  <a href="#開発">開発</a> &bull;
  <a href="#更新履歴">更新履歴</a> &bull;
  <a href="#ライセンス">ライセンス</a>
</p>

<p align="center">
  <a href="README.md">English</a> &bull; <b>日本語</b>
</p>

<p align="center">
  <img src="assets/wallpaper-day.png" width="100%"
       alt="細いターミナルペインに広がる terminarium の壁紙層: 魚と昆布と沈んだ錨のある昼の海">
</p>

ターミナルのモザイクの片隅、ひとつのペインが静かな海になります。
作業中は何も要求しません。留守のあいだも育ち続けます。
少しずつ好みの風景に育て上げてください。

## インストール

特別なツールは不要です。ワンライナーで `~/.local/bin` に入ります:

```sh
curl -fsSL https://github.com/khaym/terminarium/releases/latest/download/terminarium-installer.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/khaym/terminarium/releases/latest/download/terminarium-installer.ps1 | iex"
```

インストーラはプラットフォーム（Linux glibc/musl・macOS・Windows）に合った
バイナリを GitHub Releases から選び、checksum を検証します。スクリプト自体も
ただの Release 資産なので、気になるなら実行前に中身を読めます。

そして:

```sh
terminarium
```

（ソースからビルドする場合: このリポジトリを `git clone` して
`cargo install --path .` — 実行方法は同じです。）

アンインストールも同じくらい小さく済みます: `~/.local/bin/terminarium` を
削除し、海を残さないなら `~/.local/share/terminarium/` も削除してください。

## 遊び方

ウィンドウサイズがインターフェースです。バイナリはひとつ、層はふたつ:

- **壁紙** — 80×20 未満のペイン: ただの海。数字も入力もなし。プロンプトの
  合間に眺めるためのものです。
- **ゲーム** — 80×20 以上: 海を豊かにしてください。岩礁を選び通貨で生き物を購入しましょう。

<p align="center">
  <img src="assets/game-layer.png" width="100%"
       alt="全幅ペインの同じ海: ゲーム層では通貨・スコア・生き物の価格・キー操作の HUD が加わる">
</p>

最初は広いペインで始めます: 岩礁を組み、`s` で海を始め、最初の藻を買ったら、
ペインを細くして仕事に戻る。海では上位生物が下位生物を捕食する食物連鎖が働いています。
余剰な生産物がデトリタスとして沈殿します。この海ではデトリタスが通貨です。

岩礁を組む（`s` まで）:

| キー | 動作 |
|---|---|
| `h` `l`（または `←` `→`） | 海底を移動する |
| `j` `k`（または `↑` `↓`） | 置く岩を選ぶ |
| `Enter` / `Backspace` | 岩を置く / 拾い上げる |
| `s` | 岩礁を確定して海を始める |

海が始まってから:

| キー | 動作 |
|---|---|
| `1`–`4` | 生き物を買う: 藻 → プランクトン → 小魚 → 大魚 |
| `a` | 沈んだ錨をつかむ——海が育つと手に入る置物（`h` `l` で移動、`Enter` で確定） |
| `n` のあと `y` | 新しい海を始める（prestige） |

`q`（または `Ctrl-C`）は、どちらの層でも、いつでも終了です。

ペインをまた広げると、働いているあいだに溜まったデトリタスがまとめて回収されます。
この瞬間がこのゲームの醍醐味です。何か買って（買わなくてもいい）、ペインを細くして仕事へ。

終了しても何も失いません: プロセスを閉じているあいだも生態系は生産を続け、
次の起動時に差分が精算されます（オフライン進行）。

## ターミナル設定

**tmux — RGB カラーを有効に。** 既定の tmux は色を 256 色に量子化します。
パレットはそれでも成立するように設計してありますが、水の色はフルカラーが
いちばん映えます。`~/.tmux.conf` に以下を追加してください。

```
set -ga terminal-features ',*:RGB'
```

**コンテナ — タイムゾーンの設定を。** パレットはシステム時計に従って
夜明け・昼・夕暮れ・夜と移ろいます。コンテナ（devcontainer・Codespaces）は
既定が UTC のため周期がずれます——23 時に昼の海が広がったりします。
コンテナ環境に `TZ`（例: `TZ=Asia/Tokyo`）を設定してください。

## 開発

```sh
cargo test   # 経済の不変条件テスト + レンダリングテスト
cargo run    # ソースから実行
```

経済は決定論的で、ルールはテストから読めます: 経済の不変条件は
`tests/invariants.rs` に、二層のレンダリングは `tests/render.rs` にあります。

## 更新履歴

- **v0.3.0** — 5 つ目「lantern」と 6 つ目「lagoon」を追加: 予算 5 の壁の先で
  発光の根が灯り（スコア 100,000 で解禁）、礁湖ではクラゲが脈動しながら縦に
  漂い、頭上をウミガメが泳ぎます（スコア 60,000 で解禁）——水槽初の縦の動きです。
- **v0.2.0** — 4 つ目の岩礁「grotto」を追加: 洞の口にエビが群れ、頭上をイカが
  滑空します。スコア 40,000 で解禁。
- **v0.1.0** — 初回リリース: 二層の海・経済・3 つの岩礁・鯨と錨。

## ライセンス

[MIT](LICENSE-MIT) または [Apache-2.0](LICENSE-APACHE) のいずれか、
お好きな方を選べます。
