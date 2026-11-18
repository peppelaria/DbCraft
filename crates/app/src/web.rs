//! The pages of the AI chat and of «Impostazioni AI», as in OpenSlate: a tiny HTTP server on
//! 127.0.0.1 (a random port) that the WebViews inside DbCraft's window load (`embed`).
//!
//! - `/chat`, `/impostazioni`        the pages (with `wc.css`, `i18n.js`, `vendor/…`)
//! - `GET /api/config`, `POST /api/config`, `/api/config/test`, `/api/config/models`, `/api/compact`
//! - `GET /api/providers`            the state of the agents of `ai.toml`
//! - `GET /api/context`              the database selected in DbCraft
//! - `POST /api/chat`                the answer, streamed (Server-Sent Events)
//! - `/api/chats`, `/api/chats/<id>` saved conversations (GET reads, POST saves, `/delete`, `/restore`)
//!
//! Security, as in OpenSlate: only this computer (loopback), only names of this computer (no «DNS
//! rebinding»), and no request from another site (Origin / Sec-Fetch-Site).

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const CHAT_HTML: &str = include_str!("../web/chat.html");
const SETTINGS_HTML: &str = include_str!("../web/impostazioni.html");
/// Colours, fonts and controls in the Craft style, shared by the pages.
const WC_CSS: &str = include_str!("../web/wc.css");
const I18N_JS: &str = include_str!("../web/i18n.js");

/// Largest POST body (the chat also sends pasted images).
const MAX_BODY: usize = 40 << 20;

static PORT: OnceLock<u16> = OnceLock::new();
static WAKE: OnceLock<egui::Context> = OnceLock::new();

/// `http://127.0.0.1:PORT/` once the server runs.
pub fn origin() -> Option<String> {
    PORT.get().map(|p| format!("http://127.0.0.1:{p}/"))
}

/// The window, to wake it when the chat does something in the app.
pub fn wake() -> Option<&'static egui::Context> {
    WAKE.get()
}

/// Starts the server on `rt` (once).
pub fn start(rt: &tokio::runtime::Handle, ctx: &egui::Context) {
    let _ = WAKE.set(ctx.clone());
    if PORT.get().is_some() {
        return;
    }
    let listener = rt.block_on(async { TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await });
    let listener = match listener {
        Ok(l) => l,
        Err(e) => {
            eprintln!("chat AI: server locale non avviato: {e}");
            return;
        }
    };
    let Ok(port) = listener.local_addr().map(|a| a.port()) else { return };
    let _ = PORT.set(port);
    rt.spawn(async move {
        loop {
            match listener.accept().await {
                Ok((sock, _)) => {
                    tokio::spawn(async move {
                        let _ = handle(sock).await;
                    });
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
            }
        }
    });
}

/// A header of the request.
fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

/// The server name asked is one of this computer (IP or localhost)? A site resolved to
/// 127.0.0.1 («DNS rebinding») sends its own name.
fn host_ok(host: Option<&str>) -> bool {
    let Some(h) = host else { return true };
    let name = if let Some(rest) = h.strip_prefix('[') { rest.split(']').next().unwrap_or("") } else { h.rsplit_once(':').map(|(n, _)| n).unwrap_or(h) };
    let name = name.to_ascii_lowercase();
    name == "localhost" || name.parse::<IpAddr>().is_ok()
}

/// A request from a page of another site (Origin different from this server) must not write here.
fn origin_ok(origin: Option<&str>, host: Option<&str>) -> bool {
    match (origin, host) {
        (None, _) => true,
        (Some(o), Some(h)) => o.eq_ignore_ascii_case(&format!("http://{h}")),
        (Some(_), None) => false,
    }
}

async fn handle(mut sock: TcpStream) -> std::io::Result<()> {
    if !sock.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false) {
        return Ok(());
    }
    // the header of the request (up to the empty line, at most 16 KB)
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = tokio::time::timeout(Duration::from_secs(10), sock.read(&mut chunk)).await.map_err(|_| std::io::Error::other("timeout"))??;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 16384 {
            return respond(&mut sock, "431 Request Header Fields Too Large", "text/plain", b"intestazione troppo lunga").await;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut parts = head.lines().next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let path = target.split_once('?').map_or(target.as_str(), |(p, _)| p).to_string();
    let path = path.as_str();

    let host = header(&head, "host");
    if !host_ok(host) {
        return respond(&mut sock, "421 Misdirected Request", "text/plain", b"nome del server sconosciuto").await;
    }
    if method != "GET" && method != "HEAD" && !origin_ok(header(&head, "origin"), host) {
        return respond(&mut sock, "403 Forbidden", "text/plain", b"richiesta da un altro sito").await;
    }
    if path.starts_with("/api/") && header(&head, "sec-fetch-site").is_some_and(|s| s.eq_ignore_ascii_case("cross-site")) {
        return respond(&mut sock, "403 Forbidden", "text/plain", b"richiesta da un altro sito").await;
    }

    if method == "POST" {
        if header(&head, "transfer-encoding").is_some() {
            return respond(&mut sock, "411 Length Required", "text/plain", b"serve Content-Length").await;
        }
        let len = header(&head, "content-length").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        if len > MAX_BODY {
            return respond(&mut sock, "413 Payload Too Large", "text/plain", b"troppo grande").await;
        }
        let mut body = buf[head_end..].to_vec();
        while body.len() < len {
            let n = tokio::time::timeout(Duration::from_secs(20), sock.read(&mut chunk)).await.map_err(|_| std::io::Error::other("timeout"))??;
            if n == 0 {
                return Ok(());
            }
            body.extend_from_slice(&chunk[..n]);
        }
        body.truncate(len);
        return match path {
            "/api/config" | "/api/config/test" | "/api/config/models" | "/api/compact" => {
                let path = path.to_string();
                let out = tokio::task::spawn_blocking(move || match path.as_str() {
                    "/api/config" => crate::aiconfig::save_from_json(&body),
                    "/api/config/test" => crate::aiconfig::test_from_json(&body),
                    "/api/config/models" => crate::aiconfig::models_from_json(&body),
                    _ => crate::aiconfig::compact_from_json(&body),
                })
                .await
                .unwrap_or_else(|e| serde_json::json!({ "ok": false, "error": e.to_string() }));
                respond(&mut sock, "200 OK", "application/json", out.to_string().as_bytes()).await
            }
            "/api/chat" => chat(sock, &body).await,
            p if p.starts_with("/api/chats/") => {
                let rest = &p["/api/chats/".len()..];
                let (id, op) = rest.split_once('/').unwrap_or((rest, ""));
                let out = match op {
                    "" => crate::chats::save(id, &body).map(|_| serde_json::json!({ "ok": true })),
                    "delete" => crate::chats::trash(id).map(|_| serde_json::json!({ "ok": true })),
                    "restore" => crate::chats::restore(id).map(|_| serde_json::json!({ "ok": true })),
                    _ => Err("operazione sconosciuta".into()),
                };
                let out = out.unwrap_or_else(|e| serde_json::json!({ "ok": false, "error": e }));
                respond(&mut sock, "200 OK", "application/json", out.to_string().as_bytes()).await
            }
            _ => respond(&mut sock, "404 Not Found", "text/plain", b"non trovato").await,
        };
    }
    if method != "GET" && method != "HEAD" {
        return respond(&mut sock, "405 Method Not Allowed", "text/plain", b"solo GET e POST").await;
    }
    match path {
        "/" | "/chat" | "/chat.html" => respond(&mut sock, "200 OK", "text/html; charset=utf-8", CHAT_HTML.as_bytes()).await,
        "/impostazioni" | "/impostazioni.html" => respond(&mut sock, "200 OK", "text/html; charset=utf-8", SETTINGS_HTML.as_bytes()).await,
        "/wc.css" => respond(&mut sock, "200 OK", "text/css; charset=utf-8", WC_CSS.as_bytes()).await,
        // the interface language for the pages: table + translator
        "/i18n.js" => {
            let js = format!("window.I18N_LANG={:?};window.I18N={};\n{I18N_JS}", crate::i18n::lang().code(), crate::i18n::table_json());
            respond(&mut sock, "200 OK", "text/javascript; charset=utf-8", js.as_bytes()).await
        }
        p if p.starts_with("/vendor/") => match crate::vendor::get(&p["/vendor/".len()..]) {
            Some((ctype, body)) => respond_cached(&mut sock, ctype, body).await,
            None => respond(&mut sock, "404 Not Found", "text/plain", b"non trovato").await,
        },
        "/favicon.ico" => respond(&mut sock, "204 No Content", "image/x-icon", b"").await,
        "/api/context" => {
            let j = match crate::ai::context() {
                Some(c) => serde_json::json!({ "title": c.title, "conn": c.conn, "db": c.db, "schema": c.schema, "rev": c.rev }),
                None => serde_json::json!({ "title": "", "rev": 0 }),
            };
            respond(&mut sock, "200 OK", "application/json", j.to_string().as_bytes()).await
        }
        "/api/chats" => {
            let j = tokio::task::spawn_blocking(crate::chats::list).await.unwrap_or_default();
            respond(&mut sock, "200 OK", "application/json", j.to_string().as_bytes()).await
        }
        p if p.starts_with("/api/chats/") => match crate::chats::get(&p["/api/chats/".len()..]) {
            Some(b) => respond(&mut sock, "200 OK", "application/json", &b).await,
            None => respond(&mut sock, "404 Not Found", "application/json", b"{}").await,
        },
        "/api/config" => {
            let j = tokio::task::spawn_blocking(crate::aiconfig::to_json).await.unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() }));
            respond(&mut sock, "200 OK", "application/json", j.to_string().as_bytes()).await
        }
        "/api/providers" => {
            let j = match tokio::task::spawn_blocking(crate::ai::AiConfig::load).await {
                Ok(Ok(cfg)) => serde_json::json!({
                    "providers": crate::ai::describe(&cfg).into_iter().map(|(n, s)| serde_json::json!({ "name": n, "state": s })).collect::<Vec<_>>()
                }),
                Ok(Err(e)) => serde_json::json!({ "error": e.to_string() }),
                Err(e) => serde_json::json!({ "error": e.to_string() }),
            };
            respond(&mut sock, "200 OK", "application/json", j.to_string().as_bytes()).await
        }
        _ => respond(&mut sock, "404 Not Found", "text/plain", b"non trovato").await,
    }
}

/// Files that do not change (KaTeX, marked): the browser keeps them.
async fn respond_cached(sock: &mut TcpStream, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    let head = format!("HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: public, max-age=604800\r\nConnection: close\r\n\r\n", body.len());
    sock.write_all(head.as_bytes()).await?;
    sock.write_all(body).await?;
    sock.flush().await
}

async fn respond(sock: &mut TcpStream, status: &str, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    let head = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", body.len());
    sock.write_all(head.as_bytes()).await?;
    sock.write_all(body).await?;
    sock.flush().await
}

/// Writes a file whole or not at all (temporary file, then renamed).
pub fn write_atomic(f: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    let dir = f.parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, f)
}

fn data_url_bytes(u: &str) -> Option<Arc<Vec<u8>>> {
    use base64::Engine;
    let (head, b64) = u.split_once(',')?;
    if !head.starts_with("data:image/") || !head.ends_with(";base64") || b64.len() > 16 * 1024 * 1024 {
        return None;
    }
    base64::engine::general_purpose::STANDARD.decode(b64.trim()).ok().map(Arc::new)
}

async fn chat(mut sock: TcpStream, body: &[u8]) -> std::io::Result<()> {
    #[derive(serde::Deserialize)]
    struct In {
        role: String,
        content: String,
        /// Pasted images (data URL `data:image/png;base64,…`).
        #[serde(default)]
        images: Vec<String>,
    }
    #[derive(serde::Deserialize)]
    struct Req {
        messages: Vec<In>,
        /// The database tools.
        #[serde(default = "si", deserialize_with = "flag")]
        web: bool,
        /// An agent by name (in ai.toml); empty = the chain.
        #[serde(default)]
        provider: String,
        /// The selected database is told to the model.
        #[serde(default = "si", deserialize_with = "flag")]
        attach: bool,
    }
    fn si() -> bool {
        true
    }
    /// Yes/no as it comes: true/false, 1/0, "1"/"true".
    fn flag<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
        use serde::Deserialize;
        let v = serde_json::Value::deserialize(d)?;
        Ok(match v {
            serde_json::Value::Bool(b) => b,
            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
            serde_json::Value::String(s) => matches!(s.trim(), "1" | "true" | "si" | "sì"),
            _ => false,
        })
    }
    let req: Req = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(e) => return respond(&mut sock, "400 Bad Request", "text/plain", e.to_string().as_bytes()).await,
    };
    sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n").await?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<serde_json::Value>();
    let history: Vec<crate::ai::HistoryMsg> =
        req.messages.into_iter().map(|m| (m.role, m.content, m.images.iter().take(8).filter_map(|u| data_url_bytes(u)).collect())).collect();
    let use_tools = req.web;
    let attach = req.attach;
    let only = req.provider.trim().to_string();
    // «Stop» in the page = connection closed: seen at once, and the work stops before the next tool
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_w = stop.clone();
    let worker = tokio::task::spawn_blocking(move || {
        use crate::ai::{self, Event};
        let mut cfg = match ai::AiConfig::load() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(serde_json::json!({ "t": "error", "msg": e.to_string() }));
                return;
            }
        };
        if !only.is_empty() {
            if !cfg.providers.iter().any(|p| p.name == only) {
                let _ = tx.send(serde_json::json!({ "t": "error", "msg": format!("agente «{only}» non trovato in ai.toml") }));
                return;
            }
            cfg.answer_with = Some(only.clone());
        }
        let ctx = if attach { ai::context() } else { None };
        let msgs = ai::chat_messages_with_images(&cfg, &history, ctx.as_ref());
        let res = ai::run_with(&cfg, ai::Task::Chat, &msgs, use_tools, &mut |ev| {
            let j = match ev {
                Event::Trying { provider, model, context } => serde_json::json!({ "t": "try", "provider": provider, "model": model, "context": context }),
                Event::Usage { used, max, exact, output } => serde_json::json!({ "t": "usage", "used": used, "max": max, "exact": exact, "output": output }),
                Event::Thinking(d) => serde_json::json!({ "t": "think", "text": d }),
                // the app's texts (not the model's) in the interface language
                Event::Skipped { provider, why } => serde_json::json!({ "t": "skip", "provider": provider, "why": crate::i18n::tf(why) }),
                Event::Delta(d) => serde_json::json!({ "t": "delta", "text": d }),
                // `key` = the Italian text, to recognise the tool in the page
                Event::Tool { label } => serde_json::json!({ "t": "tool", "key": label.clone(), "label": crate::i18n::tf(label) }),
                Event::ToolDone { label, result } => serde_json::json!({ "t": "tooldone", "key": label.clone(), "label": crate::i18n::tf(label), "result": crate::i18n::tf(result) }),
                Event::OpenedQuery { title } => serde_json::json!({ "t": "opened", "title": title }),
            };
            // false = the page closed: stop
            !stop_w.load(std::sync::atomic::Ordering::SeqCst) && tx.send(j).is_ok()
        });
        let _ = tx.send(match res {
            Ok(r) => serde_json::json!({ "t": "done", "provider": r.provider, "model": r.model }),
            Err(e) => serde_json::json!({ "t": "error", "msg": crate::i18n::tf(e.to_string()) }),
        });
    });
    let mut ping = tokio::time::interval(Duration::from_secs(15));
    ping.tick().await;
    loop {
        tokio::select! {
            m = rx.recv() => match m {
                Some(j) => {
                    if let Err(e) = send_event(&mut sock, &j.to_string()).await {
                        drop(rx);
                        let _ = worker.await;
                        return Err(e);
                    }
                }
                None => break,
            },
            _ = ping.tick() => {
                if sock.write_all(b": ping\n\n").await.is_err() || sock.flush().await.is_err() {
                    stop.store(true, std::sync::atomic::Ordering::SeqCst);
                    break;
                }
            }
            r = sock.readable() => {
                let mut probe = [0u8; 256];
                let closed = match r {
                    Ok(()) => match sock.try_read(&mut probe) {
                        Ok(0) => true,
                        Ok(_) => false,
                        Err(e) => e.kind() != std::io::ErrorKind::WouldBlock,
                    },
                    Err(_) => true,
                };
                if closed {
                    stop.store(true, std::sync::atomic::Ordering::SeqCst);
                    break;
                }
            }
        }
    }
    drop(rx);
    let _ = worker.await;
    sock.shutdown().await
}

async fn send_event(sock: &mut TcpStream, json: &str) -> std::io::Result<()> {
    let mut msg = Vec::with_capacity(json.len() + 8);
    msg.extend_from_slice(b"data: ");
    msg.extend_from_slice(json.as_bytes());
    msg.extend_from_slice(b"\n\n");
    sock.write_all(&msg).await?;
    sock.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_origins() {
        assert!(host_ok(Some("127.0.0.1:5000")) && host_ok(Some("localhost:1")) && host_ok(None));
        assert!(!host_ok(Some("evil.example.com:5000")));
        assert!(origin_ok(None, Some("127.0.0.1:5")));
        assert!(origin_ok(Some("http://127.0.0.1:5"), Some("127.0.0.1:5")));
        assert!(!origin_ok(Some("http://evil.example.com"), Some("127.0.0.1:5")));
    }
}
