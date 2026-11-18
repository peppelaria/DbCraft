//! Conversazioni della chat AI salvate (come in OpenSlate): `chats/<id>.json` nella cartella
//! di configurazione di DbCraft (la conversazione, scritta dalla pagina). Le eliminate vanno in
//! `chats/cestino/`.

use serde_json::{Value, json};
use std::path::PathBuf;

fn dir() -> PathBuf {
    crate::store::config_dir().unwrap_or_else(std::env::temp_dir).join("chats")
}

/// Id valido: lettere, cifre e trattini (niente percorsi).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn json_path(id: &str) -> PathBuf {
    dir().join(format!("{id}.json"))
}

/// Rimette a posto una chat eliminata («Annulla»).
pub fn restore(id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("id non valido".into());
    }
    let src = dir().join("cestino").join(format!("{id}.json"));
    let dst = json_path(id);
    if src.exists() && !dst.exists() {
        std::fs::rename(&src, &dst).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Elenco per la barra laterale (senza i messaggi), dal più recente.
pub fn list() -> Value {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let Some(id) = name.strip_suffix(".json") else { continue };
            if !valid_id(id) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
            let msgs = v["messages"].as_array();
            out.push(json!({
                "id": id,
                "title": v["title"].as_str().unwrap_or("Nuova chat"),
                "project": v["project"].as_str().unwrap_or(""),
                "pinned": v["pinned"].as_bool().unwrap_or(false),
                "created": v["created"].as_u64().unwrap_or(0),
                "updated": v["updated"].as_u64().unwrap_or(0),
                "count": msgs.map(|m| m.len()).unwrap_or(0),
                "page": "",
            }));
        }
    }
    out.sort_by(|a, b| b["updated"].as_u64().cmp(&a["updated"].as_u64()));
    Value::Array(out)
}

pub fn get(id: &str) -> Option<Vec<u8>> {
    valid_id(id).then(|| std::fs::read(json_path(id)).ok()).flatten()
}

/// Salva la conversazione così come la manda la pagina (dev'essere JSON).
pub fn save(id: &str, body: &[u8]) -> Result<(), String> {
    if !valid_id(id) {
        return Err("id non valido".into());
    }
    let v: Value = serde_json::from_slice(body).map_err(|e| format!("JSON non valido: {e}"))?;
    if !v.is_object() {
        return Err("serve un oggetto JSON".into());
    }
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    crate::web::write_atomic(&json_path(id), body).map_err(|e| e.to_string())
}

/// Nel cestino, non cancellata davvero.
pub fn trash(id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("id non valido".into());
    }
    let bin = dir().join("cestino");
    std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
    let p = json_path(id);
    if p.exists() {
        std::fs::rename(&p, bin.join(format!("{id}.json"))).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_sicuri() {
        assert!(valid_id("c-20261019-abc123"));
        for bad in ["", "../x", "a/b", "a.b", "con spazio"] {
            assert!(!valid_id(bad), "{bad}");
        }
    }
}
