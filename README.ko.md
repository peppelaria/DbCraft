<p align="center"><img src="packaging/social/social-preview.png" width="100%" alt="DbCraft"></p>

# DbCraft

[![CI](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml/badge.svg)](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rmpaolillo/DbCraft?include_prereleases)](https://github.com/rmpaolillo/DbCraft/releases)

**Rust로 작성된 빠르고 네이티브한 오픈 소스 데스크톱 데이터베이스 클라이언트.**

DbCraft는 전문 데이터베이스 GUI에서 익숙한 "하나의 창에서 모든 데이터베이스를" 워크플로를
가벼운 네이티브 앱으로 제공합니다. 저장된 연결, 개체 트리, 큰 테이블에서도 부드럽게 동작하는
스프레드시트 형태의 데이터 그리드, SQL 편집기, 시각적 테이블 디자이너, 가져오기/내보내기를
**MySQL / MariaDB, PostgreSQL, SQL Server, SQLite**에서 사용할 수 있습니다.

**AI 어시스턴트**도 내장되어 있습니다. 평소 말로 물어보면 데이터베이스의 구조와 데이터를 읽고, SQL을 작성하고 설명하며, 쿼리를 편집기에서 열고, 데이터 변경은 사용자가 '예'라고 한 뒤에만 합니다. 로컬 모델(LM Studio, Ollama)과 클라우드 서비스(OpenAI, Claude, Gemini…) 모두에서 작동합니다.

*[English](README.md) · [Italiano](README.it.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Українська](README.uk.md) · [中文](README.zh.md) · [日本語](README.ja.md) · 한국어*

![DbCraft: 메뉴 막대, 도구 모음, 연결 트리와 함께 표시된 MySQL 데이터베이스의 개체](docs/screenshots/objects.png)

> **상태: 초기 개발 단계 (v0.2).** 네 가지 엔진 모두 실제 서버를 대상으로 동일한 엔드투엔드
> 테스트 모음을 통과합니다: PostgreSQL 17, MySQL 8.4, MariaDB 11.4, SQL Server 2022, SQLite.
> 운영 데이터베이스를 편집하기 전에 데이터를 백업하세요.

---

## 기능

| 영역 | 제공 기능 |
|---|---|
| **연결** | MySQL, MariaDB, PostgreSQL, SQL Server, SQLite. 저장된 연결, 연결 테스트, **SSL/TLS** (CA 및 호스트 이름에 대한 서버 검증, 클라이언트 인증서 인증), 비밀번호 또는 개인 키 인증을 사용하는 **SSH 터널** (암호 유무와 관계없이 사용 가능, 호스트 키는 `~/.ssh/known_hosts`와 대조), 웹 서버에서만 접근할 수 있는 데이터베이스를 위해 DbCraft의 PHP 스크립트를 거치는 **HTTP 터널** ([tunnel/README.md](tunnel/README.md)). 비밀번호는 평문이 아닌 OS 키체인(macOS 키체인, Windows 자격 증명 관리자, Linux의 Secret Service)에 보관됩니다. |
| **개체 트리** | 연결 → 데이터베이스 → 스키마 → 테이블 / 뷰 / 함수 / 프로시저 / 트리거. 필요할 때 로드되며 컨텍스트 메뉴를 제공합니다. 하단에는 **검색**(입력하는 대로 트리를 필터링), **★ 즐겨찾기만 표시**, **모두 접기**가 있습니다. |
| **메뉴 막대** | 파일, 편집, 보기, **즐겨찾기**, 도구, 창, 도움말. 모든 개체를 즐겨찾기에 추가할 수 있으며(마우스 오른쪽 클릭 › *즐겨찾기에 추가*), 즐겨찾기 메뉴에서 열면 필요한 경우 먼저 연결합니다. |
| **개체 보기** | Navicat처럼 선택한 데이터베이스 또는 스키마의 개체를 검색 가능한 목록으로 표시합니다: 이름, 자동 증가, 수정한 날짜, 데이터 크기, 엔진, 행, 주석(서버가 제공하는 열). 도구 모음의 **테이블 / 뷰 / 함수 / 기타 ▾**로 개체 종류를 전환합니다. *기타*에는 트리거와, 서버에 따라 인덱스, 시퀀스, 이벤트, 유형, 확장, 동의어가 표시됩니다. |
| **저장된 쿼리** | 도구 모음의 **쿼리**는 데이터베이스에 저장된 쿼리를 나열합니다(쿼리 탭에서 ⌘/Ctrl + S로 여기에 저장): 열기, 새로 만들기, 삭제. |
| **사용자** | 서버의 사용자와 역할 및 그 권한. **새 사용자**, **비밀번호 변경**, 데이터베이스 또는 스키마에 대한 권한 **부여 / 취소**, **사용자 삭제**. 실행 전에 SQL이 표시됩니다(비밀번호는 숨김). |
| **백업 / 복원** | 백업은 모든 테이블의 구조와 데이터(외래 키 순서) 및 뷰를 담은 SQL 스크립트입니다. **백업** 탭에서 백업 목록을 보고, 백업(또는 임의의 `.sql` 파일)을 복원하고, 삭제할 수 있습니다. 다섯 가지 엔진 모두에서 백업과 복원의 왕복을 테스트했습니다. |
| **자동화** | 예약된 작업: 백업 또는 SQL을 *n*분마다, 매일 또는 매주 지정한 시간에 실행하며, 다음/마지막 실행과 결과를 표시하고 **지금 실행**을 제공합니다. 작업은 DbCraft가 열려 있는 동안 실행됩니다. |
| **모델** | 데이터베이스 또는 스키마의 ER 다이어그램: 테이블마다 열(기본 키와 외래 키 표시)이 있는 상자 하나, 외래 키마다 선 하나. 상자 끌기, 화면 이동, 확대/축소, **배치**, **SQL 내보내기**. 테이블을 두 번 클릭하면 디자인합니다. |
| **테이블 뷰어 / 편집기** | 가상화된 그리드(보이는 셀만 그림), 서버 측 페이지 나누기(페이지당 100 – 50,000행). **필터 및 정렬** 패널: 조건은 클릭 가능한 토큰(필드 · 연산자 · 값)으로 된 문장처럼 읽히고, 각 조건은 자체 *and*/*or*로 다음 조건과 연결되며, 괄호로 묶인 중첩 그룹과 21가지 연산자(=, !=, <…, 포함, 다음으로 시작/끝남 및 그 부정, NULL임/비어 있음, 범위 안에 있음, 목록에 있음, 사용자 지정 SQL)를 지원합니다. 또는 `WHERE`를 직접 작성할 수 있고, 여러 열 정렬도 가능합니다. 열 표시/숨기기. **그리드, 폼, 텍스트 보기**. 긴 텍스트, JSON(보기 좋게 서식 지정), 바이너리(16진 덤프)를 위한 **셀 편집기** 사이드 패널. 날짜, 시간, datetime/timestamp 열의 **날짜/시간 선택**(활성 셀의 달력 버튼 또는 Alt+↓, 폼 보기와 셀 편집기에서도 사용 가능)은 소수점 이하 초와 시간대를 유지합니다. 제자리 편집, NULL 설정, 레코드 추가 및 삭제(삭제 시 확인 요청). 자동 커밋은 레코드를 벗어날 때 저장하며, **트랜잭션 시작**은 **Commit** 또는 **Rollback**할 때까지 모든 변경 사항을 보류합니다. |
| **데이터 그리드** | 유형 및 기본 키 표시가 있는 열 머리글, 현재 레코드 표시가 있는 행 번호 영역, 회색으로 표시되는 NULL, 열 크기 조정(두 번 클릭하면 자동 맞춤), 고정 열, 다중 셀 선택, 키보드 탐색, TSV로 복사(머리글 포함 또는 제외). |
| **SQL 편집기** | 구문 강조. **코드 완성**(키워드, 테이블, 뷰, 함수, 열, 별칭 `u.` 뒤에서도 동작, Ctrl+Space). 도구 모음의 연결, 데이터베이스, 스키마 선택기. 전체(⌘/Ctrl + Enter), 선택 영역 또는 커서 위치의 문(⌘/Ctrl + Shift + Enter) *실행*. **중지**(PostgreSQL과 MySQL/MariaDB에서는 서버의 쿼리도 취소). **Explain**(실행 계획, SQL Server에서는 SHOWPLAN). **SQL 서식 지정**(⌘/Ctrl + Shift + F). 미리 만든 **스니펫**. 문마다 하나의 결과 탭, 메시지 로그, 상태 표시줄의 줄/열 표시, `.sql` 파일 열기 및 저장. |
| **쿼리 빌더** | 시각적 쿼리 디자인: 테이블을 캔버스로 끌어 놓고, 열을 체크하고, 열을 다른 테이블의 열로 끌어 조인합니다(INNER / LEFT / RIGHT / FULL, 곡선으로 표시). 외래 키는 자동으로 조인됩니다. 자동 GROUP BY가 적용되는 집계, 별칭, 테이블 필터와 같은 토큰 편집기로 작성하는 조건과 정렬, DISTINCT와 LIMIT(SQL Server에서는 TOP), 실시간 SQL 미리 보기. **확인**을 누르면 SQL이 편집기에 들어갑니다. |
| **테이블 디자이너** | 필드(이름, 유형, not-null, 키, 자동 증가, 기본값, 주석), 인덱스, 외래 키. 연결의 방언에 맞는 `CREATE TABLE` / `ALTER TABLE` 문을 실시간 **SQL 미리 보기**로 표시합니다. SQLite에서는 `ALTER`로 표현할 수 없는 변경을 데이터를 유지한 채 테이블을 재구성하여 적용합니다. |
| **내보내기** | 테이블, 뷰, 쿼리 결과를 CSV, JSON, SQL `INSERT` 스크립트 또는 Excel(`.xlsx`)로 내보냅니다. |
| **가져오기** | 구분자와 머리글 행을 설정할 수 있는 CSV, 필드 매핑 미리 보기, 선택적인 빈 셀 → NULL 변환. 행은 트랜잭션 안에서 일괄 삽입됩니다. |
| **AI 채팅** | 오른쪽 패널의 AI 어시스턴트(도구 모음의 **AI 채팅**)로, OpenSlate의 것과 같습니다: SQL을 작성, 설명, 수정, 최적화하고 도구를 사용해 데이터베이스에서 작업합니다. 설정된 연결을 보고, 구조와 데이터를 읽고(읽기 전용 쿼리), 새 편집기 탭에서 쿼리를 열며, 변경은 채팅에서 **예**라고 답한 뒤에만 수행합니다. OpenAI 호환 서비스(LM Studio, Ollama, OpenAI, Anthropic Claude, Google Gemini, DeepSeek, OpenRouter…)라면 무엇이든 사용할 수 있으며, 순서대로 시도하고 실패하면 다음으로 넘어갑니다. 대화 저장, Markdown과 수식, 자동 압축 기능이 있는 컨텍스트 미터, **AI 설정**(에이전트, 키체인의 키, 테스트, 모델, 컨텍스트)을 제공합니다. |
| **언어** | 인터페이스는 **English, Italiano, Español, Français, Deutsch, Українська, 中文, 日本語, 한국어**를 지원합니다. 기본값은 시스템 언어이며, **보기 › 언어**에서 선택할 수 있습니다(즉시 적용). 중국어, 일본어, 한국어는 시스템 글꼴을 사용합니다. |
| **UI** | 밝은 테마와 어두운 테마. |

| 테이블 뷰어 | SQL 편집기 (어두운 테마) |
|---|---|
| ![필터 및 정렬 패널이 열린 테이블 뷰어](docs/screenshots/table-viewer.png) | ![코드 완성이 표시된 SQL 편집기, 어두운 테마](docs/screenshots/query-editor-dark.png) |
| **모델 (ER 다이어그램)** | **사용자 및 권한** |
| ![데이터베이스의 ER 다이어그램](docs/screenshots/model.png) | ![MySQL 서버의 사용자와 권한](docs/screenshots/users.png) |
| **백업** | **자동화** |
| ![데이터베이스의 백업](docs/screenshots/backup.png) | ![예약된 백업 및 쿼리 작업](docs/screenshots/automation.png) |
| **AI 채팅** | **AI 설정** |
| ![AI 채팅: 어시스턴트가 테이블을 읽고 쿼리를 실행한 뒤 편집기에서 열었습니다](docs/screenshots/ai-chat.png) | ![AI 설정: 순서대로 시도하는 에이전트, 키, 컨텍스트, 에이전트가 데이터베이스에서 할 수 있는 일](docs/screenshots/ai-settings.png) |

## 시작하기

### 다운로드

미리 빌드된 패키지는 [Releases](https://github.com/rmpaolillo/DbCraft/releases) 페이지에 있습니다:

- **macOS 11+ (Apple silicon)**: `DbCraft-<version>-macos-arm64.dmg`를 열고 DbCraft를
  응용 프로그램으로 끌어 놓으세요. Apple Developer ID 없이 빌드된 앱은 ad hoc 서명되어 있습니다:
  마우스 오른쪽 클릭 → **열기**, 또는 시스템 설정 → 개인정보 보호 및 보안 → **그래도 열기**로 한 번 허용하세요.
- **Debian / Ubuntu**: `sudo apt install ./dbcraft_<version>_amd64.deb`
- **기타 Linux (x86_64, glibc 2.35+)**: `dbcraft-<version>-linux-x86_64.tar.gz`
- **Windows 10/11 (x64)**: `dbcraft-<version>-windows-x86_64.zip` (Visual C++ 런타임
  포함). SmartScreen이 확인을 요청할 수 있습니다.

#### 업데이트

설치된 릴리스는 시작할 때와 12시간마다 GitHub에서 새 릴리스를 확인합니다. 새 릴리스가
있으면 도구 모음 아래의 막대에 **지금 업데이트**(다운로드, `SHA256SUMS.txt`와 SHA-256 대조,
현재 사본 대신 설치, 그 후 **지금 다시 시작**), **새로운 기능**(릴리스 노트),
**이 버전 건너뛰기**가 표시됩니다. macOS 앱은 제자리에서 교체되고(이전 앱은 설정 폴더의
`updates/` 아래에 보관), Linux `.tar.gz` 폴더에는 새 파일이 들어가며, `.deb`는
`pkexec dpkg -i`로 설치되고, Windows에서는 앱이 종료될 때 도우미가 파일을 교체합니다.
저장된 연결, 설정, 키체인 비밀번호는 변경되지 않습니다. 도구 모음의 **업데이트** 버튼으로
직접 확인할 수 있으며, 알림과 자동 설치를 켜고 끌 수 있습니다.
소스에서 빌드한 경우 자동으로 업데이트되지 않습니다(`git pull`). 그래도 막대를 시험해 보려면
`DBCRAFT_UPDATE_TEST=1 DBCRAFT_PRETEND_VERSION=0.0.1`로 실행하세요.

#### AI 어시스턴트

도구 모음의 **AI 채팅**(또는 도구 › AI 채팅)은 오른쪽에 채팅을 열고, **도구 › AI 설정**은
설정을 엽니다. 둘 다 [OpenSlate](https://github.com/rmpaolillo/OpenSlate)의 페이지이며 동작 방식도
같습니다: 에이전트는 **순서대로** 시도되며, 하나가 응답하지 않으면(서버 꺼짐, 키 없음, 오류)
다음 에이전트가 응답합니다. 에이전트는 설정 폴더의 `ai.toml`에 저장되며, 키는 절대 평문으로
기록되지 않습니다: `keychain:NAME`(설정 페이지에서 입력한 키는 OS 키체인에 저장), `env:NAME` 또는
`dsh:NAME`(DeepSeek Harness).

채팅은 DbCraft에서 선택된 데이터베이스를 알고 있으며, 다음 도구를 사용합니다:

| 도구 | 하는 일 |
|---|---|
| `db_connections`, `db_databases`, `db_tables`, `db_describe` | 설정된 연결과 그 데이터베이스의 구조 |
| `db_query` | **읽기 전용** SQL(SELECT, WITH, SHOW, EXPLAIN…)을 실행, 최대 `max_rows`행 |
| `db_open_query` | 쿼리를 실행하지 않고 새 편집기 탭에서 열기 |
| `db_execute` | 데이터나 구조를 변경: 먼저 변경을 준비만 하여 정확한 SQL을 보여 주고, 다음 메시지에서 **예**라고 답한 뒤에만 실행합니다(그리고 *AI 설정 › 데이터베이스*에서 변경을 허용한 경우에만) |

답변의 모든 SQL 블록에는 **편집기에서 열기**와 **복사**가 있습니다. 로컬 모델(LM Studio,
Ollama)을 사용하면 아무것도 컴퓨터 밖으로 나가지 않으며, 클라우드 서비스를 사용하면 질문과
AI가 읽은 데이터가 해당 서비스로 전송됩니다.

### 요구 사항

- [Rust](https://rustup.rs) **1.95+** (edition 2024)
- macOS, Windows 또는 Linux. Linux에서는 데스크톱 창을 위한 일반적인 개발 패키지
  (X11/Wayland, `libxkbcommon`)와 키체인을 위한 D-Bus가 필요합니다.

### 빌드 및 실행

```sh
git clone https://github.com/rmpaolillo/DbCraft.git
cd DbCraft
cargo run --release                     # 앱 시작
cargo run --release -- path/to/file.db  # SQLite 파일을 바로 열기 (임시 연결)
cargo run --release -- "postgres://user:password@localhost:5432/mydb"  # 또는 URL로 임의의 서버 열기
```

URL 스킴: `postgres://`, `mysql://`, `mariadb://`, `sqlserver://`(또는 `mssql://`),
`sqlite://`. 비밀번호의 특수 문자는 퍼센트 인코딩하세요(예: `@` → `%40`).
SSH 터널을 거치려면 `?ssh=user:password@ssh-host:port`를, 키 인증에는
`?ssh=user@ssh-host&ssh_key=~/.ssh/id_ed25519`를 추가하세요.

연결을 만들려면 도구 모음에서 **연결**을 클릭하고 데이터베이스 유형을 선택하세요.

### 명령줄 옵션

| 옵션 | 설명 |
|---|---|
| `<file.db>` 또는 `<url>` | SQLite 파일 또는 서버 URL을 저장되지 않는 임시 연결로 열기 |
| `--open-table <name>` | 파일이 로드되면 해당 파일의 테이블 열기 |
| `--query "<sql>"` | 이 SQL로 쿼리 탭을 열고 실행 |
| `--dark` | 어두운 테마로 시작 |
| `--lang <code>` | 이번 실행의 인터페이스 언어: `en`, `it`, `es`, `fr`, `de`, `uk`, `zh`, `ja`, `ko` |
| `--open <tool>` | 데이터베이스가 로드되면 도구 열기: `users`, `backup`, `automation`, `model`, `queries`, `chat`, `ai-settings`, `others:<kind>` |
| `--new-connection <kind> [--ssh-tab]` | 새 연결 대화 상자 열기(`mysql`, `postgres`, …), 선택적으로 SSH 탭에서 열기 |
| `--screenshot <out.png> [--delay <s>]` | `s`초 후(기본값 3) 창의 PNG를 저장하고 종료. 자동화된 UI 검사에 사용됩니다. |

저장된 연결은 OS 설정 디렉터리(`dbcraft/connections.json`)에 있습니다. 예를 들어 macOS에서는
`~/Library/Application Support/dbcraft/`이며, `settings.json`, `favorites.json`,
`automation.json`, `queries/` 및 `backups/` 폴더와 함께 있습니다. 다른 폴더를 사용하려면
`DBCRAFT_CONFIG_DIR`을 설정하세요.

### 테스트

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

실제 서버(Docker)를 대상으로 하는 드라이버 통합 테스트:

```sh
scripts/test-certs.sh && mkdir -p target/ssh-test   # 테스트 인증서, SSH 키 폴더
docker compose -f docker-compose.test.yml up -d --wait
DBCRAFT_TEST_SERVERS=1 cargo test -p db-drivers --test servers --test ssh --test tls --test http -- --test-threads=1
DBCRAFT_TEST_SERVERS=1 cargo test -p dbcraft -- --test-threads=1   # 백업/복원, 사용자 및 카탈로그
docker compose -f docker-compose.test.yml down -v
```

테스트 모음은 인덱스와 외래 키가 있는 테이블을 만들고, 여러 문으로 된 스크립트를 실행하고,
인트로스펙션을 확인하고, 트랜잭션 편집을 적용 및 롤백하고, 디자이너의 SQL로 테이블을 변경하며,
각 엔진의 데이터 유형(시간대가 있는 타임스탬프, UUID, JSON, 배열, money, 부호 없는 64비트,
XML, …)을 디코딩합니다. SSH 테스트는 비밀번호, RSA 키, 암호로 보호된 Ed25519 키를 사용해
OpenSSH 서버 컨테이너를 거칩니다. SSL 테스트는 테스트 인증서와 함께 PostgreSQL과 MySQL을
사용합니다(verify-full, 클라이언트 인증서, MySQL `REQUIRE X509` 사용자). SQL Server 이미지는 amd64 전용이며
Apple Silicon에서는 에뮬레이션으로 실행됩니다.

### 릴리스 (관리자용)

릴리스는 직접 결정할 때만 게시됩니다: **Actions → Release → Run workflow** (브랜치
`main`). 미리 수정할 것은 없습니다: 버전 번호는 준비되어 있습니다.

- 버전은 `Cargo.toml`의 `MAJOR.MINOR`(`[workspace.package] version`)에 기존 태그에서
  계산한 `PATCH`를 더한 것입니다: `v0.1.*` 태그가 아직 없으면 → `v0.1.0`, 마지막 태그가 `v0.1.4`이면 → `v0.1.5`.
- 새 시리즈(`0.2.0`, `1.0.0`…)를 시작하려면 `Cargo.toml`의 버전을 변경하고 푸시하세요.
  다음 릴리스는 `.0`부터 시작합니다.
- 번호는 빌드 안에서만 `Cargo.toml`/`Cargo.lock`에 기록됩니다: 저장소에는 아무것도
  커밋되지 않습니다.

워크플로는 macOS Apple silicon(`.dmg`), Linux x86_64(`.deb` 및 `.tar.gz`),
Windows x86_64(`.zip`)를 테스트하고 빌드한 뒤, 패키지와 `SHA256SUMS.txt`를 포함한 릴리스
`v<version>`을 게시합니다. 가장 최신 릴리스 두 개만 유지되며, 이전 릴리스와 해당 태그는
삭제됩니다.

선택 사항인 macOS 서명: 저장소 시크릿 `APPLE_CERTIFICATE`("Developer ID Application" ID가
들어 있는 base64 `.p12`), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`
(앱 전용 비밀번호), `APPLE_TEAM_ID`가 있으면 앱과 DMG가 Developer ID로 서명되고
공증됩니다. 없으면 ad hoc 서명됩니다.

모든 푸시와 풀 리퀘스트는 **CI**(`.github/workflows/ci.yml`)도 실행합니다: clippy, Linux,
macOS, Windows에서의 테스트, 그리고 실제 PostgreSQL, MySQL, MariaDB, SQL Server 서비스
컨테이너를 대상으로 하는 드라이버 테스트 모음.

## 아키텍처

DbCraft는 네 개의 크레이트로 구성된 Cargo 워크스페이스입니다:

```
crates/
├── dbgrid       가상화된 egui 데이터 그리드 (GridModel 트레이트 → DataGrid 위젯 → GridEvent)
├── db-core      Value / ResultSet / 스키마 타입, 비동기 Driver 트레이트, SQL 방언 및 DDL 생성
├── db-drivers   SQLite, PostgreSQL, MySQL/MariaDB (sqlx) 및 SQL Server (tiberius)
└── app          `dbcraft` 데스크톱 애플리케이션 (eframe)
```

- **번역**은 OpenSlate와 같은 방식으로 동작합니다: 코드의 이탈리아어 텍스트가 키이며,
  `crates/app/src/i18n/<lang>.json`이 다른 언어의 텍스트를 제공합니다(`{n}`이나 `{}` 같은
  자리 표시자로 실행 시 만들어지는 텍스트도 번역됩니다). 번역 없이 표시된 텍스트를 수집하려면
  `DBCRAFT_I18N_MISSING=missing.txt`로 실행하세요.

- UI는 절대 멈추지 않습니다: 데이터베이스 작업은 백그라운드 **tokio** 런타임에서 실행되고,
  결과는 채널을 통해 돌아와 다시 그리기를 트리거합니다.
- 그리드는 데이터베이스에 대해 아무것도 알지 못합니다. 작은 `GridModel` 트레이트를 통해 데이터를
  읽고 사용자 의도(`CellEdited`, `SortRequested`, `DeleteRows`, …)를 이벤트로 보고하므로,
  무엇을 할지는 앱이 결정합니다.
- 엔진마다 다른 SQL(식별자 인용, 리터럴, 페이지 나누기, `INSERT`, `UPDATE`,
  `DELETE`, `CREATE`, `ALTER`)은 한곳, `db_core::Dialect`에 있으며 단위 테스트됩니다.

## 로드맵

- [x] PostgreSQL, MySQL/MariaDB, SQL Server 대상 통합 테스트 (Docker)
- [x] SQL 자동 완성 (키워드, 테이블, 열)
- [x] 실행 중인 쿼리 취소
- [x] SSH 터널 (비밀번호, 선택적 암호가 있는 개인 키)
- [x] SSL 옵션 (CA 검증, 호스트 이름 검증, 클라이언트 인증서)
- [x] HTTP 터널 (DbCraft PHP 스크립트, MySQL/MariaDB 및 PostgreSQL)
- [x] 자체 벡터 아이콘 세트
- [x] 9개 언어로 된 인터페이스
- [x] ER 다이어그램, 백업/복원, 예약된 작업, 사용자 및 권한
- [x] 쿼리와 데이터베이스를 위한 AI 채팅 (OpenSlate와 같은 방식)
- [ ] 데이터 및 구조 동기화, BI
- [ ] 더 많은 엔진 (Oracle, MongoDB, Redis)

이슈와 풀 리퀘스트를 환영합니다.

## 크레딧

DbCraft는 많은 오픈 소스 프로젝트의 성과 위에 만들어졌습니다. 감사합니다!

- **[GridCraft](https://github.com/storytold/gridcraft)**: 코드로 그린 **아이콘 세트**(같은 스타일의
  데이터베이스 아이콘으로 확장)와 `crates/dbgrid`의 데이터 그리드는 GridCraft에서
  파생되었습니다(그리드: 누적 합 기반 줄 인덱스, 고정 창을 포함한 지오메트리, 팔레트
  레이아웃 및 열 크기 조정 끌기 모델을 SQL 결과 집합에 맞게 조정).
  Copyright © 2026 ArtCraft Team and the GridCraft contributors, MIT OR Apache-2.0.
  [`crates/dbgrid/NOTICE.md`](crates/dbgrid/NOTICE.md)를 참조하세요.
- **[egui / eframe](https://github.com/emilk/egui)** (Emil Ernerfeldt 및 기여자): 즉시 모드
  GUI 툴킷.
- **[sqlx](https://github.com/launchbadge/sqlx)**: SQLite, PostgreSQL, MySQL 드라이버.
- **[tiberius](https://github.com/prisma/tiberius)**: SQL Server (TDS) 드라이버.
- **[tokio](https://tokio.rs)**: 비동기 런타임.
- **[reqwest](https://github.com/seanmonstar/reqwest)**: HTTP 터널용 HTTP 클라이언트.
- **[russh](https://github.com/Eugeny/russh)**: 터널용 SSH 클라이언트.
- **[SQLite](https://sqlite.org)** (퍼블릭 도메인), `libsqlite3-sys`를 통해 번들로 포함.
- **[syntect](https://github.com/trishume/syntect)** (`egui_extras` 경유): SQL 구문 강조.
- **[rust_xlsxwriter](https://github.com/jmcnamara/rust_xlsxwriter)**,
  **[csv](https://github.com/BurntSushi/rust-csv)**, **[serde](https://serde.rs)**: 내보내기 및 가져오기.
- **[keyring](https://github.com/hwchen/keyring-rs)**, **[rfd](https://github.com/PolyMeilex/rfd)**,
  **[dirs](https://github.com/soc/dirs-rs)**: OS 키체인, 네이티브 파일 대화 상자 및 설정 경로.
- **[sys-locale](https://github.com/1Password/sys-locale)**: 시스템 언어.
- **[Noto Sans CJK](https://github.com/notofonts/noto-cjk)** (SIL OFL 1.1): OpenSlate와 마찬가지로
  메뉴의 언어 이름 中文 日本語 한국어를 위한 아주 작은 서브셋.
- **[OpenSlate](https://github.com/rmpaolillo/OpenSlate)**: 번역 시스템, 자체 업데이트, AI
  채팅(페이지, 공급자 체인, 설정).
- **[wry](https://github.com/tauri-apps/wry)**: 창 안에 표시되는 AI 채팅의 WebView.
- **[KaTeX](https://katex.org)**, **[marked](https://marked.js.org)**, **[DOMPurify](https://github.com/cure53/DOMPurify)**
  및 **[Inter](https://rsms.me/inter/)** 글꼴, 채팅 페이지용으로 번들 포함(`crates/app/web/vendor` 참조).

타사 구성 요소와 그 라이선스의 전체 목록은 [CREDITS.md](CREDITS.md)에 있습니다.

## 라이선스

다음 중 하나의 라이선스를 따릅니다:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

어느 쪽을 적용할지는 사용자가 선택할 수 있습니다.

명시적으로 달리 밝히지 않는 한, Apache-2.0 라이선스에 정의된 바에 따라 이 저작물에 포함되도록
의도적으로 제출한 모든 기여는 추가 조건 없이 위와 같이 이중 라이선스가 적용됩니다.

## 면책 조항

DbCraft는 독립적인 프로젝트입니다. PremiumSoft CyberTech Ltd.와 제휴 관계가 없으며, 그로부터
보증이나 후원을 받지 않습니다. "Navicat"은 해당 소유자의 상표입니다. DbCraft가 어떤 종류의
도구인지 설명하기 위해서만 언급되며, DbCraft는 Navicat의 코드, 아트워크, 에셋을 전혀 사용하지 않습니다.
MySQL, MariaDB, PostgreSQL, SQL Server, SQLite는 각 소유자의 상표입니다.
