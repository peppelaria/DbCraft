# DbCraft

[![CI](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml/badge.svg)](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rmpaolillo/DbCraft?include_prereleases)](https://github.com/rmpaolillo/DbCraft/releases)

**一款用 Rust 编写的快速、原生、开源的桌面数据库客户端。**

DbCraft 将专业数据库图形工具熟悉的「一个窗口管理所有数据库」的工作方式带到一个轻量的原生应用中：
已保存的连接、对象树、在大表上依然流畅的电子表格式数据网格、SQL 编辑器、可视化表设计器以及导入/导出，
支持 **MySQL / MariaDB、PostgreSQL、SQL Server 和 SQLite**。

*[English](README.md) · [Italiano](README.it.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Українська](README.uk.md) · 中文 · [日本語](README.ja.md) · [한국어](README.ko.md)*

![DbCraft：MySQL 数据库的对象，以及菜单栏、工具栏和连接树](docs/screenshots/objects.png)

> **状态：早期开发阶段（v0.1）。** 所有四种引擎都在真实服务器上通过了同一套端到端测试：
> PostgreSQL 17、MySQL 8.4、MariaDB 11.4、SQL Server 2022 和 SQLite。
> 在编辑生产数据库之前，请先备份你的数据。

---

## 功能

| 领域 | 功能内容 |
|---|---|
| **连接** | MySQL、MariaDB、PostgreSQL、SQL Server、SQLite。已保存的连接、连接测试、**SSL/TLS**（使用 CA 和主机名验证服务器、客户端证书验证）、支持密码或私钥验证的 **SSH 隧道**（私钥可带或不带密码短语；主机密钥会与 `~/.ssh/known_hosts` 比对），以及通过 DbCraft 的 PHP 脚本连接仅能从 Web 服务器访问的数据库的 **HTTP 隧道**（[tunnel/README.md](tunnel/README.md)）。密码保存在操作系统的钥匙串中（macOS 钥匙串、Windows 凭据管理器、Linux 上的 Secret Service），绝不以明文保存。 |
| **对象树** | 连接 → 数据库 → 模式 → 表 / 视图 / 函数 / 过程 / 触发器，按需加载，带右键菜单。底部有：**搜索**（输入时即筛选树）、**★ 仅显示收藏** 和 **全部折叠**。 |
| **菜单栏** | 文件、编辑、查看、**收藏**、工具、窗口、帮助。任何对象都可以添加到收藏（右键单击 › *添加到收藏*）；收藏菜单会打开该对象，必要时先建立连接。 |
| **对象视图** | 所选数据库或模式中对象的可搜索列表，与 Navicat 中一样：名称、自动递增、修改日期、数据长度、引擎、行和注释（取决于服务器提供的列）。工具栏的 **表 / 视图 / 函数 / 其他 ▾** 用于切换对象类型：*其他* 列出触发器，并视服务器而定列出索引、序列、事件、类型、扩展和同义词。 |
| **已保存的查询** | 工具栏的 **查询** 列出为该数据库保存的查询（在查询选项卡中按 ⌘/Ctrl + S 即保存到这里）：打开、新建、删除。 |
| **用户** | 服务器的用户和角色及其权限；**新建用户**、**更改密码**、在数据库或模式上 **授予 / 撤销** 权限、**删除用户**。SQL 会在执行前显示（密码已隐藏）。 |
| **备份 / 还原** | 备份是一个 SQL 脚本，包含每个表的结构和数据（按外键顺序）以及视图；**备份** 选项卡会列出备份，可还原其中一个（或任意 `.sql` 文件）并删除备份。已在全部五种引擎上测试了完整的备份与还原流程。 |
| **自动运行** | 计划任务：备份或一段 SQL，每 *n* 分钟、每天或每周在指定时间运行，显示下次/上次运行时间和结果，并可 **立即运行**。任务在 DbCraft 打开期间运行。 |
| **模型** | 数据库或模式的 ER 图：每个表一个方框，显示其列（标记主键和外键），每个外键一条连线。可拖动方框、平移、缩放、**排列**、**导出 SQL**；双击表即可设计它。 |
| **表查看器 / 编辑器** | 虚拟化网格（只绘制可见单元格），服务器端分页（每页 100 – 50,000 行）。**筛选和排序** 面板：条件以可单击的词块组成的句子呈现（字段 · 运算符 · 值），每个条件通过各自的 *and*/*or* 与下一个相连，支持嵌套的括号组和 21 种运算符（=、!=、<…、包含、开头是/结尾是及其否定、是 NULL/是空的、介于、在列表中、自定义 SQL），也可直接编写 `WHERE`；另外支持多列排序。显示/隐藏列。**网格、表单和文本视图**。**单元格编辑器** 侧边面板，用于长文本、JSON（格式化显示）和二进制数据（十六进制转储）。在 date、time 和 datetime/timestamp 列上提供 **日期/时间选择器**（活动单元格上的日历按钮或 Alt+↓，在表单视图和单元格编辑器中也可用），保留小数秒和时区。就地编辑、设置为 NULL、添加和删除记录（删除前会要求确认）。自动提交会在你离开记录时保存它；**开始事务** 会保留所有更改，直到 **提交** 或 **回滚**。 |
| **数据网格** | 带类型和主键标记的列标题、带当前记录标记的行号栏、灰色显示的 NULL、调整列宽（双击自动适应）、冻结列、多单元格选择、键盘导航、以 TSV 格式复制（可带或不带标题）。 |
| **SQL 编辑器** | 语法高亮；**代码补全**（关键字、表、视图、函数和列，也支持别名 `u.` 之后；Ctrl+空格）；工具栏中的连接、数据库和模式选择器；*运行* 全部（⌘/Ctrl + Enter）、选中内容或光标处的语句（⌘/Ctrl + Shift + Enter）；**停止**（在 PostgreSQL 和 MySQL/MariaDB 上还会取消服务器上的查询）；**解释**（执行计划；SQL Server 上为 SHOWPLAN）；**美化 SQL**（⌘/Ctrl + Shift + F）；现成的 **代码段**；每条语句一个结果选项卡、消息日志、状态栏中的行/列，打开和保存 `.sql` 文件。 |
| **查询创建工具** | 可视化设计查询：将表拖到画布上，勾选列，将一列拖到另一个表的列上即可连接（INNER / LEFT / RIGHT / FULL，以曲线显示）；外键会自动连接。支持带自动 GROUP BY 的聚合、别名、使用与表筛选相同的词块编辑器设置条件和排序、DISTINCT 和 LIMIT（SQL Server 上为 TOP），以及实时 SQL 预览；**确定** 会把 SQL 放入编辑器。 |
| **表设计器** | 字段（名称、类型、不是 null、键、自动递增、默认、注释）、索引和外键，并针对连接的方言实时显示 `CREATE TABLE` / `ALTER TABLE` 语句的 **SQL 预览**。在 SQLite 上，无法用 `ALTER` 表达的更改会通过重建表来应用，并保留其数据。 |
| **导出** | 将表、视图或查询结果导出为 CSV、JSON、SQL `INSERT` 脚本或 Excel（`.xlsx`）。 |
| **导入** | CSV，可配置分隔符和标题行，提供字段映射预览，并可选择空单元格 → NULL。行会在事务中分批插入。 |
| **语言** | 界面支持 **English、Italiano、Español、Français、Deutsch、Українська、中文、日本語 和 한국어**：默认使用系统语言，也可在 **查看 › 语言** 中选择（立即生效）。中文、日文和韩文使用系统字体。 |
| **界面** | 浅色和深色主题。 |

| 表查看器 | SQL 编辑器（深色主题） |
|---|---|
| ![带筛选和排序面板的表查看器](docs/screenshots/table-viewer.png) | ![带代码补全的 SQL 编辑器，深色主题](docs/screenshots/query-editor-dark.png) |
| **模型（ER 图）** | **用户和权限** |
| ![数据库的 ER 图](docs/screenshots/model.png) | ![MySQL 服务器的用户及其权限](docs/screenshots/users.png) |
| **备份** | **自动运行** |
| ![数据库的备份](docs/screenshots/backup.png) | ![计划的备份和查询任务](docs/screenshots/automation.png) |

## 快速开始

### 下载

预构建的安装包位于 [Releases](https://github.com/rmpaolillo/DbCraft/releases) 页面：

- **macOS 11+（Apple 芯片）**：`DbCraft-<version>-macos-arm64.dmg`，然后将 DbCraft 拖到
  “应用程序”文件夹。没有 Apple Developer ID 的构建采用临时（ad hoc）签名：首次请通过
  右键单击 → **打开** 允许该应用，或在“系统设置” → “隐私与安全性”中点按 **仍要打开**。
- **Debian / Ubuntu**：`sudo apt install ./dbcraft_<version>_amd64.deb`
- **其他 Linux（x86_64，glibc 2.35+）**：`dbcraft-<version>-linux-x86_64.tar.gz`
- **Windows 10/11（x64）**：`dbcraft-<version>-windows-x86_64.zip`（已包含 Visual C++ 运行库）；
  SmartScreen 可能会要求确认。

#### 更新

已安装的版本会在启动时以及每 12 小时向 GitHub 检查是否有更新的版本。如果有，工具栏下方会出现一个提示栏，
提供 **立即更新**（下载、对照 `SHA256SUMS.txt` 进行 SHA-256 校验、安装以替换当前副本，然后 **立即重启**）、
**更新说明**（版本说明）和 **跳过此版本**。macOS 应用会被就地替换（旧版本保留在配置文件夹的
`updates/` 下），Linux 的 `.tar.gz` 文件夹会获得新文件，`.deb` 通过 `pkexec dpkg -i` 安装，
在 Windows 上则由一个辅助程序在应用关闭时替换文件。已保存的连接、设置和钥匙串中的密码不会受到影响。
工具栏按钮 **更新** 可手动检查更新，并开启或关闭更新提醒和自动安装。
从源码构建的版本不会自行更新（请使用 `git pull`）；如仍想试用提示栏，可使用
`DBCRAFT_UPDATE_TEST=1 DBCRAFT_PRETEND_VERSION=0.0.1` 运行。

### 系统要求

- [Rust](https://rustup.rs) **1.95+**（edition 2024）
- macOS、Windows 或 Linux。Linux 需要桌面窗口所需的常用开发包
  （X11/Wayland、`libxkbcommon`），以及钥匙串所需的 D-Bus。

### 构建和运行

```sh
git clone https://github.com/rmpaolillo/DbCraft.git
cd DbCraft
cargo run --release                     # 启动应用
cargo run --release -- path/to/file.db  # 直接打开 SQLite 文件（临时连接）
cargo run --release -- "postgres://user:password@localhost:5432/mydb"  # 或通过 URL 连接任意服务器
```

URL 协议：`postgres://`、`mysql://`、`mariadb://`、`sqlserver://`（或 `mssql://`）和
`sqlite://`。密码中的特殊字符需要进行百分号编码（例如 `@` → `%40`）。
添加 `?ssh=user:password@ssh-host:port` 可通过 SSH 隧道连接，或使用
`?ssh=user@ssh-host&ssh_key=~/.ssh/id_ed25519` 进行密钥验证。

要创建连接，请单击工具栏中的 **连接** 并选择数据库类型。

### 命令行选项

| 选项 | 说明 |
|---|---|
| `<file.db>` 或 `<url>` | 将 SQLite 文件或服务器 URL 作为临时的、不保存的连接打开 |
| `--open-table <name>` | 文件加载后打开其中的某个表 |
| `--query "<sql>"` | 打开一个包含此 SQL 的查询选项卡并运行 |
| `--dark` | 以深色主题启动 |
| `--lang <code>` | 本次运行的界面语言：`en`、`it`、`es`、`fr`、`de`、`uk`、`zh`、`ja`、`ko` |
| `--open <tool>` | 数据库加载后打开某个工具：`users`、`backup`、`automation`、`model`、`queries`、`others:<kind>` |
| `--new-connection <kind> [--ssh-tab]` | 打开新建连接对话框（`mysql`、`postgres`、…），可选择直接打开其 SSH 选项卡 |
| `--screenshot <out.png> [--delay <s>]` | 在 `s` 秒后（默认 3 秒）将窗口保存为 PNG，然后退出。用于自动化界面检查。 |

已保存的连接位于操作系统的配置目录中（`dbcraft/connections.json`），例如 macOS 上的
`~/Library/Application Support/dbcraft/`，与 `settings.json`、`favorites.json`、
`automation.json` 以及 `queries/` 和 `backups/` 文件夹放在一起。设置 `DBCRAFT_CONFIG_DIR`
可使用其他文件夹。

### 测试

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

针对真实服务器（Docker）的驱动集成测试：

```sh
scripts/test-certs.sh && mkdir -p target/ssh-test   # 测试证书、SSH 密钥文件夹
docker compose -f docker-compose.test.yml up -d --wait
DBCRAFT_TEST_SERVERS=1 cargo test -p db-drivers --test servers --test ssh --test tls --test http -- --test-threads=1
DBCRAFT_TEST_SERVERS=1 cargo test -p dbcraft -- --test-threads=1   # 备份/还原、用户和目录
docker compose -f docker-compose.test.yml down -v
```

该测试套件会创建带索引和外键的表，运行多语句脚本，检查元数据读取，应用并回滚事务性编辑，
通过设计器生成的 SQL 修改表，并解码每种引擎的数据类型（带时区的时间戳、UUID、JSON、数组、
货币、无符号 64 位整数、XML、…）。SSH 测试通过一个 OpenSSH 服务器容器进行，使用密码、RSA 密钥
以及受密码短语保护的 Ed25519 密钥；SSL 测试使用带测试证书的 PostgreSQL 和 MySQL（verify-full、
客户端证书、一个 `REQUIRE X509` 的 MySQL 用户）。SQL Server 镜像仅支持 amd64，在 Apple 芯片上
以模拟方式运行。

### 发布（维护者）

只有在你决定时才会发布版本：**Actions → Release → Run workflow**（分支 `main`）。
无需事先修改任何内容：版本号已准备就绪。

- 版本号由 `Cargo.toml` 中的 `MAJOR.MINOR`（`[workspace.package] version`）加上根据已有标签计算出的
  `PATCH` 组成：尚无 `v0.1.*` 标签 → `v0.1.0`；最后一个标签为 `v0.1.4` → `v0.1.5`。
- 要开始一个新系列（`0.2.0`、`1.0.0`…），请修改 `Cargo.toml` 中的版本并推送；下一个版本将从 `.0` 开始。
- 版本号只在构建过程中写入 `Cargo.toml`/`Cargo.lock`：不会有任何内容提交回仓库。

该工作流会进行测试并构建 macOS Apple 芯片版（`.dmg`）、Linux x86_64 版（`.deb` 和 `.tar.gz`）
以及 Windows x86_64 版（`.zip`），然后发布带有这些安装包和 `SHA256SUMS.txt` 的 `v<version>` 版本。
只保留最新的两个版本：更早的版本及其标签会被删除。

可选的 macOS 签名：如果设置了仓库密钥 `APPLE_CERTIFICATE`（带有 “Developer ID Application” 身份的
base64 编码 `.p12`）、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_ID`、`APPLE_PASSWORD`（App 专用密码）和
`APPLE_TEAM_ID`，应用和 DMG 将使用 Developer ID 签名并经过公证；如果没有设置，则采用临时（ad hoc）签名。

每次推送和拉取请求还会运行 **CI**（`.github/workflows/ci.yml`）：clippy、在 Linux、macOS 和 Windows
上的测试，以及针对真实 PostgreSQL、MySQL、MariaDB 和 SQL Server 服务容器的驱动测试套件。

## 架构

DbCraft 是一个包含四个 crate 的 Cargo 工作区：

```
crates/
├── dbgrid       虚拟化的 egui 数据网格（GridModel trait → DataGrid 组件 → GridEvent）
├── db-core      Value / ResultSet / 结构类型、异步 Driver trait、SQL 方言和 DDL 生成
├── db-drivers   SQLite、PostgreSQL、MySQL/MariaDB（sqlx）和 SQL Server（tiberius）
└── app          `dbcraft` 桌面应用（eframe）
```

- **翻译** 的工作方式与 OpenSlate 相同：代码中的意大利语文本即为键，
  `crates/app/src/i18n/<lang>.json` 提供其他语言的译文（`{n}` 或 `{}` 等占位符使运行时生成的文本也能被翻译）。
  使用 `DBCRAFT_I18N_MISSING=missing.txt` 运行，可收集显示时缺少翻译的文本。

- 界面从不阻塞：数据库操作在后台的 **tokio** 运行时中执行，结果通过通道返回并触发重绘。
- 网格对数据库一无所知。它通过小巧的 `GridModel` trait 读取数据，并以事件形式报告用户意图
  （`CellEdited`、`SortRequested`、`DeleteRows`、…），由应用决定如何处理。
- 各引擎之间不同的 SQL（标识符引用、字面量、分页、`INSERT`、`UPDATE`、`DELETE`、`CREATE`、`ALTER`）
  集中在一处，即 `db_core::Dialect`，并有单元测试覆盖。

## 路线图

- [x] 针对 PostgreSQL、MySQL/MariaDB 和 SQL Server 的集成测试（Docker）
- [x] SQL 自动补全（关键字、表、列）
- [x] 取消正在运行的查询
- [x] SSH 隧道（密码、可选密码短语的私钥）
- [x] SSL 选项（CA 验证、主机名验证、客户端证书）
- [x] HTTP 隧道（DbCraft PHP 脚本，MySQL/MariaDB 和 PostgreSQL）
- [x] 自定义矢量图标集
- [x] 九种语言的界面
- [x] ER 图、备份/还原、计划任务、用户和权限
- [ ] 数据和结构同步、BI
- [ ] 更多引擎（Oracle、MongoDB、Redis）

欢迎提交 issue 和拉取请求。

## 致谢

DbCraft 建立在众多开源项目的成果之上。谢谢！

- **[GridCraft](https://github.com/storytold/gridcraft)**：以代码绘制的 **图标集**（扩展了相同风格的
  数据库图标）以及 `crates/dbgrid` 中的数据网格均源自 GridCraft（网格部分：前缀和行索引、带冻结窗格的
  几何布局、调色板布局和列宽拖动模型，已针对 SQL 结果集进行调整）。
  Copyright © 2026 ArtCraft Team and the GridCraft contributors，MIT OR Apache-2.0。参见
  [`crates/dbgrid/NOTICE.md`](crates/dbgrid/NOTICE.md)。
- **[egui / eframe](https://github.com/emilk/egui)**，作者 Emil Ernerfeldt 及贡献者：
  即时模式 GUI 工具包。
- **[sqlx](https://github.com/launchbadge/sqlx)**：SQLite、PostgreSQL 和 MySQL 驱动。
- **[tiberius](https://github.com/prisma/tiberius)**：SQL Server（TDS）驱动。
- **[tokio](https://tokio.rs)**：异步运行时。
- **[reqwest](https://github.com/seanmonstar/reqwest)**：HTTP 隧道使用的 HTTP 客户端。
- **[russh](https://github.com/Eugeny/russh)**：隧道使用的 SSH 客户端。
- **[SQLite](https://sqlite.org)**（公有领域），通过 `libsqlite3-sys` 内置。
- **[syntect](https://github.com/trishume/syntect)**（通过 `egui_extras`）：SQL 语法高亮。
- **[rust_xlsxwriter](https://github.com/jmcnamara/rust_xlsxwriter)**、
  **[csv](https://github.com/BurntSushi/rust-csv)**、**[serde](https://serde.rs)**：导出和导入。
- **[keyring](https://github.com/hwchen/keyring-rs)**、**[rfd](https://github.com/PolyMeilex/rfd)**、
  **[dirs](https://github.com/soc/dirs-rs)**：操作系统钥匙串、原生文件对话框和配置路径。
- **[sys-locale](https://github.com/1Password/sys-locale)**：系统语言。
- **[Noto Sans CJK](https://github.com/notofonts/noto-cjk)**（SIL OFL 1.1）：一个很小的子集，用于菜单中的
  语言名称 中文 日本語 한국어，与 OpenSlate 中一样。
- **[OpenSlate](https://github.com/rmpaolillo/OpenSlate)**：翻译系统和自动更新。

第三方组件及其许可证的完整列表见 [CREDITS.md](CREDITS.md)。

## 许可证

可任选以下其一授权：

- Apache License, Version 2.0（[LICENSE-APACHE](LICENSE-APACHE)）
- MIT license（[LICENSE-MIT](LICENSE-MIT)）

由你自行选择。

除非你另有明确声明，你有意提交以纳入本作品的任何贡献（按 Apache-2.0 许可证的定义），
均按上述方式双重授权，不附加任何其他条款或条件。

## 免责声明

DbCraft 是一个独立项目。它与 PremiumSoft CyberTech Ltd. 没有任何关联，也未获得其认可或赞助。
“Navicat” 是其所有者的商标。提及它仅是为了说明 DbCraft 属于哪类工具，DbCraft 未使用其任何代码、
图稿或资源。MySQL、MariaDB、PostgreSQL、SQL Server 和 SQLite 是其各自所有者的商标。
