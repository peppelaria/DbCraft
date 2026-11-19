<p align="center"><img src="packaging/icon/dbcraft.svg" width="128" height="128" alt="DbCraft"></p>

# DbCraft

[![CI](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml/badge.svg)](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rmpaolillo/DbCraft?include_prereleases)](https://github.com/rmpaolillo/DbCraft/releases)

**Rust で書かれた、高速・ネイティブ・オープンソースのデスクトップ データベースクライアント。**

DbCraft は、プロ向けデータベース GUI でおなじみの「すべてのデータベースを 1 つのウィンドウで」という
ワークフローを軽量なネイティブアプリで実現します。保存済み接続、オブジェクトツリー、大きなテーブルでも
滑らかに動くスプレッドシート風のデータグリッド、SQL エディター、ビジュアルなテーブルデザイナー、
インポート/エクスポートを、**MySQL / MariaDB、PostgreSQL、SQL Server、SQLite** に対応して提供します。

**AI アシスタント**も内蔵しています。普段の言葉で質問すると、データベースの構造とデータを読み、SQL を書いて説明し、クエリをエディターで開き、データの変更はあなたの「はい」の後にだけ行います。ローカルモデル（LM Studio、Ollama）にもクラウドサービス（OpenAI、Claude、Gemini…）にも対応しています。

*[English](README.md) · [Italiano](README.it.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Українська](README.uk.md) · [中文](README.zh.md) · 日本語 · [한국어](README.ko.md)*

![DbCraft: MySQL データベースのオブジェクト、メニューバー、ツールバー、接続ツリー](docs/screenshots/objects.png)

> **ステータス: 初期開発段階 (v0.2)。** 4 つのエンジンすべてが、実際のサーバーに対して同じエンドツーエンドの
> テストスイートに合格しています: PostgreSQL 17、MySQL 8.4、MariaDB 11.4、SQL Server 2022、SQLite。
> 本番データベースを編集する前に、データをバックアップしてください。

---

## 機能

| 分野 | 内容 |
|---|---|
| **接続** | MySQL、MariaDB、PostgreSQL、SQL Server、SQLite。保存済み接続、接続テスト、**SSL/TLS** (CA とホスト名によるサーバー検証、クライアント証明書認証)、パスワードまたは秘密鍵 (パスフレーズの有無を問わず。ホストキーは `~/.ssh/known_hosts` と照合) で認証する **SSH トンネル**、そして Web サーバーからしか到達できないデータベース向けに DbCraft の PHP スクリプトを経由する **HTTP トンネル** ([tunnel/README.md](tunnel/README.md))。パスワードは OS のキーチェーン (macOS キーチェーン、Windows 資格情報マネージャー、Linux の Secret Service) に保存され、平文で保存されることはありません。 |
| **オブジェクトツリー** | 接続 → データベース → スキーマ → テーブル / ビュー / 関数 / プロシージャ / トリガー。遅延読み込みで、コンテキストメニュー付き。下部には **検索** (入力に合わせてツリーを絞り込み)、**★ お気に入りのみ**、**すべて折りたたむ**。 |
| **メニューバー** | ファイル、編集、表示、**お気に入り**、ツール、ウィンドウ、ヘルプ。どのオブジェクトもお気に入りに追加でき (右クリック › *お気に入りに追加*)、お気に入りメニューから開けます (必要なら先に接続します)。 |
| **オブジェクト表示** | Navicat と同様の、選択したデータベースまたはスキーマ内のオブジェクトの検索可能な一覧: 名前、自動増分、更新日時、データサイズ、エンジン、行、コメント (サーバーが提供する列)。ツールバーの **テーブル / ビュー / 関数 / その他 ▾** でオブジェクトの種類を切り替えます: *その他* ではトリガーと、サーバーに応じてインデックス、シーケンス、イベント、型、拡張機能、シノニムを一覧表示します。 |
| **保存済みクエリ** | ツールバーの **クエリ** で、データベースに保存されたクエリを一覧表示します (クエリタブで ⌘/Ctrl + S を押すとここに保存): 開く、新規、削除。 |
| **ユーザー** | サーバーのユーザーとロール、およびその権限。**新規ユーザー**、**パスワードを変更**、データベースまたはスキーマに対する権限の **付与 / 取り消し**、**ユーザーを削除**。実行前に SQL が表示されます (パスワードは非表示)。 |
| **バックアップ / 復元** | バックアップは、すべてのテーブルの構造とデータ (外部キーの順序) およびビューを含む SQL スクリプトです。**バックアップ** タブで一覧表示、復元 (任意の `.sql` ファイルも可)、削除ができます。5 つのエンジンすべてで往復テスト済み。 |
| **自動化** | スケジュールタスク: バックアップまたは SQL を、*n* 分ごと、毎日、または毎週指定した時刻に実行。次回/前回の実行と結果を表示し、**今すぐ実行** も可能。タスクは DbCraft の起動中に実行されます。 |
| **モデル** | データベースまたはスキーマの ER 図: テーブルごとに列 (主キーと外部キーを表示) を含むボックス、外部キーごとに線。ボックスのドラッグ、パン、ズーム、**整列**、**SQL をエクスポート**。テーブルをダブルクリックするとデザインできます。 |
| **テーブルビューアー / エディター** | 仮想化グリッド (表示中のセルのみ描画)、サーバー側ページング (1 ページあたり 100 – 50,000 行)。**フィルターと並べ替え** パネル: 条件はクリック可能なトークンの文 (フィールド · 演算子 · 値) として表示され、それぞれ固有の *and*/*or* で次の条件とつながります。括弧によるネストしたグループと 21 種類の演算子 (=、!=、<…、次を含む、次で始まる/終わるとその否定、NULL である/空である、次の範囲内、次のリスト内、カスタム SQL)、または生の `WHERE` に対応。複数列での並べ替えも可能。列の表示/非表示。**グリッド表示、フォーム表示、テキスト表示**。長いテキスト、JSON (整形表示)、バイナリ (16 進ダンプ) 用の **セルエディター** サイドパネル。日付、時刻、datetime/timestamp 列では **日付/時刻ピッカー** (アクティブなセルのカレンダーボタンまたは Alt+↓。フォーム表示とセルエディターでも利用可能) を使え、小数秒とタイムゾーンを保持します。その場での編集、NULL の設定、レコードの追加と削除 (削除時は確認あり)。自動コミットではレコードから離れると保存されます。**トランザクション開始** を使うと、**Commit** または **Rollback** まですべての変更を保留します。 |
| **データグリッド** | 型と主キーのマーカー付きの列見出し、現在のレコードのマーカー付き行番号欄、灰色表示の NULL、列幅の変更 (ダブルクリックで自動調整)、列の固定、複数セルの選択、キーボード操作、TSV としてコピー (見出しの有無を選択可)。 |
| **SQL エディター** | シンタックスハイライト。**コード補完** (キーワード、テーブル、ビュー、関数、列。エイリアス `u.` の後でも可。Ctrl+Space)。ツールバーで接続、データベース、スキーマを選択。すべて *実行* (⌘/Ctrl + Enter)、選択範囲またはカーソル位置のステートメントを実行 (⌘/Ctrl + Shift + Enter)。**停止** (PostgreSQL と MySQL/MariaDB ではサーバー上のクエリもキャンセル)。**Explain** (実行計画。SQL Server では SHOWPLAN)。**SQL を整形** (⌘/Ctrl + Shift + F)。定型の **スニペット**。ステートメントごとに 1 つの結果タブ、メッセージログ、ステータスバーの行/列表示、`.sql` ファイルを開く/保存。 |
| **クエリビルダー** | ビジュアルなクエリ設計: テーブルをキャンバスにドラッグし、列にチェックを入れ、列を別のテーブルの列にドラッグして結合 (INNER / LEFT / RIGHT / FULL、曲線で表示)。外部キーは自動的に結合されます。自動 GROUP BY 付きの集計、エイリアス、テーブルフィルターと同じトークンエディターによる条件と並べ替え、DISTINCT と LIMIT (SQL Server では TOP)、リアルタイムの SQL プレビュー。**OK** で SQL をエディターに挿入します。 |
| **テーブルデザイナー** | フィールド (名前、型、NOT NULL、キー、自動増分、デフォルト、コメント)、インデックス、外部キー。接続先の方言に合わせた `CREATE TABLE` / `ALTER TABLE` ステートメントのリアルタイム **SQL プレビュー** 付き。SQLite では、`ALTER` で表現できない変更はデータを保持したままテーブルを再構築して適用します。 |
| **エクスポート** | テーブル、ビュー、クエリ結果を CSV、JSON、SQL `INSERT` スクリプト、Excel (`.xlsx`) に出力。 |
| **インポート** | 区切り文字と見出し行を設定できる CSV、フィールド対応付けのプレビュー、空のセル → NULL (任意)。行はトランザクション内でバッチ挿入されます。 |
| **AI チャット** | 右側のパネルに表示される AI アシスタント (ツールバーの **AI チャット**)。OpenSlate と同じもので、SQL の作成・説明・修正・最適化を行い、ツールを使ってデータベースを操作します。設定済みの接続を参照し、構造とデータを読み取り (読み取り専用のクエリ)、新しいエディタータブでクエリを開き、変更はチャットであなたが **はい** と答えた後にのみ行います。OpenAI 互換の任意のサービス (LM Studio、Ollama、OpenAI、Anthropic Claude、Google Gemini、DeepSeek、OpenRouter…) を順番に試し、失敗時は次へフォールバックします。会話の保存、Markdown と数式、自動圧縮付きのコンテキストメーター、そして **AI 設定** (エージェント、キーチェーン内のキー、テスト、モデル、コンテキスト)。 |
| **言語** | インターフェースは **English、Italiano、Español、Français、Deutsch、Українська、中文、日本語、한국어** に対応: デフォルトではシステムの言語、または **表示 › 言語** で選んだ言語 (即座に反映)。中国語、日本語、韓国語にはシステムのフォントを使用します。 |
| **UI** | ライトテーマとダークテーマ。 |

| テーブルビューアー | SQL エディター (ダークテーマ) |
|---|---|
| ![フィルターと並べ替えパネルを表示したテーブルビューアー](docs/screenshots/table-viewer.png) | ![コード補完を表示した SQL エディター、ダークテーマ](docs/screenshots/query-editor-dark.png) |
| **モデル (ER 図)** | **ユーザーと権限** |
| ![データベースの ER 図](docs/screenshots/model.png) | ![MySQL サーバーのユーザーとその権限](docs/screenshots/users.png) |
| **バックアップ** | **自動化** |
| ![データベースのバックアップ](docs/screenshots/backup.png) | ![スケジュールされたバックアップとクエリのタスク](docs/screenshots/automation.png) |
| **AI チャット** | **AI 設定** |
| ![AI チャット: アシスタントがテーブルを読み、クエリを実行してエディターで開いたところ](docs/screenshots/ai-chat.png) | ![AI 設定: 順番に試すエージェント、キー、コンテキスト、エージェントがデータベースでできること](docs/screenshots/ai-settings.png) |

## はじめに

### ダウンロード

ビルド済みパッケージは [Releases](https://github.com/rmpaolillo/DbCraft/releases) ページにあります:

- **macOS 11+ (Apple silicon)**: `DbCraft-<version>-macos-arm64.dmg` を開き、DbCraft をアプリケーションに
  ドラッグします。Apple Developer ID のないビルドはアドホック署名です: 一度だけ右クリック → **開く**、
  またはシステム設定 → プライバシーとセキュリティ → **このまま開く** でアプリを許可してください。
- **Debian / Ubuntu**: `sudo apt install ./dbcraft_<version>_amd64.deb`
- **その他の Linux (x86_64、glibc 2.35+)**: `dbcraft-<version>-linux-x86_64.tar.gz`
- **Windows 10/11 (x64)**: `dbcraft-<version>-windows-x86_64.zip` (Visual C++ ランタイム同梱)。
  SmartScreen が確認を求める場合があります。

#### アップデート

インストールしたリリースは、起動時と 12 時間ごとに GitHub で新しいリリースを確認します。新しいリリースが
あると、ツールバーの下のバーに **今すぐ更新** (ダウンロード、`SHA256SUMS.txt` による SHA-256 チェック、
このコピーと置き換えてインストール、その後 **今すぐ再起動**)、**新機能** (リリースノート)、
**このバージョンをスキップ** が表示されます。macOS アプリはその場で置き換えられ (以前のアプリは設定フォルダーの
`updates/` に保管)、Linux の `.tar.gz` フォルダーには新しいファイルが入り、`.deb` は `pkexec dpkg -i` で
インストールされ、Windows ではアプリの終了時にヘルパーがファイルを置き換えます。保存済み接続、設定、
キーチェーンのパスワードには手を加えません。ツールバーの **アップデート** ボタンで手動確認や、通知・自動
インストールのオン/オフを切り替えられます。
ソースからのビルドは自動更新されません (`git pull`)。それでもバーを試したい場合は
`DBCRAFT_UPDATE_TEST=1 DBCRAFT_PRETEND_VERSION=0.0.1` を付けて実行してください。

#### AI アシスタント

ツールバーの **AI チャット** (または ツール › AI チャット) で右側にチャットが開き、**ツール › AI 設定** で
その設定が開きます。どちらも [OpenSlate](https://github.com/rmpaolillo/OpenSlate) のページで、ロジックも
同じです: エージェントは **順番に** 試され、応答しない場合 (サーバー停止、キーなし、エラー) は次のエージェントが
応答します。エージェントは設定フォルダーの `ai.toml` に保存されます。キーがそこに平文で書かれることはありません:
`keychain:NAME` (設定ページで入力したキーは OS のキーチェーンに保存)、`env:NAME`、または
`dsh:NAME` (DeepSeek Harness)。

チャットは DbCraft で選択中のデータベースを把握しており、次のツールを持っています:

| ツール | 機能 |
|---|---|
| `db_connections`、`db_databases`、`db_tables`、`db_describe` | 設定済みの接続と、そのデータベースの構造 |
| `db_query` | **読み取り専用** の SQL (SELECT、WITH、SHOW、EXPLAIN…) を実行。最大 `max_rows` 行 |
| `db_open_query` | クエリを実行せずに、新しいエディタータブで開く |
| `db_execute` | データや構造を変更: まず変更を準備するだけで、正確な SQL が表示されます。その後、次のメッセージで **あなたが「はい」と答えた場合にのみ** 実行します (*AI 設定 › データベース* で変更が許可されている場合のみ) |

回答内のすべての SQL ブロックには **エディターで開く** と **コピー** があります。ローカルモデル (LM Studio、
Ollama) を使えば、何もコンピューターの外に出ません。クラウドサービスを使う場合、質問と AI が読み取ったデータは
そのサービスに送信されます。

### 必要環境

- [Rust](https://rustup.rs) **1.95+** (edition 2024)
- macOS、Windows、または Linux。Linux ではデスクトップウィンドウ用の一般的な開発パッケージ
  (X11/Wayland、`libxkbcommon`) と、キーチェーン用の D-Bus が必要です。

### ビルドと実行

```sh
git clone https://github.com/rmpaolillo/DbCraft.git
cd DbCraft
cargo run --release                     # アプリを起動
cargo run --release -- path/to/file.db  # SQLite ファイルを直接開く (一時的な接続)
cargo run --release -- "postgres://user:password@localhost:5432/mydb"  # または URL で任意のサーバーを開く
```

URL スキーム: `postgres://`、`mysql://`、`mariadb://`、`sqlserver://` (または `mssql://`)、
`sqlite://`。パスワード内の特殊文字はパーセントエンコードしてください (例: `@` → `%40`)。
SSH トンネルを経由するには `?ssh=user:password@ssh-host:port` を、鍵認証には
`?ssh=user@ssh-host&ssh_key=~/.ssh/id_ed25519` を追加します。

接続を作成するには、ツールバーの **接続** をクリックし、データベースの種類を選択します。

### コマンドラインオプション

| オプション | 説明 |
|---|---|
| `<file.db>` または `<url>` | SQLite ファイルまたはサーバー URL を、保存されない一時的な接続として開く |
| `--open-table <name>` | ファイルの読み込み後、そのテーブルを開く |
| `--query "<sql>"` | この SQL でクエリタブを開いて実行する |
| `--dark` | ダークテーマで起動する |
| `--lang <code>` | この実行でのインターフェース言語: `en`、`it`、`es`、`fr`、`de`、`uk`、`zh`、`ja`、`ko` |
| `--open <tool>` | データベースの読み込み後にツールを開く: `users`、`backup`、`automation`、`model`、`queries`、`chat`、`ai-settings`、`others:<kind>` |
| `--new-connection <kind> [--ssh-tab]` | 新規接続ダイアログ (`mysql`、`postgres`、…) を開く。必要に応じて SSH タブで開く |
| `--screenshot <out.png> [--delay <s>]` | `s` 秒後 (デフォルト 3) にウィンドウの PNG を保存して終了する。UI の自動チェックに使用。 |

保存済み接続は OS の設定ディレクトリ (`dbcraft/connections.json`) にあります。たとえば macOS では
`~/Library/Application Support/dbcraft/` で、`settings.json`、`favorites.json`、`automation.json`、
`queries/` フォルダー、`backups/` フォルダーと同じ場所です。別のフォルダーを使うには
`DBCRAFT_CONFIG_DIR` を設定してください。

### テスト

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

実際のサーバー (Docker) に対するドライバー統合テスト:

```sh
scripts/test-certs.sh && mkdir -p target/ssh-test   # テスト用証明書、SSH 鍵フォルダー
docker compose -f docker-compose.test.yml up -d --wait
DBCRAFT_TEST_SERVERS=1 cargo test -p db-drivers --test servers --test ssh --test tls --test http -- --test-threads=1
DBCRAFT_TEST_SERVERS=1 cargo test -p dbcraft -- --test-threads=1   # バックアップ/復元、ユーザー、カタログ
docker compose -f docker-compose.test.yml down -v
```

このスイートは、インデックスと外部キーを持つテーブルを作成し、複数ステートメントのスクリプトを実行し、
イントロスペクションを確認し、トランザクション内の編集を適用・ロールバックし、デザイナーの SQL で
テーブルを変更し、各エンジンのデータ型 (タイムゾーン付きタイムスタンプ、UUID、JSON、配列、money、
符号なし 64 ビット、XML、…) をデコードします。SSH テストは、パスワード、RSA 鍵、パスフレーズで保護された
Ed25519 鍵を使って OpenSSH サーバーコンテナーを経由します。SSL テストでは、テスト用証明書を使った
PostgreSQL と MySQL (verify-full、クライアント証明書、MySQL の `REQUIRE X509` ユーザー) を使用します。SQL Server のイメージは amd64 専用で、
Apple Silicon ではエミュレーションで動作します。

### リリース (メンテナー向け)

リリースは、あなたが決めたときにだけ公開されます: **Actions → Release → Run workflow** (ブランチ
`main`)。事前に編集するものはありません: バージョン番号は準備済みです。

- バージョンは `Cargo.toml` (`[workspace.package] version`) の `MAJOR.MINOR` に、既存のタグから数えた
  `PATCH` を加えたものです: `v0.1.*` タグがまだない → `v0.1.0`、最後のタグが `v0.1.4` → `v0.1.5`。
- 新しいシリーズ (`0.2.0`、`1.0.0`…) を始めるには、`Cargo.toml` のバージョンを変更して push します。
  次のリリースは `.0` から始まります。
- 番号はビルド内でのみ `Cargo.toml`/`Cargo.lock` に書き込まれます: リポジトリには何もコミットされません。

ワークフローは macOS Apple silicon (`.dmg`)、Linux x86_64 (`.deb` と `.tar.gz`)、Windows x86_64 (`.zip`)
をテスト・ビルドし、パッケージと `SHA256SUMS.txt` を含むリリース `v<version>` を公開します。保持されるのは
最新の 2 つのリリースのみで、古いリリースとそのタグは削除されます。

macOS の署名 (任意): リポジトリシークレット `APPLE_CERTIFICATE` ("Developer ID Application" ID を含む
base64 の `.p12`)、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_ID`、`APPLE_PASSWORD` (App 用パスワード)、
`APPLE_TEAM_ID` があれば、アプリと DMG は Developer ID で署名され、公証されます。これらがない場合は
アドホック署名になります。

すべての push とプルリクエストでは **CI** (`.github/workflows/ci.yml`) も実行されます: clippy、Linux・macOS・
Windows でのテスト、そして実際の PostgreSQL、MySQL、MariaDB、SQL Server のサービスコンテナーに対する
ドライバースイート。

## アーキテクチャ

DbCraft は 4 つのクレートからなる Cargo ワークスペースです:

```
crates/
├── dbgrid       仮想化された egui データグリッド (GridModel トレイト → DataGrid ウィジェット → GridEvent)
├── db-core      Value / ResultSet / スキーマ型、非同期 Driver トレイト、SQL 方言と DDL 生成
├── db-drivers   SQLite、PostgreSQL、MySQL/MariaDB (sqlx)、SQL Server (tiberius)
└── app          `dbcraft` デスクトップアプリケーション (eframe)
```

- **翻訳** は OpenSlate と同じ仕組みです: コード内のイタリア語テキストがキーで、
  `crates/app/src/i18n/<lang>.json` がそれを他の言語で提供します (`{n}` や `{}` などのプレースホルダーにより、
  実行時に組み立てられるテキストも翻訳されます)。`DBCRAFT_I18N_MISSING=missing.txt` を付けて実行すると、
  翻訳なしで表示されたテキストを収集できます。

- UI がブロックすることはありません: データベース処理はバックグラウンドの **tokio** ランタイムで実行され、
  結果はチャネル経由で戻って再描画をトリガーします。
- グリッドはデータベースについて何も知りません。小さな `GridModel` トレイトを通じて読み取り、ユーザーの
  意図 (`CellEdited`、`SortRequested`、`DeleteRows`、…) をイベントとして通知するので、どうするかはアプリが
  決めます。
- エンジンごとに異なる SQL (識別子のクオート、リテラル、ページング、`INSERT`、`UPDATE`、`DELETE`、
  `CREATE`、`ALTER`) は `db_core::Dialect` の 1 か所にまとめられ、単体テストされています。

## ロードマップ

- [x] PostgreSQL、MySQL/MariaDB、SQL Server に対する統合テスト (Docker)
- [x] SQL の自動補完 (キーワード、テーブル、列)
- [x] 実行中のクエリのキャンセル
- [x] SSH トンネル (パスワード、パスフレーズ付きも可能な秘密鍵)
- [x] SSL オプション (CA 検証、ホスト名検証、クライアント証明書)
- [x] HTTP トンネル (DbCraft の PHP スクリプト、MySQL/MariaDB と PostgreSQL)
- [x] 独自のベクターアイコンセット
- [x] 9 言語のインターフェース
- [x] ER 図、バックアップ/復元、スケジュールタスク、ユーザーと権限
- [x] クエリとデータベースのための AI チャット (OpenSlate と同様)
- [ ] データと構造の同期、BI
- [ ] その他のエンジン (Oracle、MongoDB、Redis)

Issue とプルリクエストを歓迎します。

## クレジット

DbCraft は多くのオープンソースプロジェクトの成果の上に成り立っています。ありがとうございます!

- **[GridCraft](https://github.com/storytold/gridcraft)**: コードで描画する **アイコンセット** (同じスタイルの
  データベースアイコンで拡張) と `crates/dbgrid` のデータグリッドは GridCraft から派生しています
  (グリッド: 累積和による行インデックス、固定ペイン付きのジオメトリ、パレットのレイアウト、列幅変更の
  ドラッグモデルを、SQL の結果セット向けに改変)。
  Copyright © 2026 ArtCraft Team and the GridCraft contributors, MIT OR Apache-2.0。
  [`crates/dbgrid/NOTICE.md`](crates/dbgrid/NOTICE.md) を参照してください。
- **[egui / eframe](https://github.com/emilk/egui)** (Emil Ernerfeldt とコントリビューター): イミディエイト
  モード GUI ツールキット。
- **[sqlx](https://github.com/launchbadge/sqlx)**: SQLite、PostgreSQL、MySQL ドライバー。
- **[tiberius](https://github.com/prisma/tiberius)**: SQL Server (TDS) ドライバー。
- **[tokio](https://tokio.rs)**: 非同期ランタイム。
- **[reqwest](https://github.com/seanmonstar/reqwest)**: HTTP トンネル用の HTTP クライアント。
- **[russh](https://github.com/Eugeny/russh)**: トンネル用の SSH クライアント。
- **[SQLite](https://sqlite.org)** (パブリックドメイン)、`libsqlite3-sys` 経由で同梱。
- **[syntect](https://github.com/trishume/syntect)** (`egui_extras` 経由): SQL のシンタックスハイライト。
- **[rust_xlsxwriter](https://github.com/jmcnamara/rust_xlsxwriter)**、
  **[csv](https://github.com/BurntSushi/rust-csv)**、**[serde](https://serde.rs)**: エクスポートとインポート。
- **[keyring](https://github.com/hwchen/keyring-rs)**、**[rfd](https://github.com/PolyMeilex/rfd)**、
  **[dirs](https://github.com/soc/dirs-rs)**: OS のキーチェーン、ネイティブのファイルダイアログ、設定パス。
- **[sys-locale](https://github.com/1Password/sys-locale)**: システムの言語。
- **[Noto Sans CJK](https://github.com/notofonts/noto-cjk)** (SIL OFL 1.1): OpenSlate と同様、メニュー内の
  言語名 中文 日本語 한국어 のためのごく小さなサブセット。
- **[OpenSlate](https://github.com/rmpaolillo/OpenSlate)**: 翻訳システム、自動アップデート、AI
  チャット (そのページ、プロバイダーチェーン、設定)。
- **[wry](https://github.com/tauri-apps/wry)**: ウィンドウ内の AI チャットの WebView。
- **[KaTeX](https://katex.org)**、**[marked](https://marked.js.org)**、**[DOMPurify](https://github.com/cure53/DOMPurify)**
  と **[Inter](https://rsms.me/inter/)** フォント。チャットページ用に同梱 (`crates/app/web/vendor` を参照)。

サードパーティ製コンポーネントとそのライセンスの完全な一覧は [CREDITS.md](CREDITS.md) にあります。

## ライセンス

次のいずれかのライセンスの下で提供されます

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

どちらを選ぶかはあなたの自由です。

あなたが明示的に別段の意思を示さない限り、Apache-2.0 ライセンスの定義に従い、本成果物に含めることを
意図してあなたが提出したコントリビューションは、追加の条項や条件なしに、上記のとおりデュアルライセンスされる
ものとします。

## 免責事項

DbCraft は独立したプロジェクトです。PremiumSoft CyberTech Ltd. とは提携しておらず、同社による承認や
後援も受けていません。「Navicat」はその所有者の商標です。DbCraft がどのような種類のツールであるかを説明する
目的でのみ言及しており、DbCraft はそのコード、アートワーク、アセットを一切使用していません。
MySQL、MariaDB、PostgreSQL、SQL Server、SQLite はそれぞれの所有者の商標です。
