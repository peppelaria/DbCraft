# DbCraft

[![CI](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml/badge.svg)](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rmpaolillo/DbCraft?include_prereleases)](https://github.com/rmpaolillo/DbCraft/releases)

**Un client de bases de données de bureau rapide, natif et open source, écrit en Rust.**

DbCraft apporte le fonctionnement bien connu des interfaces professionnelles de bases de données,
« une seule fenêtre pour toutes vos bases », dans une application native légère : connexions
enregistrées, arborescence des objets, grille de données de type tableur qui reste fluide sur les
grandes tables, éditeur SQL, concepteur visuel de tables et import/export, pour
**MySQL / MariaDB, PostgreSQL, SQL Server et SQLite**.

Il intègre aussi un **assistant IA** : vous lui posez vos questions avec vos mots, et il lit la structure et les données de vos bases, écrit et explique le SQL, ouvre les requêtes dans l'éditeur et ne modifie les données qu'après votre oui. Il fonctionne avec des modèles locaux (LM Studio, Ollama) ou des services dans le cloud (OpenAI, Claude, Gemini…).

*[English](README.md) · [Italiano](README.it.md) · [Español](README.es.md) · Français · [Deutsch](README.de.md) · [Українська](README.uk.md) · [中文](README.zh.md) · [日本語](README.ja.md) · [한국어](README.ko.md)*

![DbCraft : les objets d'une base de données MySQL, avec la barre de menus, la barre d'outils et l'arborescence des connexions](docs/screenshots/objects.png)

> **État : développement précoce (v0.2).** Les quatre moteurs passent la même suite de tests de
> bout en bout sur de vrais serveurs : PostgreSQL 17, MySQL 8.4, MariaDB 11.4, SQL Server 2022 et SQLite.
> Sauvegardez vos données avant de modifier des bases de production.

---

## Fonctionnalités

| Domaine | Ce que vous obtenez |
|---|---|
| **Connexions** | MySQL, MariaDB, PostgreSQL, SQL Server, SQLite. Connexions enregistrées, test de connexion, **SSL/TLS** (vérification du serveur avec une CA et le nom d'hôte, authentification par certificat client), un **tunnel SSH** avec authentification par mot de passe ou par clé privée (avec ou sans passphrase ; la clé de l'hôte est vérifiée dans `~/.ssh/known_hosts`), et un **tunnel HTTP** via le script PHP de DbCraft pour les bases accessibles uniquement depuis un serveur web ([tunnel/README.md](tunnel/README.md)). Les mots de passe sont conservés dans le trousseau du système (Trousseau macOS, Gestionnaire d'identification Windows, Secret Service sous Linux), jamais en clair. |
| **Arborescence des objets** | Connexion → Base de données → Schéma → Tables / Vues / Fonctions / Procédures / Triggers, chargés à la demande, avec menus contextuels. En bas : **Rechercher** (filtre l'arborescence pendant la saisie), **★ seulement les favoris** et **Tout réduire**. |
| **Barre de menus** | Fichier, Édition, Affichage, **Favoris**, Outils, Fenêtre, Aide. Tout objet peut être ajouté aux favoris (clic droit › *Ajouter aux favoris*) ; le menu Favoris l'ouvre, en se connectant d'abord si nécessaire. |
| **Vue des objets** | Une liste filtrable des objets de la base de données ou du schéma sélectionné, comme dans Navicat : nom, auto increment, date de modification, taille des données, engine, lignes et commentaire (les colonnes que fournit le serveur). Les boutons **Table / Vue / Fonction / Autres ▾** de la barre d'outils changent le type d'objet : *Autres* liste les triggers et, selon le serveur, les index, séquences, événements, types, extensions et synonymes. |
| **Requêtes enregistrées** | Le bouton **Requêtes** de la barre d'outils liste les requêtes enregistrées pour la base de données (⌘/Ctrl + S dans un onglet de requête les enregistre là) : ouvrir, nouvelle, supprimer. |
| **Utilisateurs** | Utilisateurs et rôles du serveur avec leurs privilèges ; **Nouvel utilisateur**, **Changer le mot de passe**, **Accorder / Révoquer** des privilèges sur une base de données ou un schéma, **Supprimer l'utilisateur**. Le SQL est affiché avant d'être exécuté (mot de passe masqué). |
| **Sauvegarde / restauration** | Une sauvegarde est un script SQL avec la structure et les données de chaque table (dans l'ordre des clés étrangères) et les vues ; l'onglet **Sauvegarde** les liste, en restaure une (ou n'importe quel fichier `.sql`) et les supprime. Aller-retour testé sur les cinq moteurs. |
| **Automatisation** | Tâches planifiées : une sauvegarde ou du SQL, toutes les *n* minutes, tous les jours ou toutes les semaines à une heure donnée, avec prochaine/dernière exécution et résultat, et **Exécuter maintenant**. Les tâches s'exécutent tant que DbCraft est ouvert. |
| **Modèle** | Le diagramme ER d'une base de données ou d'un schéma : un cadre par table avec ses colonnes (clés primaires et étrangères signalées) et un trait par clé étrangère. Déplacer les cadres, faire défiler, zoomer, **Disposer**, **Exporter SQL** ; double-cliquez sur une table pour la concevoir. |
| **Visualiseur / éditeur de tables** | Grille virtualisée (seules les cellules visibles sont dessinées), pagination côté serveur (100 – 50 000 lignes par page). Panneau **Filtre et tri** : les conditions se lisent comme des phrases de jetons cliquables (champ · opérateur · valeur), chacune reliée à la suivante par son propre *and*/*or*, avec des groupes imbriqués entre parenthèses et 21 opérateurs (=, !=, <…, contient, commence/se termine par et leurs négations, est NULL/vide, est compris entre, est dans la liste, SQL personnalisé), ou un `WHERE` brut ; plus le tri sur plusieurs colonnes. Afficher/masquer les colonnes. **Vues grille, formulaire et texte**. Panneau latéral **Éditeur de cellule** pour les textes longs, le JSON (mise en forme) et le binaire (dump hexadécimal). **Sélecteur de date/heure** sur les colonnes date, time et datetime/timestamp (bouton calendrier sur la cellule active ou Alt+↓, également dans la vue formulaire et l'éditeur de cellule), qui conserve les fractions de seconde et les fuseaux horaires. Modification sur place, mise à NULL, ajout et suppression d'enregistrements (les suppressions demandent confirmation). L'auto-commit enregistre un enregistrement quand vous le quittez ; **Démarrer une transaction** retient chaque modification jusqu'au **Commit** ou au **Rollback**. |
| **Grille de données** | En-têtes de colonne avec type et marqueurs de clé primaire, gouttière des numéros de ligne avec marqueur de l'enregistrement courant, NULL en gris, redimensionnement des colonnes (double-clic pour ajuster automatiquement), colonnes figées, sélection de plusieurs cellules, navigation au clavier, copie en TSV (avec ou sans en-têtes). |
| **Éditeur SQL** | Coloration syntaxique ; **complétion de code** (mots-clés, tables, vues, fonctions et colonnes, aussi après un alias `u.` ; Ctrl+Espace) ; sélecteurs de connexion, de base de données et de schéma dans la barre d'outils ; *Exécuter* tout (⌘/Ctrl + Entrée), la sélection ou l'instruction sous le curseur (⌘/Ctrl + Maj + Entrée) ; **Stop** (annule aussi la requête sur le serveur avec PostgreSQL et MySQL/MariaDB) ; **Explain** (plan d'exécution ; SHOWPLAN sur SQL Server) ; **Embellir le SQL** (⌘/Ctrl + Maj + F) ; **snippets** prêts à l'emploi ; un onglet de résultat par instruction, un journal des messages, Ln/Col dans la barre d'état, ouverture et enregistrement de fichiers `.sql`. |
| **Query Builder** | Conception visuelle de requêtes : faites glisser des tables sur un canevas, cochez des colonnes, faites glisser une colonne sur la colonne d'une autre table pour les joindre (INNER / LEFT / RIGHT / FULL, affichées en courbes) ; les clés étrangères se joignent automatiquement. Agrégats avec GROUP BY automatique, alias, conditions et tri avec le même éditeur à jetons que le filtre de table, DISTINCT et LIMIT (TOP sur SQL Server), aperçu SQL en direct ; **OK** place le SQL dans l'éditeur. |
| **Concepteur de tables** | Champs (nom, type, non null, clé, auto-incrément, valeur par défaut, commentaire), index et clés étrangères, avec un **Aperçu SQL** en direct des instructions `CREATE TABLE` / `ALTER TABLE` dans le dialecte de la connexion. Sous SQLite, les modifications qu'`ALTER` ne peut pas exprimer sont appliquées en reconstruisant la table, en conservant ses données. |
| **Export** | Tables, vues ou résultats de requête vers CSV, JSON, script SQL `INSERT` ou Excel (`.xlsx`). |
| **Import** | CSV avec délimiteur et ligne d'en-tête configurables, aperçu de l'association des champs et option cellule vide → NULL. Les lignes sont insérées par lots dans des transactions. |
| **Chat IA** | Un assistant IA dans un panneau à droite (**Chat IA** dans la barre d'outils), le même que celui d'OpenSlate : il écrit, explique, corrige et optimise le SQL, et travaille sur vos bases de données avec des outils. Il voit les connexions configurées, lit la structure et les données (requêtes en lecture seule), ouvre des requêtes dans un nouvel onglet de l'éditeur et n'effectue des modifications qu'après votre **oui** dans le chat. N'importe quel service compatible OpenAI (LM Studio, Ollama, OpenAI, Anthropic Claude, Google Gemini, DeepSeek, OpenRouter…), essayés dans l'ordre avec repli ; conversations enregistrées, Markdown et formules, un indicateur de contexte avec compactage automatique, et **Réglages IA** (agents, clés dans le trousseau, test, modèles, contexte). |
| **Langues** | L'interface parle **English, Italiano, Español, Français, Deutsch, Українська, 中文, 日本語 et 한국어** : la langue du système par défaut, ou celle choisie dans **Affichage › Langue** (elle s'applique immédiatement). Le chinois, le japonais et le coréen utilisent une police du système. |
| **Interface** | Thèmes clair et sombre. |

| Visualiseur de tables | Éditeur SQL (thème sombre) |
|---|---|
| ![Visualiseur de tables avec le panneau Filtre et tri](docs/screenshots/table-viewer.png) | ![Éditeur SQL avec complétion de code, thème sombre](docs/screenshots/query-editor-dark.png) |
| **Modèle (diagramme ER)** | **Utilisateurs et privilèges** |
| ![Diagramme ER d'une base de données](docs/screenshots/model.png) | ![Utilisateurs d'un serveur MySQL avec leurs privilèges](docs/screenshots/users.png) |
| **Sauvegarde** | **Automatisation** |
| ![Sauvegardes d'une base de données](docs/screenshots/backup.png) | ![Tâches planifiées de sauvegarde et de requête](docs/screenshots/automation.png) |
| **Chat IA** | **Réglages IA** |
| ![Le chat IA : l'assistant a lu les tables, exécuté une requête et l'a ouverte dans l'éditeur](docs/screenshots/ai-chat.png) | ![Réglages IA : agents essayés dans l'ordre, clés, contexte et ce que l'agent peut faire avec les bases](docs/screenshots/ai-settings.png) |

## Prise en main

### Téléchargement

Des paquets précompilés sont disponibles sur la page [Releases](https://github.com/rmpaolillo/DbCraft/releases) :

- **macOS 11+ (Apple silicon)** : `DbCraft-<version>-macos-arm64.dmg`, puis faites glisser DbCraft dans
  Applications. Les builds sans Apple Developer ID sont signés ad hoc : autorisez l'application une fois
  avec clic droit → **Ouvrir**, ou Réglages Système → Confidentialité et sécurité → **Ouvrir quand même**.
- **Debian / Ubuntu** : `sudo apt install ./dbcraft_<version>_amd64.deb`
- **Autres Linux (x86_64, glibc 2.35+)** : `dbcraft-<version>-linux-x86_64.tar.gz`
- **Windows 10/11 (x64)** : `dbcraft-<version>-windows-x86_64.zip` (le runtime Visual C++ est
  inclus) ; SmartScreen peut demander une confirmation.

#### Mises à jour

Une release installée vérifie sur GitHub s'il existe une release plus récente au démarrage et toutes les
12 heures. Si c'est le cas, une barre sous la barre d'outils propose **Mettre à jour** (téléchargement,
vérification SHA-256 avec `SHA256SUMS.txt`, installation à la place de cette copie, puis
**Redémarrer maintenant**), **Nouveautés** (notes de version) et **Ignorer cette version**. L'application
macOS est remplacée sur place (la précédente est conservée dans le dossier de configuration sous
`updates/`), le dossier du `.tar.gz` Linux reçoit les nouveaux fichiers, le `.deb` est installé avec
`pkexec dpkg -i`, et sous Windows un assistant remplace les fichiers à la fermeture de l'application.
Les connexions enregistrées, les paramètres et les mots de passe du trousseau ne sont pas touchés. Le
bouton **Mises à jour** de la barre d'outils vérifie manuellement et active ou désactive l'avis ou
l'installation automatique. Les builds compilés depuis les sources ne se mettent pas à jour eux-mêmes
(`git pull`) ; pour essayer la barre malgré tout, lancez avec
`DBCRAFT_UPDATE_TEST=1 DBCRAFT_PRETEND_VERSION=0.0.1`.

#### Assistant IA

**Chat IA** dans la barre d'outils (ou Outils › Chat IA) ouvre le chat à droite ; **Outils › Réglages IA** ouvre
ses réglages. Ce sont les pages d'[OpenSlate](https://github.com/rmpaolillo/OpenSlate), avec la même
logique : les agents sont essayés **dans l'ordre**, et si l'un ne répond pas (serveur éteint, pas de clé,
erreur), c'est le suivant qui répond. Ils se trouvent dans `ai.toml` dans le dossier de configuration ; une
clé n'y est jamais écrite en clair : `keychain:NAME` (une clé saisie dans la page des réglages va dans le
trousseau du système), `env:NAME` ou `dsh:NAME` (DeepSeek Harness).

Le chat sait quelle base de données est sélectionnée dans DbCraft, et il dispose d'outils :

| Outil | Ce qu'il fait |
|---|---|
| `db_connections`, `db_databases`, `db_tables`, `db_describe` | les connexions configurées et la structure de leurs bases de données |
| `db_query` | exécute du SQL **en lecture seule** (SELECT, WITH, SHOW, EXPLAIN…), au plus `max_rows` lignes |
| `db_open_query` | ouvre une requête dans un nouvel onglet de l'éditeur, sans l'exécuter |
| `db_execute` | modifie les données ou la structure : d'abord il prépare seulement la modification et vous voyez le SQL exact, puis il l'exécute **uniquement après votre oui** dans le message suivant (et seulement si *Réglages IA › La base de données* autorise les modifications) |

Chaque bloc SQL d'une réponse a **Ouvrir dans l'éditeur** et **Copier**. Avec un modèle local (LM Studio,
Ollama), rien ne quitte votre ordinateur ; avec un service cloud, les questions et les données que lit l'IA
sont envoyées à ce service.

### Prérequis

- [Rust](https://rustup.rs) **1.95+** (édition 2024)
- macOS, Windows ou Linux. Linux nécessite les paquets de développement habituels pour une fenêtre
  de bureau (X11/Wayland, `libxkbcommon`) et D-Bus pour le trousseau.

### Compiler et lancer

```sh
git clone https://github.com/rmpaolillo/DbCraft.git
cd DbCraft
cargo run --release                     # lance l'application
cargo run --release -- path/to/file.db  # ouvre directement un fichier SQLite (connexion temporaire)
cargo run --release -- "postgres://user:password@localhost:5432/mydb"  # ou n'importe quel serveur par URL
```

Schémas d'URL : `postgres://`, `mysql://`, `mariadb://`, `sqlserver://` (ou `mssql://`) et
`sqlite://`. Encodez en pourcentage les caractères spéciaux du mot de passe (par exemple `@` → `%40`).
Ajoutez `?ssh=user:password@ssh-host:port` pour passer par un tunnel SSH, ou
`?ssh=user@ssh-host&ssh_key=~/.ssh/id_ed25519` pour l'authentification par clé.

Pour créer une connexion, cliquez sur **Connexion** dans la barre d'outils et choisissez le type de base de données.

### Options de ligne de commande

| Option | Description |
|---|---|
| `<file.db>` ou `<url>` | Ouvre un fichier SQLite ou une URL de serveur comme connexion temporaire, non enregistrée |
| `--open-table <name>` | Ouvre une table de ce fichier une fois celui-ci chargé |
| `--query "<sql>"` | Ouvre un onglet de requête avec ce SQL et l'exécute |
| `--dark` | Démarre avec le thème sombre |
| `--lang <code>` | Langue de l'interface pour cette exécution : `en`, `it`, `es`, `fr`, `de`, `uk`, `zh`, `ja`, `ko` |
| `--open <tool>` | Ouvre un outil une fois la base de données chargée : `users`, `backup`, `automation`, `model`, `queries`, `chat`, `ai-settings`, `others:<kind>` |
| `--new-connection <kind> [--ssh-tab]` | Ouvre la boîte de dialogue de nouvelle connexion (`mysql`, `postgres`, …), éventuellement sur son onglet SSH |
| `--screenshot <out.png> [--delay <s>]` | Enregistre un PNG de la fenêtre après `s` secondes (3 par défaut), puis quitte. Utilisé pour les vérifications automatiques de l'interface. |

Les connexions enregistrées se trouvent dans le dossier de configuration du système
(`dbcraft/connections.json`), par exemple `~/Library/Application Support/dbcraft/` sous macOS, à côté de
`settings.json`, `favorites.json`, `automation.json` et des dossiers `queries/` et `backups/`. Définissez
`DBCRAFT_CONFIG_DIR` pour utiliser un autre dossier.

### Tests

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Tests d'intégration des pilotes sur de vrais serveurs (Docker) :

```sh
scripts/test-certs.sh && mkdir -p target/ssh-test   # certificats de test, dossier des clés SSH
docker compose -f docker-compose.test.yml up -d --wait
DBCRAFT_TEST_SERVERS=1 cargo test -p db-drivers --test servers --test ssh --test tls --test http -- --test-threads=1
DBCRAFT_TEST_SERVERS=1 cargo test -p dbcraft -- --test-threads=1   # sauvegarde/restauration, utilisateurs et catalogues
docker compose -f docker-compose.test.yml down -v
```

La suite crée des tables avec index et clés étrangères, exécute des scripts à plusieurs instructions,
vérifie l'introspection, applique et annule des modifications transactionnelles, modifie des tables via
le SQL du concepteur et décode les types de données de chaque moteur (timestamps avec fuseau horaire,
UUID, JSON, tableaux, money, entiers 64 bits non signés, XML, …). Les tests SSH passent par un conteneur
de serveur OpenSSH avec mot de passe, clés RSA et clés Ed25519 protégées par passphrase ; les tests SSL
utilisent PostgreSQL et MySQL avec des certificats de test (verify-full, certificats client, un
utilisateur MySQL `REQUIRE X509`). L'image SQL Server est uniquement amd64 et s'exécute en émulation sur
Apple Silicon.

### Publier une release (mainteneurs)

Une release n'est publiée que lorsque vous le décidez : **Actions → Release → Run workflow** (branche
`main`). Il n'y a rien à modifier au préalable : le numéro de version est prêt.

- La version est `MAJOR.MINOR` tiré de `Cargo.toml` (`[workspace.package] version`) plus un `PATCH`
  calculé à partir des tags existants : aucun tag `v0.1.*` encore → `v0.1.0` ; dernier tag `v0.1.4` → `v0.1.5`.
- Pour commencer une nouvelle série (`0.2.0`, `1.0.0`…), changez la version dans `Cargo.toml` et poussez ;
  la release suivante commence à `.0`.
- Le numéro n'est écrit dans `Cargo.toml`/`Cargo.lock` qu'à l'intérieur du build : rien n'est
  recommité dans le dépôt.

Le workflow teste et compile macOS Apple silicon (`.dmg`), Linux x86_64 (`.deb` et `.tar.gz`)
et Windows x86_64 (`.zip`), puis publie la release `v<version>` avec les paquets et
`SHA256SUMS.txt`. Seules les deux releases les plus récentes sont conservées : les releases plus
anciennes et leurs tags sont supprimés.

Signature macOS facultative : avec les secrets du dépôt `APPLE_CERTIFICATE` (`.p12` en base64 avec une
identité « Developer ID Application »), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`
(mot de passe spécifique à l'application) et `APPLE_TEAM_ID`, l'application et le DMG sont signés avec le
Developer ID et notariés ; sans eux, ils sont signés ad hoc.

Chaque push et pull request exécute aussi la **CI** (`.github/workflows/ci.yml`) : clippy, tests sous
Linux, macOS et Windows, et la suite des pilotes sur de vrais conteneurs de service PostgreSQL, MySQL,
MariaDB et SQL Server.

## Architecture

DbCraft est un workspace Cargo composé de quatre crates :

```
crates/
├── dbgrid       Grille de données egui virtualisée (trait GridModel → widget DataGrid → GridEvent)
├── db-core      Types Value / ResultSet / schéma, le trait async Driver, dialectes SQL et génération DDL
├── db-drivers   SQLite, PostgreSQL, MySQL/MariaDB (sqlx) et SQL Server (tiberius)
└── app          L'application de bureau `dbcraft` (eframe)
```

- **Les traductions** fonctionnent comme dans OpenSlate : le texte italien dans le code est la clé, et
  `crates/app/src/i18n/<lang>.json` le donne dans les autres langues (des placeholders comme
  `{n}` ou `{}` permettent aussi de traduire les textes construits à l'exécution). Lancez avec
  `DBCRAFT_I18N_MISSING=missing.txt` pour recueillir les textes affichés sans traduction.

- L'interface ne se bloque jamais : le travail sur les bases de données s'exécute dans un runtime
  **tokio** en arrière-plan, et les résultats reviennent par un canal et déclenchent un nouveau rendu.
- La grille ne sait rien des bases de données. Elle lit via le petit trait `GridModel` et signale
  les intentions de l'utilisateur (`CellEdited`, `SortRequested`, `DeleteRows`, …) sous forme
  d'événements, de sorte que l'application décide quoi faire.
- Le SQL qui diffère d'un moteur à l'autre (délimitation des identifiants, littéraux, pagination,
  `INSERT`, `UPDATE`, `DELETE`, `CREATE`, `ALTER`) se trouve en un seul endroit, `db_core::Dialect`,
  et est couvert par des tests unitaires.

## Feuille de route

- [x] Tests d'intégration sur PostgreSQL, MySQL/MariaDB et SQL Server (Docker)
- [x] Autocomplétion SQL (mots-clés, tables, colonnes)
- [x] Annulation d'une requête en cours
- [x] Tunnel SSH (mot de passe, clé privée avec passphrase facultative)
- [x] Options SSL (vérification de la CA, vérification du nom d'hôte, certificats client)
- [x] Tunnel HTTP (script PHP de DbCraft, MySQL/MariaDB et PostgreSQL)
- [x] Jeu d'icônes vectorielles personnalisé
- [x] Interface en neuf langues
- [x] Diagramme ER, sauvegarde/restauration, tâches planifiées, utilisateurs et privilèges
- [x] Chat IA pour les requêtes et les bases de données (comme dans OpenSlate)
- [ ] Synchronisation des données et de la structure, BI
- [ ] Davantage de moteurs (Oracle, MongoDB, Redis)

Les issues et pull requests sont les bienvenues.

## Remerciements

DbCraft repose sur le travail de nombreux projets open source. Merci !

- **[GridCraft](https://github.com/storytold/gridcraft)** : le **jeu d'icônes** dessiné en code (étendu
  avec des icônes de bases de données dans le même style) et la grille de données de `crates/dbgrid`
  sont dérivés de GridCraft (la grille : index de lignes par sommes préfixes, géométrie avec volets figés,
  disposition de la palette et modèle de glissement pour le redimensionnement des colonnes, adaptés aux
  jeux de résultats SQL).
  Copyright © 2026 ArtCraft Team et les contributeurs de GridCraft, MIT OR Apache-2.0. Voir
  [`crates/dbgrid/NOTICE.md`](crates/dbgrid/NOTICE.md).
- **[egui / eframe](https://github.com/emilk/egui)** par Emil Ernerfeldt et ses contributeurs : la
  boîte à outils d'interface en mode immédiat.
- **[sqlx](https://github.com/launchbadge/sqlx)** : pilotes SQLite, PostgreSQL et MySQL.
- **[tiberius](https://github.com/prisma/tiberius)** : pilote SQL Server (TDS).
- **[tokio](https://tokio.rs)** : runtime asynchrone.
- **[reqwest](https://github.com/seanmonstar/reqwest)** : client HTTP pour le tunnel HTTP.
- **[russh](https://github.com/Eugeny/russh)** : client SSH pour le tunnel.
- **[SQLite](https://sqlite.org)** (domaine public), intégré via `libsqlite3-sys`.
- **[syntect](https://github.com/trishume/syntect)** (via `egui_extras`) : coloration syntaxique SQL.
- **[rust_xlsxwriter](https://github.com/jmcnamara/rust_xlsxwriter)**,
  **[csv](https://github.com/BurntSushi/rust-csv)**, **[serde](https://serde.rs)** : export et import.
- **[keyring](https://github.com/hwchen/keyring-rs)**, **[rfd](https://github.com/PolyMeilex/rfd)**,
  **[dirs](https://github.com/soc/dirs-rs)** : trousseau du système, boîtes de dialogue de fichiers natives et chemins de configuration.
- **[sys-locale](https://github.com/1Password/sys-locale)** : la langue du système.
- **[Noto Sans CJK](https://github.com/notofonts/noto-cjk)** (SIL OFL 1.1) : un tout petit sous-ensemble
  pour les noms de langues 中文 日本語 한국어 dans le menu, comme dans OpenSlate.
- **[OpenSlate](https://github.com/rmpaolillo/OpenSlate)** : le système de traduction, la mise à jour automatique et le chat
  IA (ses pages, la chaîne des fournisseurs et les réglages).
- **[wry](https://github.com/tauri-apps/wry)** : les WebViews du chat IA dans la fenêtre.
- **[KaTeX](https://katex.org)**, **[marked](https://marked.js.org)**, **[DOMPurify](https://github.com/cure53/DOMPurify)**
  et la police **[Inter](https://rsms.me/inter/)**, intégrés pour les pages du chat (voir `crates/app/web/vendor`).

La liste complète des composants tiers et de leurs licences se trouve dans [CREDITS.md](CREDITS.md).

## Licence

Distribué sous l'une ou l'autre des licences suivantes

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

à votre choix.

Sauf indication contraire explicite de votre part, toute contribution que vous soumettez
intentionnellement pour inclusion dans ce travail, telle que définie dans la licence Apache-2.0, est
distribuée sous la double licence ci-dessus, sans termes ni conditions supplémentaires.

## Avertissement

DbCraft est un projet indépendant. Il n'est ni affilié à PremiumSoft CyberTech Ltd., ni approuvé ou
sponsorisé par celle-ci. « Navicat » est une marque de son propriétaire. Elle n'est mentionnée que pour
décrire le type d'outil qu'est DbCraft, et DbCraft n'utilise aucun de ses codes, visuels ou ressources.
MySQL, MariaDB, PostgreSQL, SQL Server et SQLite sont des marques de leurs propriétaires respectifs.
