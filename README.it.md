<p align="center"><img src="packaging/social/social-preview.png" width="100%" alt="DbCraft"></p>

# DbCraft

[![CI](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml/badge.svg)](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rmpaolillo/DbCraft?include_prereleases)](https://github.com/rmpaolillo/DbCraft/releases)

**Client database desktop nativo, veloce e open source, scritto in Rust.**

DbCraft porta in un'app nativa e leggera il flusso di lavoro dei client database professionali
("una finestra per tutti i tuoi database"): connessioni salvate, albero degli oggetti, una griglia
dati in stile foglio di calcolo che resta fluida anche su tabelle grandi, editor SQL, designer
visuale delle tabelle e import/export, per **MySQL / MariaDB, PostgreSQL, SQL Server e SQLite**.

Ha anche un **assistente AI** integrato: gli chiedi le cose a parole e lui legge la struttura e i dati dei tuoi database, scrive e spiega l'SQL, apre le query nell'editor e modifica i dati solo dopo il tuo sì. Funziona con modelli locali (LM Studio, Ollama) o con servizi nel cloud (OpenAI, Claude, Gemini…).

*[English](README.md) · Italiano · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Українська](README.uk.md) · [中文](README.zh.md) · [日本語](README.ja.md) · [한국어](README.ko.md)*

![DbCraft: gli oggetti di un database MySQL, con la barra dei menu, la toolbar e l'albero delle connessioni](docs/screenshots/objects.png)

> **Stato: sviluppo iniziale (v0.2).** Tutti i motori superano la stessa suite di test
> end-to-end su server reali: PostgreSQL 17, MySQL 8.4, MariaDB 11.4, SQL Server 2022 e SQLite.
> Fai un backup prima di modificare database di produzione.

---

## Funzionalità

| Area | Cosa offre |
|---|---|
| **Connessioni** | MySQL, MariaDB, PostgreSQL, SQL Server, SQLite. Connessioni salvate, prova connessione, **SSL/TLS** (verifica del server con CA e nome host, autenticazione con certificato client), **tunnel SSH** con autenticazione tramite password o chiave privata (con o senza passphrase; la chiave dell'host è verificata con `~/.ssh/known_hosts`) e **tunnel HTTP** tramite lo script PHP di DbCraft, per i database raggiungibili solo da un web server ([tunnel/README.md](tunnel/README.md)). Le password stanno nel portachiavi del sistema operativo (Portachiavi di macOS, Gestione credenziali di Windows, Secret Service su Linux), mai in chiaro. |
| **Albero oggetti** | Connessione → Database → Schema → Tabelle / Viste / Funzioni / Procedure / Trigger, caricati su richiesta, con menu contestuali. In fondo: **Cerca** (filtra l'albero mentre scrivi), **★ solo preferiti** e **Comprimi tutto**. |
| **Barra dei menu** | File, Modifica, Visualizza, **Preferiti**, Strumenti, Finestra, Aiuto. Qualsiasi oggetto si aggiunge ai preferiti (clic destro › *Aggiungi ai preferiti*); il menu Preferiti lo apre, collegandosi prima se serve. |
| **Vista Oggetti** | Elenco ricercabile degli oggetti del database o schema selezionato, come in Navicat: nome, auto increment, data modifica, dimensione dati, engine, righe e commento (le colonne che il server fornisce). **Tabella / Vista / Funzione / Altri ▾** della toolbar cambiano il tipo di oggetto: *Altri* mostra i trigger e, secondo il server, indici, sequenze, eventi, tipi, estensioni e sinonimi. |
| **Query salvate** | **Query** nella toolbar elenca le query salvate per il database (⌘/Ctrl + S in una scheda Query le salva lì): apri, nuova, elimina. |
| **Utenti** | Utenti e ruoli del server con i loro privilegi; **nuovo utente**, **cambia password**, **concedi / revoca** privilegi su un database o schema, **elimina utente**. L'SQL si vede prima di eseguirlo (con la password nascosta). |
| **Backup / ripristino** | Un backup è uno script SQL con struttura e dati di tutte le tabelle (nell'ordine delle chiavi esterne) e le viste; la scheda **Backup** li elenca, ne ripristina uno (o qualsiasi file `.sql`) e li elimina. Andata e ritorno verificati su tutti e cinque i motori. |
| **Automazione** | Attività pianificate: un backup o del codice SQL, ogni *n* minuti, ogni giorno o ogni settimana a un'ora scelta, con prossima/ultima esecuzione ed esito, e **Esegui ora**. Le attività girano mentre DbCraft è aperto. |
| **Modello** | Il diagramma ER di un database o schema: un riquadro per tabella con le sue colonne (chiavi primarie ed esterne segnate) e una linea per chiave esterna. Riquadri trascinabili, scorrimento, zoom, **Disponi**, **Esporta SQL**; doppio clic su una tabella per progettarla. |
| **Visualizzatore / editor tabelle** | Griglia virtualizzata (disegna solo le celle visibili), paginazione lato server (da 100 a 50.000 righe per pagina). Pannello **Filtro e ordinamento**: le condizioni si leggono come frasi di elementi cliccabili (campo · operatore · valore), ognuna collegata alla successiva dal proprio *and*/*or*, con gruppi tra parentesi annidabili e 21 operatori (=, !=, <…, contiene, inizia/finisce con e le loro negazioni, è NULL/vuoto, compreso tra, nell'elenco, SQL personalizzato), oppure `WHERE` libero; e ordinamento su più colonne. Mostra/nascondi colonne. **Viste Griglia, Modulo e Testo**. **Editor cella** laterale per testi lunghi, JSON (formattazione) e dati binari (dump esadecimale). **Selettore data/ora** sulle colonne date, time e datetime/timestamp (pulsante calendario sulla cella attiva o Alt+↓, anche nella vista Modulo e nell'Editor cella), che conserva frazioni di secondo e fuso orario. Modifica nella cella, imposta NULL, aggiungi ed elimina record (l'eliminazione chiede conferma). In auto-commit un record viene salvato quando lo lasci; **Inizia transazione** tiene tutte le modifiche in sospeso fino a **Commit** o **Rollback**. |
| **Griglia dati** | Intestazioni con tipo e indicatore di chiave primaria, numeri di riga con indicatore del record corrente, NULL in grigio, ridimensionamento colonne (doppio clic per adattare), colonne bloccate, selezione multipla, navigazione da tastiera, copia in formato TSV (con o senza intestazioni). |
| **Editor SQL** | Evidenziazione della sintassi; **completamento del codice** (parole chiave, tabelle, viste, funzioni e colonne, anche dopo un alias `u.`; Ctrl+Spazio); scelta di connessione, database e schema dalla barra; *Esegui* tutto (⌘/Ctrl + Invio), la selezione o l'istruzione al cursore (⌘/Ctrl + Maiusc + Invio); **Stop** (annulla la query anche sul server con PostgreSQL e MySQL/MariaDB); **Explain** (piano di esecuzione; SHOWPLAN su SQL Server); **Abbellisci SQL** (⌘/Ctrl + Maiusc + F); **Snippet** di istruzioni pronte; un tab di risultato per istruzione, log dei messaggi, Ln/Col nella barra di stato, apertura e salvataggio di file `.sql`. |
| **Query Builder** | Progettazione visuale delle query: trascina le tabelle sulla tela, spunta le colonne, trascina una colonna su quella di un'altra tabella per unirle (INNER / LEFT / RIGHT / FULL, disegnati come curve); le chiavi esterne creano i JOIN in automatico. Funzioni di aggregazione con GROUP BY automatico, alias, condizioni e ordinamento con lo stesso editor a frasi del filtro, DISTINCT e LIMIT (TOP su SQL Server), anteprima SQL in tempo reale; **OK** inserisce l'SQL nell'editor. |
| **Designer tabelle** | Campi (nome, tipo, non null, chiave, auto-incremento, predefinito, commento), indici e chiavi esterne, con **anteprima SQL** in tempo reale delle istruzioni `CREATE TABLE` / `ALTER TABLE` nel dialetto della connessione. Su SQLite, le modifiche non esprimibili con `ALTER` vengono applicate ricostruendo la tabella, senza perdere i dati. |
| **Esportazione** | Tabelle, viste o risultati di query in CSV, JSON, script SQL `INSERT` o Excel (`.xlsx`). |
| **Importazione** | CSV con delimitatore e riga di intestazione configurabili, anteprima e associazione dei campi, opzione "cella vuota = NULL". Le righe vengono inserite a blocchi dentro transazioni. |
| **Chat AI** | Un assistente AI in un riquadro a destra (**Chat AI** nella toolbar), lo stesso di OpenSlate: scrive, spiega, corregge e ottimizza l'SQL e lavora sui tuoi database con degli strumenti. Vede le connessioni configurate, legge struttura e dati (query di sola lettura), apre le query in una nuova scheda dell'editor e fa modifiche solo dopo il tuo **sì** in chat. Qualsiasi servizio compatibile OpenAI (LM Studio, Ollama, OpenAI, Anthropic Claude, Google Gemini, DeepSeek, OpenRouter…), provati in ordine con riserva; conversazioni salvate, Markdown e formule, indicatore del contesto con compattazione automatica e **Impostazioni AI** (agenti, chiavi nel Portachiavi, prova, modelli, contesto). |
| **Lingue** | L'interfaccia parla **English, Italiano, Español, Français, Deutsch, Українська, 中文, 日本語 e 한국어**: quella del sistema, oppure quella scelta in **Visualizza › Lingua** (vale subito). Cinese, giapponese e coreano usano un carattere del sistema. |
| **Interfaccia** | Tema chiaro e scuro. |

| Visualizzatore tabelle | Editor SQL (tema scuro) |
|---|---|
| ![Visualizzatore tabelle con il pannello Filtro e ordinamento](docs/screenshots/table-viewer.png) | ![Editor SQL con il completamento del codice, tema scuro](docs/screenshots/query-editor-dark.png) |
| **Modello (diagramma ER)** | **Utenti e privilegi** |
| ![Diagramma ER di un database](docs/screenshots/model.png) | ![Utenti di un server MySQL con i loro privilegi](docs/screenshots/users.png) |
| **Backup** | **Automazione** |
| ![Backup di un database](docs/screenshots/backup.png) | ![Attività pianificate di backup e query](docs/screenshots/automation.png) |
| **Chat AI** | **Impostazioni AI** |
| ![La chat AI: l'assistente ha letto le tabelle, eseguito una query e l'ha aperta nell'editor](docs/screenshots/ai-chat.png) | ![Impostazioni AI: agenti provati in ordine, chiavi, contesto e cosa può fare l'agente con i database](docs/screenshots/ai-settings.png) |

Gli screenshot mostrano l'interfaccia in inglese; in italiano si sceglie da **View › Language** (o
è già così se il sistema è in italiano).

## Per iniziare

### Download

I pacchetti già compilati sono nella pagina [Releases](https://github.com/rmpaolillo/DbCraft/releases):

- **macOS 11+ (Apple silicon)**: `DbCraft-<versione>-macos-arm64.dmg`, poi trascina DbCraft in
  Applicazioni. Le build senza Apple Developer ID sono firmate ad hoc: consenti l'app una volta
  con clic destro → **Apri**, oppure Impostazioni di Sistema → Privacy e sicurezza → **Apri
  comunque**.
- **Debian / Ubuntu**: `sudo apt install ./dbcraft_<versione>_amd64.deb`
- **Altri Linux (x86_64, glibc 2.35+)**: `dbcraft-<versione>-linux-x86_64.tar.gz`
- **Windows 10/11 (x64)**: `dbcraft-<versione>-windows-x86_64.zip` (il runtime Visual C++ è
  incluso); SmartScreen potrebbe chiedere conferma.

#### Aggiornamenti

Una release installata chiede a GitHub, all'avvio e ogni 12 ore, se c'è una versione più nuova.
Se c'è, una barra sotto la toolbar propone **Aggiorna ora** (scarica, controlla l'impronta SHA-256 con
`SHA256SUMS.txt`, installa al posto di questa copia, poi **Riavvia ora**), **Novità** (note della
release) e **Salta questa versione**. Su macOS l'app viene sostituita sul posto (la precedente
resta nella cartella di configurazione, in `updates/`); su Linux la cartella del `.tar.gz` riceve i
file nuovi e il `.deb` si installa con `pkexec dpkg -i`; su Windows un aiutante cambia i file
quando chiudi l'app. Connessioni salvate, impostazioni e password nel portachiavi non vengono
toccate. Il pulsante **Aggiornamenti** della toolbar controlla a mano e attiva o disattiva
l'avviso e l'installazione automatica. Le build dai sorgenti non si aggiornano da sole
(`git pull`); per provare la barra: `DBCRAFT_UPDATE_TEST=1 DBCRAFT_PRETEND_VERSION=0.0.1`.

#### Assistente AI

**Chat AI** nella toolbar (o Strumenti › Chat AI) apre la chat a destra; **Strumenti › Impostazioni AI**
apre le sue impostazioni. Sono le pagine di [OpenSlate](https://github.com/rmpaolillo/OpenSlate), con la
stessa logica: gli agenti si provano **in ordine** e se uno non risponde (server spento, chiave mancante,
errore) passa al successivo. Stanno in `ai.toml` nella cartella di configurazione; una chiave non ci
finisce mai in chiaro: `keychain:NOME` (una chiave scritta nella pagina delle impostazioni va nel
Portachiavi del sistema), `env:NOME` o `dsh:NOME` (DeepSeek Harness).

La chat sa qual è il database selezionato in DbCraft e ha degli strumenti:

| Strumento | Cosa fa |
|---|---|
| `db_connections`, `db_databases`, `db_tables`, `db_describe` | le connessioni configurate e la struttura dei loro database |
| `db_query` | esegue SQL **di sola lettura** (SELECT, WITH, SHOW, EXPLAIN…), al massimo `max_rows` righe |
| `db_open_query` | apre una query in una nuova scheda dell'editor, senza eseguirla |
| `db_execute` | modifica dati o struttura: prima prepara soltanto la modifica e tu vedi l'SQL esatto, poi la esegue **solo dopo il tuo sì** nel messaggio successivo (e solo se *Impostazioni AI › Il database* permette le modifiche) |

Ogni blocco SQL di una risposta ha **Apri nell'editor** e **Copia**. Con un modello locale (LM Studio,
Ollama) non esce niente dal computer; con un servizio nel cloud, le domande e i dati letti dall'AI vanno
a quel servizio.

### Requisiti

- [Rust](https://rustup.rs) **1.95+** (edition 2024)
- macOS, Windows o Linux. Su Linux servono i pacchetti di sviluppo per le finestre desktop
  (X11/Wayland, `libxkbcommon`), D-Bus per il portachiavi e WebKitGTK 4.1 per la chat AI
  (`libwebkit2gtk-4.1-dev`).

### Compilare ed eseguire

```sh
git clone https://github.com/rmpaolillo/DbCraft.git
cd DbCraft
cargo run --release                      # avvia l'app
cargo run --release -- percorso/file.db  # apre direttamente un file SQLite (connessione temporanea)
cargo run --release -- "postgres://utente:password@localhost:5432/miodb"  # o un server tramite URL
```

Schemi URL: `postgres://`, `mysql://`, `mariadb://`, `sqlserver://` (o `mssql://`) e
`sqlite://`. Codifica i caratteri speciali della password (per esempio `@` → `%40`).
Aggiungi `?ssh=utente:password@host-ssh:porta` per passare da un tunnel SSH, oppure
`?ssh=utente@host-ssh&ssh_key=~/.ssh/id_ed25519` per l'autenticazione con chiave.

Per creare una connessione, clicca **Connessione** nella barra in alto e scegli il tipo di database.

### Opzioni da riga di comando

| Opzione | Descrizione |
|---|---|
| `<file.db>` o `<url>` | Apre un file SQLite o un server tramite URL come connessione temporanea, non salvata |
| `--open-table <nome>` | Apre una tabella di quel file appena è caricata |
| `--query "<sql>"` | Apre un tab query con questo SQL e lo esegue |
| `--dark` | Avvia con il tema scuro |
| `--lang <codice>` | Lingua dell'interfaccia per questo avvio: `it`, `en`, `es`, `fr`, `de`, `uk`, `zh`, `ja`, `ko` |
| `--open <strumento>` | Apre uno strumento quando il database è caricato: `users`, `backup`, `automation`, `model`, `queries`, `chat`, `ai-settings`, `others:<tipo>` |
| `--new-connection <tipo> [--ssh-tab]` | Apre il dialogo di nuova connessione (`mysql`, `postgres`, …), eventualmente sulla scheda SSH |
| `--screenshot <out.png> [--delay <s>]` | Salva un PNG della finestra dopo `s` secondi (predefinito 3), poi esce. Serve per le verifiche automatiche dell'interfaccia. |

Le connessioni salvate stanno nella cartella di configurazione del sistema
(`dbcraft/connections.json`), per esempio `~/Library/Application Support/dbcraft/` su macOS,
insieme a `settings.json`, `favorites.json`, `automation.json` e alle cartelle `queries/` e
`backups/`. Imposta `DBCRAFT_CONFIG_DIR` per usare un'altra cartella.

### Test

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Test di integrazione dei driver su server reali (Docker):

```sh
scripts/test-certs.sh && mkdir -p target/ssh-test   # certificati di test, cartella chiavi SSH
docker compose -f docker-compose.test.yml up -d --wait
DBCRAFT_TEST_SERVERS=1 cargo test -p db-drivers --test servers --test ssh --test tls --test http -- --test-threads=1
DBCRAFT_TEST_SERVERS=1 cargo test -p dbcraft -- --test-threads=1   # backup/ripristino, utenti e cataloghi
docker compose -f docker-compose.test.yml down -v
```

La suite crea tabelle con indici e chiavi esterne, esegue script con più istruzioni, verifica
l'introspezione, applica e annulla modifiche in transazione, modifica le tabelle con l'SQL del
designer e controlla la decodifica dei tipi di ogni motore (timestamp con fuso orario, UUID,
JSON, array, money, interi senza segno a 64 bit, XML, …). I test SSH passano da un container
OpenSSH con password, chiave RSA e chiave Ed25519 protetta da passphrase; i test SSL usano
PostgreSQL e MySQL con certificati di test (verify-full, certificati client, un utente MySQL
`REQUIRE X509`). L'immagine di SQL Server è solo amd64 e
su Apple Silicon gira in emulazione.

### Pubblicare una release (manutentori)

Una release si pubblica solo quando lo decidi tu: **Actions → Release → Run workflow** (ramo
`main`). Non c'è niente da modificare prima: il numero di versione è già pronto.

- La versione è `MAJOR.MINOR` del `Cargo.toml` (`[workspace.package] version`) più una `PATCH`
  contata dai tag esistenti: nessun tag `v0.1.*` → `v0.1.0`; ultimo tag `v0.1.4` → `v0.1.5`.
- Per iniziare una nuova serie (`0.2.0`, `1.0.0`…) cambia la versione nel `Cargo.toml` e fai
  push; la release successiva parte da `.0`.
- Il numero viene scritto in `Cargo.toml`/`Cargo.lock` solo durante la compilazione: nel
  repository non viene fatto nessun commit.

Il workflow esegue i test e compila macOS Apple silicon (`.dmg`), Linux x86_64 (`.deb` e
`.tar.gz`) e Windows x86_64 (`.zip`), poi pubblica la release `v<versione>` con i pacchetti e
`SHA256SUMS.txt`. Restano solo le due release più recenti: quelle più vecchie vengono rimosse
insieme ai loro tag.

Firma macOS opzionale: con i secret del repository `APPLE_CERTIFICATE` (`.p12` in base64 con
un'identità "Developer ID Application"), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`,
`APPLE_PASSWORD` (password specifica per app) e `APPLE_TEAM_ID`, l'app e il DMG vengono firmati
con il Developer ID e notarizzati; senza, sono firmati ad hoc.

Ogni push e pull request esegue anche la **CI** (`.github/workflows/ci.yml`): clippy, test su
Linux, macOS e Windows, e la suite dei driver su server reali PostgreSQL, MySQL, MariaDB e SQL
Server (service container).

## Architettura

DbCraft è un workspace Cargo con quattro crate:

```
crates/
├── dbgrid       Griglia dati egui virtualizzata (trait GridModel → widget DataGrid → GridEvent)
├── db-core      Tipi Value / ResultSet / schema, trait asincrono Driver, dialetti SQL e generazione DDL
├── db-drivers   SQLite, PostgreSQL, MySQL/MariaDB (sqlx) e SQL Server (tiberius)
└── app          L'applicazione desktop `dbcraft` (eframe)
```

- Le **traduzioni** funzionano come in OpenSlate: il testo italiano nel codice è la chiave e
  `crates/app/src/i18n/<lingua>.json` lo dà nelle altre lingue (i segnaposto come `{n}` o `{}`
  traducono anche i testi composti durante l'esecuzione). Con `DBCRAFT_I18N_MISSING=mancanti.txt`
  i testi mostrati senza traduzione finiscono in quel file.

- L'interfaccia non si blocca mai: le operazioni sul database girano su un runtime **tokio** in
  background, e i risultati tornano tramite un canale che richiede il ridisegno.
- La griglia non sa nulla di database. Legge i dati attraverso il piccolo trait `GridModel` e
  comunica le azioni dell'utente (`CellEdited`, `SortRequested`, `DeleteRows`, …) come eventi:
  è l'app a decidere cosa fare.
- L'SQL che cambia da un motore all'altro (quoting degli identificatori, letterali, paginazione,
  `INSERT`, `UPDATE`, `DELETE`, `CREATE`, `ALTER`) sta in un unico punto, `db_core::Dialect`, ed è
  coperto da test.

## Roadmap

- [x] Test di integrazione su PostgreSQL, MySQL/MariaDB e SQL Server (Docker)
- [x] Autocompletamento SQL (parole chiave, tabelle, colonne)
- [x] Interruzione di una query in esecuzione
- [x] Tunnel SSH (password, chiave privata con passphrase facoltativa)
- [x] Opzioni SSL (verifica CA e nome host, certificati client)
- [x] Tunnel HTTP (script PHP di DbCraft, MySQL/MariaDB e PostgreSQL)
- [x] Set di icone vettoriali proprio
- [x] Interfaccia in nove lingue
- [x] Diagramma ER, backup/ripristino, attività pianificate, utenti e privilegi
- [x] Chat AI per query e database (come in OpenSlate)
- [ ] Sincronizzazione di dati e struttura, BI
- [ ] Altri motori (Oracle, MongoDB, Redis)

Issue e pull request sono benvenute.

## Riconoscimenti

DbCraft si basa sul lavoro di molti progetti open source. Grazie!

- **[GridCraft](https://github.com/storytold/gridcraft)**: il **set di icone** disegnate in codice
  (esteso con icone per database nello stesso stile) e la griglia dati in `crates/dbgrid`
  derivano da GridCraft; la griglia deriva da quella del foglio di calcolo (indice delle linee a somme prefisse,
  geometria con riquadri bloccati, struttura della palette del tema e modello di trascinamento
  per il ridimensionamento delle colonne), adattata ai result set SQL.
  Copyright © 2026 ArtCraft Team and the GridCraft contributors, MIT OR Apache-2.0. Vedi
  [`crates/dbgrid/NOTICE.md`](crates/dbgrid/NOTICE.md).
- **[egui / eframe](https://github.com/emilk/egui)** di Emil Ernerfeldt e contributori: il toolkit
  grafico immediate-mode.
- **[sqlx](https://github.com/launchbadge/sqlx)**: driver per SQLite, PostgreSQL e MySQL.
- **[tiberius](https://github.com/prisma/tiberius)**: driver per SQL Server (TDS).
- **[tokio](https://tokio.rs)**: runtime asincrono.
- **[reqwest](https://github.com/seanmonstar/reqwest)**: client HTTP per il tunnel HTTP.
- **[russh](https://github.com/Eugeny/russh)**: client SSH per il tunnel.
- **[SQLite](https://sqlite.org)** (pubblico dominio), incluso tramite `libsqlite3-sys`.
- **[syntect](https://github.com/trishume/syntect)** (tramite `egui_extras`): evidenziazione SQL.
- **[rust_xlsxwriter](https://github.com/jmcnamara/rust_xlsxwriter)**,
  **[csv](https://github.com/BurntSushi/rust-csv)**, **[serde](https://serde.rs)**: esportazione e importazione.
- **[keyring](https://github.com/hwchen/keyring-rs)**, **[rfd](https://github.com/PolyMeilex/rfd)**,
  **[dirs](https://github.com/soc/dirs-rs)**: portachiavi di sistema, finestre di dialogo native
  per i file e cartelle di configurazione.
- **[sys-locale](https://github.com/1Password/sys-locale)**: la lingua del sistema.
- **[Noto Sans CJK](https://github.com/notofonts/noto-cjk)** (SIL OFL 1.1): un ritaglio minuscolo per
  i nomi delle lingue 中文 日本語 한국어 nel menu, come in OpenSlate.
- **[OpenSlate](https://github.com/rmpaolillo/OpenSlate)**: il sistema delle traduzioni, l'aggiornamento automatico e la
  chat AI (le sue pagine, la catena dei provider e le impostazioni).
- **[wry](https://github.com/tauri-apps/wry)**: le WebView della chat AI dentro la finestra.
- **[KaTeX](https://katex.org)**, **[marked](https://marked.js.org)**, **[DOMPurify](https://github.com/cure53/DOMPurify)**
  e il carattere **[Inter](https://rsms.me/inter/)**, inclusi per le pagine della chat (vedi `crates/app/web/vendor`).

L'elenco completo dei componenti di terze parti e delle loro licenze è in [CREDITS.md](CREDITS.md).

## Licenza

Distribuito, a tua scelta, sotto una delle due licenze:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- licenza MIT ([LICENSE-MIT](LICENSE-MIT))

Salvo diversa indicazione esplicita, ogni contributo che invii intenzionalmente per l'inclusione
in questo progetto, come definito dalla licenza Apache-2.0, è distribuito con la stessa doppia
licenza, senza termini o condizioni aggiuntivi.

## Avvertenze

DbCraft è un progetto indipendente. Non è affiliato, approvato né sponsorizzato da PremiumSoft
CyberTech Ltd. "Navicat" è un marchio del rispettivo proprietario. È citato solo per descrivere il
tipo di strumento che DbCraft è, e DbCraft non ne usa codice, grafica o risorse.
MySQL, MariaDB, PostgreSQL, SQL Server e SQLite sono marchi dei rispettivi proprietari.
