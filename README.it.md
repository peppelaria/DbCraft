# DbCraft

[![CI](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml/badge.svg)](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rmpaolillo/DbCraft?include_prereleases)](https://github.com/rmpaolillo/DbCraft/releases)

**Client database desktop nativo, veloce e open source, scritto in Rust.**

DbCraft porta in un'app nativa e leggera il flusso di lavoro dei client database professionali
("una finestra per tutti i tuoi database"): connessioni salvate, albero degli oggetti, una griglia
dati in stile foglio di calcolo che resta fluida anche su tabelle grandi, editor SQL, designer
visuale delle tabelle e import/export, per **MySQL / MariaDB, PostgreSQL, SQL Server e SQLite**.

🇬🇧 [Read in English](README.md)

![Visualizzatore tabelle](docs/screenshots/table-viewer.png)

> **Stato: sviluppo iniziale (v0.1).** Tutti i motori superano la stessa suite di test
> end-to-end su server reali: PostgreSQL 17, MySQL 8.4, MariaDB 11.4, SQL Server 2022 e SQLite.
> Fai un backup prima di modificare database di produzione.

---

## Funzionalità

| Area | Cosa offre |
|---|---|
| **Connessioni** | MySQL, MariaDB, PostgreSQL, SQL Server, SQLite. Connessioni salvate, prova connessione, SSL/TLS opzionale. Le password stanno nel portachiavi del sistema operativo (Portachiavi di macOS, Gestione credenziali di Windows, Secret Service su Linux), mai in chiaro. |
| **Albero oggetti** | Connessione → Database → Schema → Tabelle / Viste / Funzioni / Procedure / Trigger, caricati su richiesta, con menu contestuali. |
| **Vista Oggetti** | Elenco ricercabile degli oggetti del database o schema selezionato, con numero di righe e commenti. |
| **Visualizzatore / editor tabelle** | Griglia virtualizzata (disegna solo le celle visibili), paginazione lato server (da 100 a 50.000 righe per pagina), filtro `WHERE`, ordinamento lato server, "filtra per questo valore". Modifica nella cella, imposta NULL o stringa vuota, aggiungi ed elimina record. Le modifiche restano in sospeso (evidenziate) finché non le **applichi**: vengono allora salvate in un'unica transazione. In alternativa puoi **annullarle**. |
| **Griglia dati** | Intestazioni con tipo e indicatore di chiave primaria, numeri di riga con indicatore del record corrente, NULL in grigio, ridimensionamento colonne (doppio clic per adattare), colonne bloccate, selezione multipla, navigazione da tastiera, copia in formato TSV (con o senza intestazioni). |
| **Editor SQL** | Evidenziazione della sintassi, *Esegui* (⌘/Ctrl + Invio) ed *Esegui selezione*, più istruzioni con un tab di risultato per ciascuna, log dei messaggi con righe interessate e tempi, apertura e salvataggio di file `.sql`. |
| **Designer tabelle** | Campi (nome, tipo, non null, chiave, auto-incremento, predefinito, commento), indici e chiavi esterne, con **anteprima SQL** in tempo reale delle istruzioni `CREATE TABLE` / `ALTER TABLE` nel dialetto della connessione. Su SQLite, le modifiche non esprimibili con `ALTER` vengono applicate ricostruendo la tabella, senza perdere i dati. |
| **Esportazione** | Tabelle, viste o risultati di query in CSV, JSON, script SQL `INSERT` o Excel (`.xlsx`). |
| **Importazione** | CSV con delimitatore e riga di intestazione configurabili, anteprima e associazione dei campi, opzione "cella vuota = NULL". Le righe vengono inserite a blocchi dentro transazioni. |
| **Interfaccia** | Tema chiaro e scuro. L'interfaccia per ora è in italiano. |

| Oggetti | Editor SQL (tema scuro) |
|---|---|
| ![Oggetti](docs/screenshots/objects.png) | ![Editor SQL](docs/screenshots/query-editor-dark.png) |

## Per iniziare

### Download

I pacchetti già compilati sono nella pagina [Releases](https://github.com/rmpaolillo/DbCraft/releases):
`.app` universale per macOS 11+, Windows x64 e Linux x64/arm64 (glibc 2.35+). Le build non sono
notarizzate: su macOS, la prima volta apri l'app con clic destro → Apri (oppure esegui
`xattr -dr com.apple.quarantine /Applications/DbCraft.app`); su Windows, SmartScreen potrebbe
chiedere conferma.

### Requisiti

- [Rust](https://rustup.rs) **1.95+** (edition 2024)
- macOS, Windows o Linux. Su Linux servono i pacchetti di sviluppo per le finestre desktop
  (X11/Wayland, `libxkbcommon`) e D-Bus per il portachiavi.

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

Per creare una connessione, clicca **Connessione** nella barra in alto e scegli il tipo di database.

### Opzioni da riga di comando

| Opzione | Descrizione |
|---|---|
| `<file.db>` o `<url>` | Apre un file SQLite o un server tramite URL come connessione temporanea, non salvata |
| `--open-table <nome>` | Apre una tabella di quel file appena è caricata |
| `--query "<sql>"` | Apre un tab query con questo SQL e lo esegue |
| `--dark` | Avvia con il tema scuro |
| `--screenshot <out.png> [--delay <s>]` | Salva un PNG della finestra dopo `s` secondi (predefinito 3), poi esce. Serve per le verifiche automatiche dell'interfaccia. |

Le connessioni salvate stanno nella cartella di configurazione del sistema
(`dbcraft/connections.json`), per esempio `~/Library/Application Support/dbcraft/` su macOS.
Imposta `DBCRAFT_CONFIG_DIR` per usare un'altra cartella.

### Test

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Test di integrazione dei driver su server reali (Docker):

```sh
docker compose -f docker-compose.test.yml up -d --wait
DBCRAFT_TEST_SERVERS=1 cargo test -p db-drivers --test servers -- --test-threads=1
docker compose -f docker-compose.test.yml down -v
```

La suite crea tabelle con indici e chiavi esterne, esegue script con più istruzioni, verifica
l'introspezione, applica e annulla modifiche in transazione, modifica le tabelle con l'SQL del
designer e controlla la decodifica dei tipi di ogni motore (timestamp con fuso orario, UUID,
JSON, array, money, interi senza segno a 64 bit, XML, …). L'immagine di SQL Server è solo amd64 e
su Apple Silicon gira in emulazione.

### Pubblicare una release (manutentori)

Le release vengono compilate da GitHub Actions e si avviano **a mano**:

1. Aggiorna `version` in `[workspace.package]` nel `Cargo.toml`, fai commit e push.
2. Apri **Actions → Release → Run workflow**. Scegli il branch, indica facoltativamente la
   versione (deve coincidere con `Cargo.toml`) e scegli *pre-release* o *draft*.
3. Il workflow esegue clippy e i test, compila macOS (universale), Windows x64, Linux x64 e
   Linux arm64, poi crea il tag `v<versione>` e la release GitHub con i pacchetti e
   `SHA256SUMS.txt`.

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
- [ ] Autocompletamento SQL (parole chiave, tabelle, colonne)
- [ ] Interruzione di una query in esecuzione
- [ ] Tunnel SSH e più opzioni SSL
- [ ] Set di icone vettoriali proprio
- [ ] Interfaccia in inglese / localizzazione
- [ ] Diagramma ER, sincronizzazione di dati e struttura, backup/ripristino
- [ ] Altri motori (Oracle, MongoDB, Redis)

Issue e pull request sono benvenute.

## Riconoscimenti

DbCraft si basa sul lavoro di molti progetti open source. Grazie!

- **[GridCraft](https://github.com/storytold/gridcraft)**: la griglia dati in `crates/dbgrid`
  deriva dalla griglia del foglio di calcolo di GridCraft (indice delle linee a somme prefisse,
  geometria con riquadri bloccati, struttura della palette del tema e modello di trascinamento
  per il ridimensionamento delle colonne), adattata ai result set SQL.
  Copyright © 2026 ArtCraft Team and the GridCraft contributors, MIT OR Apache-2.0. Vedi
  [`crates/dbgrid/NOTICE.md`](crates/dbgrid/NOTICE.md).
- **[egui / eframe](https://github.com/emilk/egui)** di Emil Ernerfeldt e contributori: il toolkit
  grafico immediate-mode.
- **[sqlx](https://github.com/launchbadge/sqlx)**: driver per SQLite, PostgreSQL e MySQL.
- **[tiberius](https://github.com/prisma/tiberius)**: driver per SQL Server (TDS).
- **[tokio](https://tokio.rs)**: runtime asincrono.
- **[SQLite](https://sqlite.org)** (pubblico dominio), incluso tramite `libsqlite3-sys`.
- **[syntect](https://github.com/trishume/syntect)** (tramite `egui_extras`): evidenziazione SQL.
- **[rust_xlsxwriter](https://github.com/jmcnamara/rust_xlsxwriter)**,
  **[csv](https://github.com/BurntSushi/rust-csv)**, **[serde](https://serde.rs)**: esportazione e importazione.
- **[keyring](https://github.com/hwchen/keyring-rs)**, **[rfd](https://github.com/PolyMeilex/rfd)**,
  **[dirs](https://github.com/soc/dirs-rs)**: portachiavi di sistema, finestre di dialogo native
  per i file e cartelle di configurazione.

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
