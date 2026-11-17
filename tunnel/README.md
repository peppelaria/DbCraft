# DbCraft HTTP tunnel

`dbcraft-tunnel.php` lets DbCraft reach a MySQL/MariaDB or PostgreSQL database that is only
accessible from a web server (shared hosting, firewalled networks). DbCraft sends SQL as JSON over
HTTP(S); the script runs it with PDO and returns the results.

## Install

1. Copy `dbcraft-tunnel.php` to the web server (DbCraft can save it for you: connection dialog →
   **HTTP** → *Salva script del tunnel*). Requirements: PHP 7.4+ with `pdo_mysql` and/or
   `pdo_pgsql`.
2. Edit the settings at the top of the file, or set them as environment variables:

   | Setting | Default | Meaning |
   |---|---|---|
   | `DBCRAFT_TUNNEL_HOSTS` | `localhost,127.0.0.1` | Database hosts the script may connect to (`*` = any; not recommended) |
   | `DBCRAFT_TUNNEL_TOKEN` | — | Secret every request must carry (DbCraft: *Token*) |
   | `DBCRAFT_TUNNEL_USER` / `DBCRAFT_TUNNEL_PASSWORD` | — | HTTP Basic credentials required by the script |
   | `DBCRAFT_TUNNEL_MAX_ROWS` | `100000` | Rows returned per result at most |

3. Open the URL in a browser: it answers with the PHP database drivers it can use.
4. In DbCraft, create the connection with the database host **as seen from the web server**
   (usually `localhost`), enable *Usa tunnel HTTP* and enter the script URL.

## Security

- Serve the script over **HTTPS**: database credentials travel with each request.
- Set a **token** (and/or HTTP authentication) so that only you can use the script.
- Keep `DBCRAFT_TUNNEL_HOSTS` as narrow as possible, so the script cannot be used to reach other
  machines from your server.

## Protocol

`POST` with a JSON body `{"v": 1, "action": "ping" | "query" | "apply", "db": {driver, host, port,
user, password, database}, "statements": [...], "base64": bool, "max_rows": n}`. `query` returns one
result per statement (`columns`, `rows`, `affected`); `apply` runs the statements in a transaction.
Binary values are returned as `{"$b64": "..."}`.

This script is part of DbCraft and is not related to, or compatible with, other products' tunnel
scripts. License: MIT OR Apache-2.0.
