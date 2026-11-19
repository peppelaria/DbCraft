<?php
/*
 * DbCraft HTTP tunnel — https://github.com/rmpaolillo/DbCraft
 * Licensed under MIT OR Apache-2.0.
 *
 * Upload this file to a PHP web server (PHP 7.4+, with the PDO MySQL and/or PostgreSQL
 * extensions) that can reach your database, then use its URL in DbCraft's connection dialog
 * (HTTP tab). DbCraft posts SQL statements as JSON; the script runs them with PDO and returns the
 * results as JSON. The database user name and password travel with every request: serve this
 * script over HTTPS.
 *
 * SECURITY — adjust the settings below (or set the environment variables of the same name):
 *   DBCRAFT_TUNNEL_HOSTS     database hosts the script may connect to (comma separated).
 *                            Default: localhost and 127.0.0.1 only, so the script cannot be
 *                            used to reach other machines.
 *   DBCRAFT_TUNNEL_TOKEN     if set, requests must carry it (DbCraft: HTTP tab → Token).
 *   DBCRAFT_TUNNEL_USER /    if set, requests must use HTTP Basic authentication with these
 *   DBCRAFT_TUNNEL_PASSWORD  credentials (you can also protect the URL in the web server).
 *   DBCRAFT_TUNNEL_MAX_ROWS  rows returned per result at most (default 100000).
 */

$SETTINGS = [
    'DBCRAFT_TUNNEL_HOSTS' => 'localhost,127.0.0.1',
    'DBCRAFT_TUNNEL_TOKEN' => '',
    'DBCRAFT_TUNNEL_USER' => '',
    'DBCRAFT_TUNNEL_PASSWORD' => '',
    'DBCRAFT_TUNNEL_MAX_ROWS' => '100000',
];

const DBCRAFT_TUNNEL_VERSION = 1;

function setting(string $name): string
{
    global $SETTINGS;
    $env = getenv($name);
    return ($env !== false && $env !== '') ? $env : (string)($SETTINGS[$name] ?? '');
}

function reply(int $status, array $body): void
{
    http_response_code($status);
    header('Content-Type: application/json; charset=utf-8');
    header('Cache-Control: no-store');
    echo json_encode($body, JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_PRESERVE_ZERO_FRACTION);
    exit;
}

function fail(int $status, string $message): void
{
    reply($status, ['ok' => false, 'error' => $message, 'version' => DBCRAFT_TUNNEL_VERSION]);
}

/** Column type name, lower case, from PDO's metadata. */
function type_name(array $meta, string $driver): string
{
    $t = strtolower((string)($meta['native_type'] ?? ''));
    if ($driver === 'mysql') {
        $map = [
            'tiny' => 'tinyint', 'short' => 'smallint', 'int24' => 'mediumint', 'long' => 'int',
            'longlong' => 'bigint', 'newdecimal' => 'decimal', 'var_string' => 'varchar',
            'string' => 'char', 'blob' => 'blob', 'double' => 'double', 'float' => 'float',
        ];
        $t = $map[$t] ?? $t;
    }
    return $t;
}

function convert($v, string $type)
{
    if (is_resource($v)) {
        $v = stream_get_contents($v); // PostgreSQL bytea
    }
    if ($v === null || is_bool($v) || is_int($v) || is_float($v)) {
        return $v;
    }
    $v = (string)$v;
    $int_types = ['tinyint', 'smallint', 'mediumint', 'int', 'bigint', 'int2', 'int4', 'int8', 'year'];
    $float_types = ['double', 'float', 'float4', 'float8'];
    if (in_array($type, $int_types, true) && preg_match('/^-?\d{1,18}$/', $v)) {
        return (int)$v;
    }
    if (in_array($type, $float_types, true) && is_numeric($v)) {
        return (float)$v;
    }
    if ($type === 'bool' && ($v === 't' || $v === 'f')) {
        return $v === 't';
    }
    if (!preg_match('//u', $v)) {
        return ['$b64' => base64_encode($v)]; // binary data
    }
    return $v;
}

function connect(array $db): PDO
{
    $driver = $db['driver'] ?? '';
    if (!in_array($driver, ['mysql', 'pgsql'], true)) {
        fail(400, 'Unsupported database driver');
    }
    if (!extension_loaded('pdo_' . $driver)) {
        fail(500, "The PHP extension pdo_$driver is not installed on this server");
    }
    $host = (string)($db['host'] ?? 'localhost');
    $allowed = array_filter(array_map('trim', explode(',', setting('DBCRAFT_TUNNEL_HOSTS'))));
    if (!in_array('*', $allowed, true) && !in_array(strtolower($host), array_map('strtolower', $allowed), true)) {
        fail(403, "Database host '$host' is not allowed by DBCRAFT_TUNNEL_HOSTS on the tunnel");
    }
    $port = (int)($db['port'] ?? 0);
    $name = (string)($db['database'] ?? '');
    $dsn = "$driver:host=$host" . ($port > 0 ? ";port=$port" : '') . ($name !== '' ? ";dbname=$name" : '');
    if ($driver === 'mysql') {
        $dsn .= ';charset=utf8mb4';
    } elseif ($name === '') {
        $dsn .= ';dbname=postgres';
    }
    try {
        return new PDO($dsn, (string)($db['user'] ?? ''), (string)($db['password'] ?? ''), [
            PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION,
            PDO::ATTR_DEFAULT_FETCH_MODE => PDO::FETCH_NUM,
            PDO::ATTR_STRINGIFY_FETCHES => false,
        ]);
    } catch (PDOException $e) {
        fail(200, 'Connection: ' . $e->getMessage());
    }
}

// ----- request -----

if (($_SERVER['REQUEST_METHOD'] ?? 'GET') !== 'POST') {
    reply(200, ['ok' => true, 'service' => 'DbCraft HTTP tunnel', 'version' => DBCRAFT_TUNNEL_VERSION,
        'drivers' => array_values(array_intersect(['mysql', 'pgsql'], PDO::getAvailableDrivers()))]);
}

$user = setting('DBCRAFT_TUNNEL_USER');
if ($user !== '') {
    $u = $_SERVER['PHP_AUTH_USER'] ?? '';
    $p = $_SERVER['PHP_AUTH_PW'] ?? '';
    if (!hash_equals($user, $u) || !hash_equals(setting('DBCRAFT_TUNNEL_PASSWORD'), $p)) {
        header('WWW-Authenticate: Basic realm="DbCraft tunnel"');
        fail(401, 'HTTP authentication failed');
    }
}
$token = setting('DBCRAFT_TUNNEL_TOKEN');
if ($token !== '' && !hash_equals($token, (string)($_SERVER['HTTP_X_DBCRAFT_TOKEN'] ?? ''))) {
    fail(403, 'Invalid tunnel token');
}

$req = json_decode((string)file_get_contents('php://input'), true);
if (!is_array($req)) {
    fail(400, 'Invalid JSON request');
}
$action = (string)($req['action'] ?? '');
$db = is_array($req['db'] ?? null) ? $req['db'] : [];
$statements = is_array($req['statements'] ?? null) ? $req['statements'] : [];
if (!empty($req['base64'])) {
    $statements = array_map(function ($s) {
        $d = base64_decode((string)$s, true);
        if ($d === false) {
            fail(400, 'Invalid base64 statement');
        }
        return $d;
    }, $statements);
}
$max_rows = max(1, (int)setting('DBCRAFT_TUNNEL_MAX_ROWS'));
if (isset($req['max_rows'])) {
    $max_rows = max(1, min($max_rows, (int)$req['max_rows']));
}

$pdo = connect($db);
$driver = $db['driver'];

if ($action === 'ping') {
    reply(200, ['ok' => true, 'version' => DBCRAFT_TUNNEL_VERSION, 'server' => (string)$pdo->getAttribute(PDO::ATTR_SERVER_VERSION)]);
}

if ($action === 'query') {
    $results = [];
    foreach ($statements as $sql) {
        $started = microtime(true);
        try {
            $st = $pdo->query((string)$sql);
            $count = $st->columnCount();
            if ($count === 0) {
                $results[] = ['columns' => [], 'rows' => [], 'affected' => $st->rowCount(), 'ms' => (microtime(true) - $started) * 1000];
                continue;
            }
            $cols = [];
            $types = [];
            for ($i = 0; $i < $count; $i++) {
                $meta = $st->getColumnMeta($i) ?: [];
                $types[$i] = type_name($meta, $driver);
                $cols[] = ['name' => (string)($meta['name'] ?? "col$i"), 'type' => $types[$i]];
            }
            $rows = [];
            $truncated = false;
            while (($row = $st->fetch(PDO::FETCH_NUM)) !== false) {
                if (count($rows) >= $max_rows) {
                    $truncated = true;
                    break;
                }
                foreach ($row as $i => $v) {
                    $row[$i] = convert($v, $types[$i] ?? '');
                }
                $rows[] = $row;
            }
            $st->closeCursor();
            $results[] = ['columns' => $cols, 'rows' => $rows, 'truncated' => $truncated, 'ms' => (microtime(true) - $started) * 1000];
        } catch (PDOException $e) {
            reply(200, ['ok' => false, 'error' => $e->getMessage(), 'results' => $results, 'failed_sql' => (string)$sql]);
        }
    }
    reply(200, ['ok' => true, 'results' => $results]);
}

if ($action === 'apply') {
    $affected = 0;
    try {
        $pdo->beginTransaction();
        foreach ($statements as $sql) {
            $n = (int)$pdo->exec((string)$sql);
            // every change is about one record: more rows means a key that is not unique
            if ($n > 1) {
                $pdo->rollBack();
                reply(200, ['ok' => false, 'error' => "$n righe interessate invece di una, nessuna modifica salvata: $sql"]);
            }
            $affected += $n;
        }
        $pdo->commit();
    } catch (PDOException $e) {
        if ($pdo->inTransaction()) {
            $pdo->rollBack();
        }
        reply(200, ['ok' => false, 'error' => $e->getMessage()]);
    }
    reply(200, ['ok' => true, 'affected' => $affected]);
}

fail(400, 'Unknown action');
