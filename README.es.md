<p align="center"><img src="packaging/icon/dbcraft.svg" width="128" height="128" alt="DbCraft"></p>

# DbCraft

[![CI](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml/badge.svg)](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rmpaolillo/DbCraft?include_prereleases)](https://github.com/rmpaolillo/DbCraft/releases)

**Un cliente de bases de datos de escritorio rápido, nativo y de código abierto, escrito en Rust.**

DbCraft lleva el conocido flujo de trabajo de «una sola ventana para todas tus bases de datos» de las
interfaces gráficas profesionales a una aplicación nativa y ligera: conexiones guardadas, un árbol de objetos,
una cuadrícula de datos al estilo de una hoja de cálculo que sigue siendo fluida con tablas grandes, un editor SQL,
un diseñador visual de tablas e importación/exportación, para **MySQL / MariaDB, PostgreSQL, SQL Server y SQLite**.

También incluye un **asistente de IA**: le preguntas con tus palabras y lee la estructura y los datos de tus bases de datos, escribe y explica SQL, abre las consultas en el editor y solo modifica datos después de tu sí. Funciona con modelos locales (LM Studio, Ollama) o con servicios en la nube (OpenAI, Claude, Gemini…).

*[English](README.md) · [Italiano](README.it.md) · Español · [Français](README.fr.md) · [Deutsch](README.de.md) · [Українська](README.uk.md) · [中文](README.zh.md) · [日本語](README.ja.md) · [한국어](README.ko.md)*

![DbCraft: los objetos de una base de datos MySQL, con la barra de menús, la barra de herramientas y el árbol de conexiones](docs/screenshots/objects.png)

> **Estado: desarrollo inicial (v0.2).** Los cuatro motores superan el mismo conjunto de pruebas de extremo a extremo
> contra servidores reales: PostgreSQL 17, MySQL 8.4, MariaDB 11.4, SQL Server 2022 y SQLite.
> Haz una copia de seguridad de tus datos antes de editar bases de datos de producción.

---

## Funciones

| Área | Qué ofrece |
|---|---|
| **Conexiones** | MySQL, MariaDB, PostgreSQL, SQL Server, SQLite. Conexiones guardadas, prueba de conexión, **SSL/TLS** (verificación del servidor con una CA y el nombre de host, autenticación con certificado de cliente), un **túnel SSH** con autenticación por contraseña o clave privada (con o sin frase de contraseña; la clave del host se comprueba con `~/.ssh/known_hosts`) y un **túnel HTTP** a través del script PHP de DbCraft para bases de datos accesibles solo desde un servidor web ([tunnel/README.md](tunnel/README.md)). Las contraseñas se guardan en el llavero del sistema operativo (Llavero de macOS, Administrador de credenciales de Windows, Secret Service en Linux), nunca en texto plano. |
| **Árbol de objetos** | Conexión → Base de datos → Esquema → Tablas / Vistas / Funciones / Procedimientos / Triggers, cargados bajo demanda, con menús contextuales. En la parte inferior: **buscar** (filtra el árbol mientras escribes), **★ solo favoritos** y **contraer todo**. |
| **Barra de menús** | Archivo, Editar, Ver, **Favoritos**, Herramientas, Ventana, Ayuda. Cualquier objeto se puede añadir a favoritos (clic derecho › *Añadir a favoritos*); el menú Favoritos lo abre, conectándose primero si hace falta. |
| **Vista de objetos** | Una lista con búsqueda de los objetos de la base de datos o el esquema seleccionado, como en Navicat: nombre, auto increment, fecha de modificación, tamaño de datos, engine, filas y comentario (las columnas que proporciona el servidor). Los botones **Tabla / Vista / Función / Más ▾** de la barra de herramientas cambian el tipo de objeto: *Más* muestra los triggers y, según el servidor, índices, secuencias, eventos, tipos, extensiones y sinónimos. |
| **Consultas guardadas** | El botón **Consulta** de la barra de herramientas muestra las consultas guardadas para la base de datos (⌘/Ctrl + S en una pestaña de consulta las guarda ahí): abrir, nueva, eliminar. |
| **Usuarios** | Usuarios y roles del servidor con sus privilegios; **nuevo usuario**, **cambiar contraseña**, **conceder / revocar** privilegios sobre una base de datos o un esquema, **eliminar usuario**. El SQL se muestra antes de ejecutarse (con la contraseña oculta). |
| **Backup / restauración** | Un backup es un script SQL con la estructura y los datos de cada tabla (en el orden de las claves foráneas) y las vistas; la pestaña **Backup** los lista, restaura uno (o cualquier archivo `.sql`) y los elimina. Ida y vuelta probada en los cinco motores. |
| **Automatización** | Tareas programadas: un backup o algo de SQL, cada *n* minutos, cada día o cada semana a una hora dada, con próxima/última ejecución y resultado, y **Ejecutar ahora**. Las tareas se ejecutan mientras DbCraft está abierto. |
| **Modelo** | El diagrama ER de una base de datos o un esquema: un cuadro por tabla con sus columnas (claves primarias y foráneas marcadas) y una línea por clave foránea. Arrastra los cuadros, desplaza, haz zoom, **Organizar**, **Exportar SQL**; doble clic en una tabla para diseñarla. |
| **Visor / editor de tablas** | Cuadrícula virtualizada (solo se dibujan las celdas visibles), paginación en el servidor (de 100 a 50.000 filas por página). Panel **Filtro y orden**: las condiciones se leen como frases de elementos en los que se puede hacer clic (campo · operador · valor), cada una unida a la siguiente por su propio *and*/*or*, con grupos anidados entre paréntesis y 21 operadores (=, !=, <…, contiene, empieza por/termina en y sus negaciones, es NULL/vacío, entre, en la lista, SQL personalizado), o un `WHERE` escrito a mano; además de ordenación por varias columnas. Mostrar/ocultar columnas. **Vistas de cuadrícula, formulario y texto**. Panel lateral **Editor de celda** para texto largo, JSON (con formato) y binario (volcado hexadecimal). **Selector de fecha/hora** en columnas de fecha, hora y datetime/timestamp (botón de calendario en la celda activa o Alt+↓, también en la vista de formulario y en el editor de celda), conservando las fracciones de segundo y las zonas horarias. Edición en el sitio, establecer a NULL, añadir y eliminar registros (las eliminaciones piden confirmación). El auto-commit guarda un registro al salir de él; **Iniciar transacción** retiene todos los cambios hasta **Commit** o **Rollback**. |
| **Cuadrícula de datos** | Encabezados de columna con marcadores de tipo y de clave primaria, margen con números de fila y marcador del registro actual, NULL en gris, cambio de ancho de columnas (doble clic para ajuste automático), columnas fijas, selección de varias celdas, navegación con el teclado, copia como TSV (con o sin encabezados). |
| **Editor SQL** | Resaltado de sintaxis; **autocompletado** (palabras clave, tablas, vistas, funciones y columnas, también tras un alias `u.`; Ctrl+Espacio); selectores de conexión, base de datos y esquema en la barra de herramientas; *Ejecutar* todo (⌘/Ctrl + Enter), la selección o la sentencia en el cursor (⌘/Ctrl + Shift + Enter); **Detener** (también cancela la consulta en el servidor con PostgreSQL y MySQL/MariaDB); **Explain** (plan de ejecución; SHOWPLAN en SQL Server); **Embellecer SQL** (⌘/Ctrl + Shift + F); **fragmentos** predefinidos; una pestaña de resultados por sentencia, un registro de mensajes, Lín/Col en la barra de estado, abrir y guardar archivos `.sql`. |
| **Query Builder** | Diseño visual de consultas: arrastra tablas a un lienzo, marca columnas, arrastra una columna sobre la columna de otra tabla para unirlas (INNER / LEFT / RIGHT / FULL, mostradas como curvas); las claves foráneas se unen automáticamente. Agregados con GROUP BY automático, alias, condiciones y ordenación con el mismo editor de elementos que el filtro de tablas, DISTINCT y LIMIT (TOP en SQL Server), vista previa del SQL en directo; **Aceptar** coloca el SQL en el editor. |
| **Diseñador de tablas** | Campos (nombre, tipo, no nulo, clave, auto incremento, predeterminado, comentario), índices y claves foráneas, con una **Vista previa SQL** en directo de las sentencias `CREATE TABLE` / `ALTER TABLE` en el dialecto de la conexión. En SQLite, los cambios que no se pueden expresar con `ALTER` se aplican reconstruyendo la tabla, conservando sus datos. |
| **Exportar** | Tablas, vistas o resultados de consultas a CSV, JSON, script SQL `INSERT` o Excel (`.xlsx`). |
| **Importar** | CSV con delimitador y fila de encabezados configurables, una vista previa de la asociación de campos y conversión opcional de celdas vacías → NULL. Las filas se insertan por lotes dentro de transacciones. |
| **Chat IA** | Un asistente de IA en un panel a la derecha (**Chat IA** en la barra de herramientas), el mismo que el de OpenSlate: escribe, explica, corrige y optimiza SQL, y trabaja con tus bases de datos mediante herramientas. Ve las conexiones configuradas, lee la estructura y los datos (consultas de solo lectura), abre consultas en una nueva pestaña del editor y hace cambios solo después de tu **sí** en el chat. Cualquier servicio compatible con OpenAI (LM Studio, Ollama, OpenAI, Anthropic Claude, Google Gemini, DeepSeek, OpenRouter…), probados en orden con alternativa; conversaciones guardadas, Markdown y fórmulas, un indicador de contexto con compactación automática, y **Ajustes de IA** (agentes, claves en el llavero, prueba, modelos, contexto). |
| **Idiomas** | La interfaz habla **English, Italiano, Español, Français, Deutsch, Українська, 中文, 日本語 y 한국어**: por defecto el idioma del sistema, o el elegido en **Ver › Idioma** (se aplica al instante). El chino, el japonés y el coreano usan una fuente del sistema. |
| **Interfaz** | Temas claro y oscuro. |

| Visor de tablas | Editor SQL (tema oscuro) |
|---|---|
| ![Visor de tablas con el panel Filtro y orden](docs/screenshots/table-viewer.png) | ![Editor SQL con autocompletado, tema oscuro](docs/screenshots/query-editor-dark.png) |
| **Modelo (diagrama ER)** | **Usuarios y privilegios** |
| ![Diagrama ER de una base de datos](docs/screenshots/model.png) | ![Usuarios de un servidor MySQL con sus privilegios](docs/screenshots/users.png) |
| **Backup** | **Automatización** |
| ![Backups de una base de datos](docs/screenshots/backup.png) | ![Tareas programadas de backup y de consulta](docs/screenshots/automation.png) |
| **Chat de IA** | **Ajustes de IA** |
| ![El chat de IA: el asistente leyó las tablas, ejecutó una consulta y la abrió en el editor](docs/screenshots/ai-chat.png) | ![Ajustes de IA: agentes probados en orden, claves, contexto y lo que el agente puede hacer con las bases de datos](docs/screenshots/ai-settings.png) |

## Primeros pasos

### Descarga

Los paquetes precompilados están en la página de [Releases](https://github.com/rmpaolillo/DbCraft/releases):

- **macOS 11+ (Apple silicon)**: `DbCraft-<version>-macos-arm64.dmg`, y luego arrastra DbCraft a
  Aplicaciones. Las compilaciones sin un Apple Developer ID se firman ad hoc: permite la aplicación una vez con
  clic derecho → **Abrir**, o en Ajustes del Sistema → Privacidad y seguridad → **Abrir igualmente**.
- **Debian / Ubuntu**: `sudo apt install ./dbcraft_<version>_amd64.deb`
- **Otras distribuciones Linux (x86_64, glibc 2.35+)**: `dbcraft-<version>-linux-x86_64.tar.gz`
- **Windows 10/11 (x64)**: `dbcraft-<version>-windows-x86_64.zip` (incluye el runtime de Visual C++);
  SmartScreen puede pedir confirmación.

#### Actualizaciones

Una versión instalada busca en GitHub una versión más reciente al iniciarse y cada 12 horas. Cuando
la hay, una barra bajo la barra de herramientas ofrece **Actualizar ahora** (descarga, comprobación SHA-256 con
`SHA256SUMS.txt`, instalación en lugar de esta copia y luego **Reiniciar ahora**), **Novedades** (notas
de la versión) y **Omitir esta versión**. La aplicación de macOS se reemplaza en el sitio (la anterior se conserva
en la carpeta de configuración, en `updates/`), la carpeta del `.tar.gz` de Linux recibe los archivos nuevos, el
`.deb` se instala con `pkexec dpkg -i`, y en Windows un asistente reemplaza los archivos cuando se cierra la
aplicación. Las conexiones guardadas, la configuración y las contraseñas del llavero no se tocan. El botón
**Actualizaciones** de la barra de herramientas comprueba manualmente y activa o desactiva el aviso o la instalación automática.
Las compilaciones desde el código fuente no se actualizan solas (`git pull`); para probar la barra de todos modos, ejecuta con
`DBCRAFT_UPDATE_TEST=1 DBCRAFT_PRETEND_VERSION=0.0.1`.

#### Asistente de IA

**Chat IA** en la barra de herramientas (o Herramientas › Chat IA) abre el chat a la derecha; **Herramientas › Ajustes de IA** abre
su configuración. Ambas son las páginas de [OpenSlate](https://github.com/rmpaolillo/OpenSlate), con la misma
lógica: los agentes se prueban **en orden**, y si uno no responde (servidor apagado, sin clave, error) responde
el siguiente. Se guardan en `ai.toml` en la carpeta de configuración; nunca se escribe ahí una clave en claro:
`keychain:NAME` (una clave escrita en la página de ajustes va al llavero del sistema operativo), `env:NAME` o
`dsh:NAME` (DeepSeek Harness).

El chat sabe qué base de datos está seleccionada en DbCraft y dispone de herramientas:

| Herramienta | Qué hace |
|---|---|
| `db_connections`, `db_databases`, `db_tables`, `db_describe` | las conexiones configuradas y la estructura de sus bases de datos |
| `db_query` | ejecuta SQL de **solo lectura** (SELECT, WITH, SHOW, EXPLAIN…), como máximo `max_rows` filas |
| `db_open_query` | abre una consulta en una nueva pestaña del editor, sin ejecutarla |
| `db_execute` | modifica datos o estructura: primero solo prepara el cambio y ves el SQL exacto, luego lo ejecuta **solo después de tu sí** en el siguiente mensaje (y solo si *Ajustes de IA › La base de datos* permite cambios) |

Cada bloque SQL de una respuesta tiene **Abrir en el editor** y **Copiar**. Con un modelo local (LM Studio,
Ollama) nada sale de tu ordenador; con un servicio en la nube, las preguntas y los datos que lee la IA
van a ese servicio.

### Requisitos

- [Rust](https://rustup.rs) **1.95+** (edition 2024)
- macOS, Windows o Linux. Linux necesita los paquetes de desarrollo habituales para una ventana de escritorio
  (X11/Wayland, `libxkbcommon`) y D-Bus para el llavero.

### Compilar y ejecutar

```sh
git clone https://github.com/rmpaolillo/DbCraft.git
cd DbCraft
cargo run --release                     # inicia la aplicación
cargo run --release -- path/to/file.db  # abre directamente un archivo SQLite (conexión temporal)
cargo run --release -- "postgres://user:password@localhost:5432/mydb"  # o cualquier servidor por URL
```

Esquemas de URL: `postgres://`, `mysql://`, `mariadb://`, `sqlserver://` (o `mssql://`) y
`sqlite://`. Codifica con porcentajes los caracteres especiales de la contraseña (por ejemplo `@` → `%40`).
Añade `?ssh=user:password@ssh-host:port` para pasar por un túnel SSH, o
`?ssh=user@ssh-host&ssh_key=~/.ssh/id_ed25519` para la autenticación con clave.

Para crear una conexión, haz clic en **Conexión** en la barra de herramientas y elige el tipo de base de datos.

### Opciones de línea de comandos

| Opción | Descripción |
|---|---|
| `<file.db>` o `<url>` | Abre un archivo SQLite o una URL de servidor como conexión temporal, sin guardar |
| `--open-table <name>` | Abre una tabla de ese archivo una vez cargado |
| `--query "<sql>"` | Abre una pestaña de consulta con este SQL y lo ejecuta |
| `--dark` | Inicia con el tema oscuro |
| `--lang <code>` | Idioma de la interfaz para esta ejecución: `en`, `it`, `es`, `fr`, `de`, `uk`, `zh`, `ja`, `ko` |
| `--open <tool>` | Abre una herramienta una vez cargada la base de datos: `users`, `backup`, `automation`, `model`, `queries`, `chat`, `ai-settings`, `others:<kind>` |
| `--new-connection <kind> [--ssh-tab]` | Abre el diálogo de nueva conexión (`mysql`, `postgres`, …), opcionalmente en su pestaña SSH |
| `--screenshot <out.png> [--delay <s>]` | Guarda un PNG de la ventana tras `s` segundos (3 por defecto) y sale. Se usa para comprobaciones automáticas de la interfaz. |

Las conexiones guardadas residen en el directorio de configuración del sistema operativo (`dbcraft/connections.json`), por ejemplo
`~/Library/Application Support/dbcraft/` en macOS, junto a `settings.json`, `favorites.json`,
`automation.json` y las carpetas `queries/` y `backups/`. Define `DBCRAFT_CONFIG_DIR` para usar
otra carpeta.

### Pruebas

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Pruebas de integración de los drivers contra servidores reales (Docker):

```sh
scripts/test-certs.sh && mkdir -p target/ssh-test   # certificados de prueba, carpeta de claves SSH
docker compose -f docker-compose.test.yml up -d --wait
DBCRAFT_TEST_SERVERS=1 cargo test -p db-drivers --test servers --test ssh --test tls --test http -- --test-threads=1
DBCRAFT_TEST_SERVERS=1 cargo test -p dbcraft -- --test-threads=1   # backup/restauración, usuarios y catálogos
docker compose -f docker-compose.test.yml down -v
```

El conjunto de pruebas crea tablas con índices y claves foráneas, ejecuta scripts de varias sentencias, comprueba
la introspección, aplica y revierte ediciones transaccionales, modifica tablas mediante el SQL del diseñador
y decodifica los tipos de datos de cada motor (timestamps con zona horaria, UUID, JSON, arrays,
money, enteros de 64 bits sin signo, XML, …). Las pruebas SSH pasan por un contenedor con un servidor OpenSSH con
contraseña, claves RSA y Ed25519 protegidas con frase de contraseña; las pruebas SSL usan PostgreSQL y MySQL
con certificados de prueba (verify-full, certificados de cliente, un usuario MySQL `REQUIRE X509`). La imagen de SQL Server es solo amd64 y se ejecuta con emulación en
Apple Silicon.

### Publicar versiones (mantenedores)

Una versión se publica solo cuando tú lo decides: **Actions → Release → Run workflow** (rama
`main`). No hay que editar nada antes: el número de versión ya está listo.

- La versión es `MAJOR.MINOR` de `Cargo.toml` (`[workspace.package] version`) más un `PATCH`
  calculado a partir de las etiquetas existentes: aún no hay ninguna etiqueta `v0.1.*` → `v0.1.0`; última etiqueta `v0.1.4` → `v0.1.5`.
- Para empezar una nueva serie (`0.2.0`, `1.0.0`…) cambia la versión en `Cargo.toml` y haz push; la
  siguiente versión empieza en `.0`.
- El número se escribe en `Cargo.toml`/`Cargo.lock` solo dentro de la compilación: no se
  confirma nada de vuelta en el repositorio.

El workflow prueba y compila macOS Apple silicon (`.dmg`), Linux x86_64 (`.deb` y `.tar.gz`)
y Windows x86_64 (`.zip`), y luego publica la versión `v<version>` con los paquetes y
`SHA256SUMS.txt`. Solo se conservan las dos versiones más recientes: las versiones anteriores y sus etiquetas se
eliminan.

Firma opcional para macOS: con los secretos del repositorio `APPLE_CERTIFICATE` (`.p12` en base64 con una
identidad "Developer ID Application"), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`
(contraseña específica de la aplicación) y `APPLE_TEAM_ID`, la aplicación y el DMG se firman con el Developer ID
y se notarizan; sin ellos se firman ad hoc.

Cada push y pull request ejecuta también la **CI** (`.github/workflows/ci.yml`): clippy, pruebas en
Linux, macOS y Windows, y el conjunto de pruebas de drivers contra contenedores de servicio reales de PostgreSQL, MySQL, MariaDB y SQL
Server.

## Arquitectura

DbCraft es un workspace de Cargo con cuatro crates:

```
crates/
├── dbgrid       Cuadrícula de datos egui virtualizada (trait GridModel → widget DataGrid → GridEvent)
├── db-core      Tipos Value / ResultSet / esquema, el trait asíncrono Driver, dialectos SQL y generación de DDL
├── db-drivers   SQLite, PostgreSQL, MySQL/MariaDB (sqlx) y SQL Server (tiberius)
└── app          La aplicación de escritorio `dbcraft` (eframe)
```

- **Las traducciones** funcionan como en OpenSlate: el texto italiano del código es la clave, y
  `crates/app/src/i18n/<lang>.json` lo da en los demás idiomas (los marcadores como
  `{n}` o `{}` permiten traducir también los textos construidos en tiempo de ejecución). Ejecuta con
  `DBCRAFT_I18N_MISSING=missing.txt` para recopilar los textos que se muestran sin traducción.

- La interfaz nunca se bloquea: el trabajo con la base de datos se ejecuta en un runtime **tokio** en segundo plano, y los resultados
  vuelven por un canal y provocan un redibujado.
- La cuadrícula no sabe nada de bases de datos. Lee a través del pequeño trait `GridModel` y
  comunica las intenciones del usuario (`CellEdited`, `SortRequested`, `DeleteRows`, …) como eventos, de modo que la aplicación
  decide qué hacer.
- El SQL que difiere entre motores (entrecomillado de identificadores, literales, paginación, `INSERT`, `UPDATE`,
  `DELETE`, `CREATE`, `ALTER`) está en un único lugar, `db_core::Dialect`, y tiene pruebas unitarias.

## Hoja de ruta

- [x] Pruebas de integración contra PostgreSQL, MySQL/MariaDB y SQL Server (Docker)
- [x] Autocompletado SQL (palabras clave, tablas, columnas)
- [x] Cancelar una consulta en ejecución
- [x] Túnel SSH (contraseña, clave privada con frase de contraseña opcional)
- [x] Opciones SSL (verificación de la CA, verificación del nombre de host, certificados de cliente)
- [x] Túnel HTTP (script PHP de DbCraft, MySQL/MariaDB y PostgreSQL)
- [x] Conjunto de iconos vectoriales propio
- [x] Interfaz en nueve idiomas
- [x] Diagrama ER, backup/restauración, tareas programadas, usuarios y privilegios
- [x] Chat IA para consultas y bases de datos (como en OpenSlate)
- [ ] Sincronización de datos y estructura, BI
- [ ] Más motores (Oracle, MongoDB, Redis)

Los issues y las pull requests son bienvenidos.

## Créditos

DbCraft se basa en el trabajo de muchos proyectos de código abierto. ¡Gracias!

- **[GridCraft](https://github.com/storytold/gridcraft)**: el **conjunto de iconos** dibujados por código (ampliado
  con iconos de bases de datos en el mismo estilo) y la cuadrícula de datos de `crates/dbgrid`
  derivan de GridCraft (la cuadrícula: índice de líneas por sumas de prefijos, geometría con paneles fijos, disposición
  de la paleta y modelo de arrastre para cambiar el ancho de las columnas, adaptados a los conjuntos de resultados SQL).
  Copyright © 2026 ArtCraft Team y los colaboradores de GridCraft, MIT OR Apache-2.0. Consulta
  [`crates/dbgrid/NOTICE.md`](crates/dbgrid/NOTICE.md).
- **[egui / eframe](https://github.com/emilk/egui)** de Emil Ernerfeldt y colaboradores: el
  toolkit de interfaz gráfica en modo inmediato.
- **[sqlx](https://github.com/launchbadge/sqlx)**: drivers de SQLite, PostgreSQL y MySQL.
- **[tiberius](https://github.com/prisma/tiberius)**: driver de SQL Server (TDS).
- **[tokio](https://tokio.rs)**: runtime asíncrono.
- **[reqwest](https://github.com/seanmonstar/reqwest)**: cliente HTTP para el túnel HTTP.
- **[russh](https://github.com/Eugeny/russh)**: cliente SSH para el túnel.
- **[SQLite](https://sqlite.org)** (dominio público), incluido a través de `libsqlite3-sys`.
- **[syntect](https://github.com/trishume/syntect)** (vía `egui_extras`): resaltado de sintaxis SQL.
- **[rust_xlsxwriter](https://github.com/jmcnamara/rust_xlsxwriter)**,
  **[csv](https://github.com/BurntSushi/rust-csv)**, **[serde](https://serde.rs)**: exportación e importación.
- **[keyring](https://github.com/hwchen/keyring-rs)**, **[rfd](https://github.com/PolyMeilex/rfd)**,
  **[dirs](https://github.com/soc/dirs-rs)**: llavero del sistema, diálogos de archivo nativos y rutas de configuración.
- **[sys-locale](https://github.com/1Password/sys-locale)**: el idioma del sistema.
- **[Noto Sans CJK](https://github.com/notofonts/noto-cjk)** (SIL OFL 1.1): un subconjunto mínimo para los
  nombres de idioma 中文 日本語 한국어 del menú, como en OpenSlate.
- **[OpenSlate](https://github.com/rmpaolillo/OpenSlate)**: el sistema de traducción, la actualización automática y el chat
  de IA (sus páginas, la cadena de proveedores y los ajustes).
- **[wry](https://github.com/tauri-apps/wry)**: las WebViews del chat de IA dentro de la ventana.
- **[KaTeX](https://katex.org)**, **[marked](https://marked.js.org)**, **[DOMPurify](https://github.com/cure53/DOMPurify)**
  y la fuente **[Inter](https://rsms.me/inter/)**, incluidos para las páginas del chat (consulta `crates/app/web/vendor`).

La lista completa de componentes de terceros y sus licencias está en [CREDITS.md](CREDITS.md).

## Licencia

Distribuido bajo cualquiera de estas licencias

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

a tu elección.

Salvo que indiques expresamente lo contrario, cualquier contribución que envíes intencionadamente para su inclusión en
esta obra, tal como se define en la licencia Apache-2.0, tendrá la doble licencia indicada arriba, sin
términos ni condiciones adicionales.

## Aviso legal

DbCraft es un proyecto independiente. No está afiliado a PremiumSoft CyberTech Ltd., ni cuenta con su respaldo ni con su
patrocinio. "Navicat" es una marca comercial de su propietario. Se menciona solo para
describir el tipo de herramienta que es DbCraft, y DbCraft no utiliza nada de su código, sus gráficos ni sus recursos.
MySQL, MariaDB, PostgreSQL, SQL Server y SQLite son marcas comerciales de sus respectivos propietarios.
