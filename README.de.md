# DbCraft

[![CI](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml/badge.svg)](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rmpaolillo/DbCraft?include_prereleases)](https://github.com/rmpaolillo/DbCraft/releases)

**Ein schneller, nativer Open-Source-Datenbankclient für den Desktop, geschrieben in Rust.**

DbCraft bringt den vertrauten Arbeitsablauf professioneller Datenbank-GUIs – „ein Fenster für
alle Datenbanken“ – in eine schlanke native App: gespeicherte Verbindungen, ein Objektbaum, ein
tabellenkalkulationsähnliches Datenraster, das auch bei großen Tabellen flüssig bleibt, ein
SQL-Editor, ein visueller Tabellendesigner sowie Import/Export, für **MySQL / MariaDB, PostgreSQL,
SQL Server und SQLite**.

*[English](README.md) · [Italiano](README.it.md) · [Español](README.es.md) · [Français](README.fr.md) · Deutsch · [Українська](README.uk.md) · [中文](README.zh.md) · [日本語](README.ja.md) · [한국어](README.ko.md)*

![DbCraft: die Objekte einer MySQL-Datenbank, mit Menüleiste, Symbolleiste und Verbindungsbaum](docs/screenshots/objects.png)

> **Status: frühe Entwicklung (v0.1).** Alle vier Engines bestehen dieselbe End-to-End-Testsuite
> gegen echte Server: PostgreSQL 17, MySQL 8.4, MariaDB 11.4, SQL Server 2022 und SQLite.
> Sichere deine Daten, bevor du Produktionsdatenbanken bearbeitest.

---

## Funktionen

| Bereich | Was du bekommst |
|---|---|
| **Verbindungen** | MySQL, MariaDB, PostgreSQL, SQL Server, SQLite. Gespeicherte Verbindungen, Verbindungstest, **SSL/TLS** (Serverprüfung gegen eine CA und den Hostnamen, Authentifizierung mit Client-Zertifikat), ein **SSH-Tunnel** mit Passwort- oder Private-Key-Authentifizierung (mit oder ohne Passphrase; der Host-Schlüssel wird mit `~/.ssh/known_hosts` abgeglichen) und ein **HTTP-Tunnel** über das PHP-Skript von DbCraft für Datenbanken, die nur von einem Webserver aus erreichbar sind ([tunnel/README.md](tunnel/README.md)). Passwörter liegen im Schlüsselbund des Betriebssystems (macOS-Schlüsselbund, Windows-Anmeldeinformationsverwaltung, Secret Service unter Linux), niemals im Klartext. |
| **Objektbaum** | Verbindung → Datenbank → Schema → Tabellen / Ansichten / Funktionen / Prozeduren / Trigger, bei Bedarf geladen, mit Kontextmenüs. Unten: **Suche** (filtert den Baum während der Eingabe), **★ nur Favoriten** und **alles einklappen**. |
| **Menüleiste** | Datei, Bearbeiten, Ansicht, **Favoriten**, Werkzeuge, Fenster, Hilfe. Jedes Objekt lässt sich zu den Favoriten hinzufügen (Rechtsklick › *Zu Favoriten hinzufügen*); das Menü Favoriten öffnet es und verbindet sich bei Bedarf zuerst. |
| **Objektansicht** | Eine durchsuchbare Liste der Objekte der ausgewählten Datenbank oder des Schemas, wie in Navicat: Name, Auto Increment, Änderungsdatum, Datengröße, Engine, Zeilen und Kommentar (die Spalten, die der Server liefert). **Tabelle / Ansicht / Funktion / Weitere ▾** in der Symbolleiste wechseln die Objektart: *Weitere* listet Trigger und, je nach Server, Indizes, Sequenzen, Ereignisse, Typen, Erweiterungen und Synonyme. |
| **Gespeicherte Abfragen** | **Abfrage** in der Symbolleiste listet die für die Datenbank gespeicherten Abfragen (⌘/Strg + S in einem Abfrage-Tab speichert dort): öffnen, neu, löschen. |
| **Benutzer** | Benutzer und Rollen des Servers mit ihren Privilegien; **Neuer Benutzer**, **Passwort ändern**, Privilegien auf einer Datenbank oder einem Schema **erteilen / entziehen**, **Benutzer löschen**. Das SQL wird vor der Ausführung angezeigt (Passwort verborgen). |
| **Backup / Wiederherstellung** | Ein Backup ist ein SQL-Skript mit Struktur und Daten jeder Tabelle (in Fremdschlüssel-Reihenfolge) und den Ansichten; der Tab **Backup** listet sie auf, stellt eines wieder her (oder eine beliebige `.sql`-Datei) und löscht sie. Hin- und Rückweg auf allen fünf Engines getestet. |
| **Automatisierung** | Geplante Aufgaben: ein Backup oder etwas SQL, alle *n* Minuten, täglich oder wöchentlich zu einer bestimmten Uhrzeit, mit nächster/letzter Ausführung und Ergebnis, und **Jetzt ausführen**. Aufgaben laufen, solange DbCraft geöffnet ist. |
| **Modell** | Das ER-Diagramm einer Datenbank oder eines Schemas: ein Kasten pro Tabelle mit ihren Spalten (Primär- und Fremdschlüssel markiert) und eine Linie pro Fremdschlüssel. Kästen ziehen, verschieben, zoomen, **Anordnen**, **SQL exportieren**; Doppelklick auf eine Tabelle, um sie zu entwerfen. |
| **Tabellenbetrachter / -editor** | Virtualisiertes Raster (nur sichtbare Zellen werden gezeichnet), serverseitiges Paging (100 – 50.000 Zeilen pro Seite). Bereich **Filter und Sortierung**: Bedingungen lesen sich als Sätze aus anklickbaren Elementen (Feld · Operator · Wert), jede mit der nächsten durch ein eigenes *and*/*or* verbunden, mit verschachtelten Gruppen in Klammern und 21 Operatoren (=, !=, <…, enthält, beginnt/endet mit und ihre Verneinungen, ist NULL/leer, liegt zwischen, ist in der Liste, benutzerdefiniertes SQL), oder ein freies `WHERE`; dazu Sortierung nach mehreren Spalten. Spalten ein-/ausblenden. **Raster-, Formular- und Textansicht**. Seitenbereich **Zelleneditor** für lange Texte, JSON (formatiert) und Binärdaten (Hex-Dump). **Datums-/Uhrzeitauswahl** in Datums-, Zeit- und Datetime/Timestamp-Spalten (Kalenderschaltfläche in der aktiven Zelle oder Alt+↓, auch in der Formularansicht und im Zelleneditor), wobei Sekundenbruchteile und Zeitzonen erhalten bleiben. Direkt bearbeiten, auf NULL setzen, Datensätze hinzufügen und löschen (Löschen fragt nach Bestätigung). Auto-Commit speichert einen Datensatz, sobald du ihn verlässt; **Transaktion starten** hält jede Änderung zurück bis **Commit** oder **Rollback**. |
| **Datenraster** | Spaltenköpfe mit Typ- und Primärschlüsselmarkierung, Zeilennummernleiste mit Markierung des aktuellen Datensatzes, NULL-Werte in Grau, Spaltenbreite anpassen (Doppelklick für automatische Breite), fixierte Spalten, Mehrfachauswahl von Zellen, Tastaturnavigation, Kopieren als TSV (mit oder ohne Überschriften). |
| **SQL-Editor** | Syntaxhervorhebung; **Code-Vervollständigung** (Schlüsselwörter, Tabellen, Ansichten, Funktionen und Spalten, auch nach einem Alias `u.`; Strg+Leertaste); Auswahl von Verbindung, Datenbank und Schema in der Symbolleiste; *Ausführen* von allem (⌘/Strg + Enter), der Auswahl oder der Anweisung am Cursor (⌘/Strg + Umschalt + Enter); **Stopp** (bricht bei PostgreSQL und MySQL/MariaDB die Abfrage auch auf dem Server ab); **Explain** (Ausführungsplan; SHOWPLAN bei SQL Server); **SQL verschönern** (⌘/Strg + Umschalt + F); fertige **Snippets**; ein Ergebnis-Tab pro Anweisung, ein Meldungsprotokoll, Z./Sp. in der Statusleiste, `.sql`-Dateien öffnen und speichern. |
| **Query Builder** | Visueller Abfrageentwurf: Tabellen auf eine Arbeitsfläche ziehen, Spalten anhaken, eine Spalte auf die Spalte einer anderen Tabelle ziehen, um sie zu verknüpfen (INNER / LEFT / RIGHT / FULL, als Kurven dargestellt); Fremdschlüssel werden automatisch verknüpft. Aggregatfunktionen mit automatischem GROUP BY, Aliase, Bedingungen und Sortierung mit demselben Element-Editor wie der Tabellenfilter, DISTINCT und LIMIT (TOP bei SQL Server), Live-SQL-Vorschau; **OK** übernimmt das SQL in den Editor. |
| **Tabellendesigner** | Felder (Name, Typ, Not null, Schlüssel, Auto-Inkrement, Standardwert, Kommentar), Indizes und Fremdschlüssel, mit einer Live-**SQL-Vorschau** der `CREATE TABLE`- / `ALTER TABLE`-Anweisungen im Dialekt der Verbindung. Bei SQLite werden Änderungen, die sich nicht mit `ALTER` ausdrücken lassen, durch Neuaufbau der Tabelle angewendet, wobei ihre Daten erhalten bleiben. |
| **Export** | Tabellen, Ansichten oder Abfrageergebnisse als CSV, JSON, SQL-`INSERT`-Skript oder Excel (`.xlsx`). |
| **Import** | CSV mit einstellbarem Trennzeichen und Kopfzeile, einer Vorschau der Feldzuordnung und optional leere Zelle → NULL. Zeilen werden stapelweise innerhalb von Transaktionen eingefügt. |
| **Sprachen** | Die Oberfläche spricht **English, Italiano, Español, Français, Deutsch, Українська, 中文, 日本語 und 한국어**: standardmäßig die Systemsprache oder die unter **Ansicht › Sprache** gewählte (wirkt sofort). Chinesisch, Japanisch und Koreanisch verwenden eine Schrift des Systems. |
| **Oberfläche** | Helles und dunkles Design. |

| Tabellenbetrachter | SQL-Editor (dunkles Design) |
|---|---|
| ![Tabellenbetrachter mit dem Bereich Filter und Sortierung](docs/screenshots/table-viewer.png) | ![SQL-Editor mit Code-Vervollständigung, dunkles Design](docs/screenshots/query-editor-dark.png) |
| **Modell (ER-Diagramm)** | **Benutzer und Privilegien** |
| ![ER-Diagramm einer Datenbank](docs/screenshots/model.png) | ![Benutzer eines MySQL-Servers mit ihren Privilegien](docs/screenshots/users.png) |
| **Backup** | **Automatisierung** |
| ![Backups einer Datenbank](docs/screenshots/backup.png) | ![Geplante Backup- und Abfrageaufgaben](docs/screenshots/automation.png) |

## Erste Schritte

### Download

Fertige Pakete gibt es auf der Seite [Releases](https://github.com/rmpaolillo/DbCraft/releases):

- **macOS 11+ (Apple silicon)**: `DbCraft-<version>-macos-arm64.dmg`, dann DbCraft in den Ordner
  Programme ziehen. Builds ohne Apple Developer ID sind ad hoc signiert: Erlaube die App einmalig
  per Rechtsklick → **Öffnen** oder unter Systemeinstellungen → Datenschutz & Sicherheit → **Dennoch öffnen**.
- **Debian / Ubuntu**: `sudo apt install ./dbcraft_<version>_amd64.deb`
- **Andere Linux-Systeme (x86_64, glibc 2.35+)**: `dbcraft-<version>-linux-x86_64.tar.gz`
- **Windows 10/11 (x64)**: `dbcraft-<version>-windows-x86_64.zip` (die Visual-C++-Laufzeit ist
  enthalten); SmartScreen fragt eventuell nach einer Bestätigung.

#### Updates

Eine installierte Version prüft beim Start und alle 12 Stunden auf GitHub, ob es eine neuere
Release gibt. Ist eine vorhanden, bietet eine Leiste unter der Symbolleiste **Jetzt aktualisieren**
(Download, SHA-256-Prüfung gegen `SHA256SUMS.txt`, Installation anstelle dieser Kopie, dann
**Jetzt neu starten**), **Neuigkeiten** (Versionshinweise) und **Diese Version überspringen** an.
Die macOS-App wird an Ort und Stelle ausgetauscht (die vorherige bleibt im Konfigurationsordner
unter `updates/` erhalten), der Linux-Ordner aus dem `.tar.gz` erhält die neuen Dateien, das
`.deb` wird mit `pkexec dpkg -i` installiert, und unter Windows tauscht ein Hilfsprogramm die
Dateien aus, wenn die App geschlossen wird. Gespeicherte Verbindungen, Einstellungen und
Passwörter im Schlüsselbund bleiben unberührt. Die Schaltfläche **Updates** in der Symbolleiste
prüft von Hand und schaltet die Benachrichtigung oder die automatische Installation ein und aus.
Aus dem Quellcode kompilierte Builds aktualisieren sich nicht selbst (`git pull`); um die Leiste
trotzdem auszuprobieren, starte mit `DBCRAFT_UPDATE_TEST=1 DBCRAFT_PRETEND_VERSION=0.0.1`.

### Voraussetzungen

- [Rust](https://rustup.rs) **1.95+** (Edition 2024)
- macOS, Windows oder Linux. Linux benötigt die üblichen Entwicklungspakete für ein Desktopfenster
  (X11/Wayland, `libxkbcommon`) und D-Bus für den Schlüsselbund.

### Bauen und starten

```sh
git clone https://github.com/rmpaolillo/DbCraft.git
cd DbCraft
cargo run --release                     # die App starten
cargo run --release -- path/to/file.db  # eine SQLite-Datei direkt öffnen (temporäre Verbindung)
cargo run --release -- "postgres://user:password@localhost:5432/mydb"  # oder einen beliebigen Server per URL
```

URL-Schemata: `postgres://`, `mysql://`, `mariadb://`, `sqlserver://` (oder `mssql://`) und
`sqlite://`. Sonderzeichen im Passwort müssen prozentkodiert werden (zum Beispiel `@` → `%40`).
Füge `?ssh=user:password@ssh-host:port` hinzu, um über einen SSH-Tunnel zu gehen, oder
`?ssh=user@ssh-host&ssh_key=~/.ssh/id_ed25519` für die Authentifizierung per Schlüssel.

Um eine Verbindung anzulegen, klicke in der Symbolleiste auf **Verbindung** und wähle den Datenbanktyp.

### Kommandozeilenoptionen

| Option | Beschreibung |
|---|---|
| `<file.db>` oder `<url>` | Eine SQLite-Datei oder eine Server-URL als temporäre, nicht gespeicherte Verbindung öffnen |
| `--open-table <name>` | Eine Tabelle dieser Datei öffnen, sobald sie geladen ist |
| `--query "<sql>"` | Einen Abfrage-Tab mit diesem SQL öffnen und ausführen |
| `--dark` | Mit dem dunklen Design starten |
| `--lang <code>` | Sprache der Oberfläche für diesen Start: `en`, `it`, `es`, `fr`, `de`, `uk`, `zh`, `ja`, `ko` |
| `--open <tool>` | Ein Werkzeug öffnen, sobald die Datenbank geladen ist: `users`, `backup`, `automation`, `model`, `queries`, `others:<kind>` |
| `--new-connection <kind> [--ssh-tab]` | Den Dialog für eine neue Verbindung öffnen (`mysql`, `postgres`, …), optional auf seinem SSH-Tab |
| `--screenshot <out.png> [--delay <s>]` | Nach `s` Sekunden (Standard 3) ein PNG des Fensters speichern und beenden. Wird für automatische UI-Prüfungen verwendet. |

Gespeicherte Verbindungen liegen im Konfigurationsverzeichnis des Betriebssystems
(`dbcraft/connections.json`), zum Beispiel `~/Library/Application Support/dbcraft/` unter macOS,
neben `settings.json`, `favorites.json`, `automation.json` und den Ordnern `queries/` und
`backups/`. Setze `DBCRAFT_CONFIG_DIR`, um einen anderen Ordner zu verwenden.

### Tests

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Integrationstests der Treiber gegen echte Server (Docker):

```sh
scripts/test-certs.sh && mkdir -p target/ssh-test   # Testzertifikate, Ordner für SSH-Schlüssel
docker compose -f docker-compose.test.yml up -d --wait
DBCRAFT_TEST_SERVERS=1 cargo test -p db-drivers --test servers --test ssh --test tls --test http -- --test-threads=1
DBCRAFT_TEST_SERVERS=1 cargo test -p dbcraft -- --test-threads=1   # Backup/Wiederherstellung, Benutzer und Kataloge
docker compose -f docker-compose.test.yml down -v
```

Die Suite legt Tabellen mit Indizes und Fremdschlüsseln an, führt Skripte mit mehreren
Anweisungen aus, prüft die Introspektion, wendet transaktionale Änderungen an und rollt sie
zurück, ändert Tabellen über das SQL des Designers und dekodiert die Datentypen jeder Engine
(Zeitstempel mit Zeitzonen, UUID, JSON, Arrays, Money, vorzeichenlose 64-Bit-Zahlen, XML, …). Die
SSH-Tests laufen über einen OpenSSH-Server-Container mit Passwort, RSA- und passphrasegeschützten
Ed25519-Schlüsseln; die SSL-Tests verwenden PostgreSQL und MySQL mit Testzertifikaten
(verify-full, Client-Zertifikate, ein MySQL-Benutzer mit `REQUIRE X509`). Das SQL-Server-Image
gibt es nur für amd64; auf Apple Silicon läuft es unter Emulation.

### Veröffentlichen (Maintainer)

Eine Release wird nur veröffentlicht, wenn du es entscheidest: **Actions → Release → Run workflow**
(Branch `main`). Vorher muss nichts bearbeitet werden: Die Versionsnummer ist bereit.

- Die Version ist `MAJOR.MINOR` aus `Cargo.toml` (`[workspace.package] version`) plus ein `PATCH`,
  der anhand der vorhandenen Tags gezählt wird: noch kein `v0.1.*`-Tag → `v0.1.0`; letzter Tag
  `v0.1.4` → `v0.1.5`.
- Um eine neue Reihe zu beginnen (`0.2.0`, `1.0.0`…), ändere die Version in `Cargo.toml` und
  pushe; die nächste Release beginnt bei `.0`.
- Die Nummer wird nur innerhalb des Builds in `Cargo.toml`/`Cargo.lock` geschrieben: Nichts wird
  ins Repository zurück committet.

Der Workflow testet und baut macOS Apple silicon (`.dmg`), Linux x86_64 (`.deb` und `.tar.gz`)
und Windows x86_64 (`.zip`) und veröffentlicht dann die Release `v<version>` mit den Paketen und
`SHA256SUMS.txt`. Nur die beiden neuesten Releases werden behalten: Ältere Releases und ihre Tags
werden entfernt.

Optionale macOS-Signierung: Mit den Repository-Secrets `APPLE_CERTIFICATE` (Base64-`.p12` mit
einer „Developer ID Application“-Identität), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`,
`APPLE_PASSWORD` (app-spezifisches Passwort) und `APPLE_TEAM_ID` werden die App und das DMG mit
der Developer ID signiert und notarisiert; ohne sie werden sie ad hoc signiert.

Jeder Push und jeder Pull Request führt außerdem die **CI** aus (`.github/workflows/ci.yml`):
clippy, Tests unter Linux, macOS und Windows sowie die Treiber-Suite gegen echte PostgreSQL-,
MySQL-, MariaDB- und SQL-Server-Service-Container.

## Architektur

DbCraft ist ein Cargo-Workspace mit vier Crates:

```
crates/
├── dbgrid       Virtualisiertes egui-Datenraster (GridModel-Trait → DataGrid-Widget → GridEvent)
├── db-core      Value- / ResultSet- / Schema-Typen, der asynchrone Driver-Trait, SQL-Dialekte und DDL-Erzeugung
├── db-drivers   SQLite, PostgreSQL, MySQL/MariaDB (sqlx) und SQL Server (tiberius)
└── app          Die Desktopanwendung `dbcraft` (eframe)
```

- **Übersetzungen** funktionieren wie in OpenSlate: Der italienische Text im Code ist der
  Schlüssel, und `crates/app/src/i18n/<lang>.json` liefert ihn in den anderen Sprachen
  (Platzhalter wie `{n}` oder `{}` übersetzen auch zur Laufzeit zusammengesetzte Texte). Starte
  mit `DBCRAFT_I18N_MISSING=missing.txt`, um die ohne Übersetzung angezeigten Texte zu sammeln.

- Die Oberfläche blockiert nie: Datenbankarbeit läuft in einer **tokio**-Laufzeit im
  Hintergrund, und die Ergebnisse kommen über einen Kanal zurück und lösen ein Neuzeichnen aus.
- Das Raster weiß nichts über Datenbanken. Es liest über den kleinen `GridModel`-Trait und meldet
  Benutzerabsichten (`CellEdited`, `SortRequested`, `DeleteRows`, …) als Ereignisse, sodass die
  App entscheidet, was zu tun ist.
- SQL, das sich zwischen den Engines unterscheidet (Quoting von Bezeichnern, Literale, Paging,
  `INSERT`, `UPDATE`, `DELETE`, `CREATE`, `ALTER`), liegt an einer Stelle, `db_core::Dialect`, und
  ist durch Unit-Tests abgedeckt.

## Roadmap

- [x] Integrationstests gegen PostgreSQL, MySQL/MariaDB und SQL Server (Docker)
- [x] SQL-Autovervollständigung (Schlüsselwörter, Tabellen, Spalten)
- [x] Laufende Abfrage abbrechen
- [x] SSH-Tunnel (Passwort, privater Schlüssel mit optionaler Passphrase)
- [x] SSL-Optionen (CA-Prüfung, Hostnamenprüfung, Client-Zertifikate)
- [x] HTTP-Tunnel (PHP-Skript von DbCraft, MySQL/MariaDB und PostgreSQL)
- [x] Eigener Satz von Vektorsymbolen
- [x] Oberfläche in neun Sprachen
- [x] ER-Diagramm, Backup/Wiederherstellung, geplante Aufgaben, Benutzer und Privilegien
- [ ] Daten- und Struktursynchronisation, BI
- [ ] Weitere Engines (Oracle, MongoDB, Redis)

Issues und Pull Requests sind willkommen.

## Danksagungen

DbCraft baut auf der Arbeit vieler Open-Source-Projekte auf. Vielen Dank!

- **[GridCraft](https://github.com/storytold/gridcraft)**: Der per Code gezeichnete
  **Symbolsatz** (im selben Stil um Datenbanksymbole erweitert) und das Datenraster in
  `crates/dbgrid` sind von GridCraft abgeleitet (das Raster: Zeilenindex auf Präfixsummen,
  Geometrie mit fixierten Bereichen, Palettenlayout und Ziehmodell zur Spaltenbreitenänderung,
  angepasst an SQL-Ergebnismengen).
  Copyright © 2026 ArtCraft Team and the GridCraft contributors, MIT OR Apache-2.0. Siehe
  [`crates/dbgrid/NOTICE.md`](crates/dbgrid/NOTICE.md).
- **[egui / eframe](https://github.com/emilk/egui)** von Emil Ernerfeldt und Mitwirkenden: das
  Immediate-Mode-GUI-Toolkit.
- **[sqlx](https://github.com/launchbadge/sqlx)**: Treiber für SQLite, PostgreSQL und MySQL.
- **[tiberius](https://github.com/prisma/tiberius)**: Treiber für SQL Server (TDS).
- **[tokio](https://tokio.rs)**: asynchrone Laufzeit.
- **[reqwest](https://github.com/seanmonstar/reqwest)**: HTTP-Client für den HTTP-Tunnel.
- **[russh](https://github.com/Eugeny/russh)**: SSH-Client für den Tunnel.
- **[SQLite](https://sqlite.org)** (gemeinfrei), eingebunden über `libsqlite3-sys`.
- **[syntect](https://github.com/trishume/syntect)** (über `egui_extras`): SQL-Syntaxhervorhebung.
- **[rust_xlsxwriter](https://github.com/jmcnamara/rust_xlsxwriter)**,
  **[csv](https://github.com/BurntSushi/rust-csv)**, **[serde](https://serde.rs)**: Export und Import.
- **[keyring](https://github.com/hwchen/keyring-rs)**, **[rfd](https://github.com/PolyMeilex/rfd)**,
  **[dirs](https://github.com/soc/dirs-rs)**: Schlüsselbund des Betriebssystems, native Dateidialoge und Konfigurationspfade.
- **[sys-locale](https://github.com/1Password/sys-locale)**: die Systemsprache.
- **[Noto Sans CJK](https://github.com/notofonts/noto-cjk)** (SIL OFL 1.1): eine winzige Teilmenge
  für die Sprachnamen 中文 日本語 한국어 im Menü, wie in OpenSlate.
- **[OpenSlate](https://github.com/rmpaolillo/OpenSlate)**: das Übersetzungssystem und die Selbstaktualisierung.

Die vollständige Liste der Drittkomponenten und ihrer Lizenzen steht in [CREDITS.md](CREDITS.md).

## Lizenz

Lizenziert nach Wahl unter

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT-Lizenz ([LICENSE-MIT](LICENSE-MIT))

Sofern du nicht ausdrücklich etwas anderes angibst, wird jeder Beitrag, den du absichtlich zur
Aufnahme in dieses Werk einreichst, wie in der Apache-2.0-Lizenz definiert, wie oben doppelt
lizenziert, ohne zusätzliche Bedingungen oder Konditionen.

## Haftungsausschluss

DbCraft ist ein unabhängiges Projekt. Es ist weder mit PremiumSoft CyberTech Ltd. verbunden noch
von ihr unterstützt oder gesponsert. „Navicat“ ist eine Marke ihres Inhabers. Sie wird nur
erwähnt, um die Art von Werkzeug zu beschreiben, die DbCraft ist, und DbCraft verwendet keinerlei
Code, Grafiken oder sonstige Assets davon. MySQL, MariaDB, PostgreSQL, SQL Server und SQLite sind
Marken ihrer jeweiligen Inhaber.
