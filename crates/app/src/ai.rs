//! Agenti AI tramite API in formato OpenAI (`/chat/completions`), come in OpenSlate: una catena
//! di fallback, si prova il primo provider di `ai.toml`; se non risponde (spento, chiave
//! mancante, errore…) si passa al successivo.
//!
//! Il formato OpenAI è solo il "dialetto" dell'API: lo parlano LM Studio, Ollama, OpenAI,
//! DeepSeek, Google Gemini e Anthropic Claude.
//!
//! Le chiavi non stanno in chiaro nel file: `key = "env:NOME"` (variabile d'ambiente) oppure
//! `key = "keychain:NOME"` (Portachiavi), anche in alternativa: `"env:NOME | keychain:NOME"`.
//!
//! In DbCraft la chat ha gli strumenti del database (`dbagent`): vede le connessioni, legge
//! struttura e dati, scrive query e le apre nell'editor.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const CONFIG_FILE: &str = "ai.toml";

/// `ai.toml` nella cartella di configurazione di DbCraft (o il file indicato da `DBCRAFT_AI_CONFIG`).
pub fn config_path() -> String {
    if let Ok(p) = std::env::var("DBCRAFT_AI_CONFIG") {
        return p;
    }
    match crate::store::config_dir() {
        Some(d) => {
            let _ = std::fs::create_dir_all(&d);
            d.join(CONFIG_FILE).to_string_lossy().into_owned()
        }
        None => CONFIG_FILE.to_string(),
    }
}

/// Il percorso completo di `ai.toml` (per mostrarlo: dice quale file si usa).
pub fn config_path_full() -> String {
    let p = std::path::PathBuf::from(config_path());
    let p = if p.is_absolute() { p } else { std::env::current_dir().map(|d| d.join(&p)).unwrap_or(p) };
    p.to_string_lossy().into_owned()
}

/// File creato al primo avvio (commentato, da modificare a mano o dalla pagina «Impostazioni AI»).
pub const DEFAULT_TOML: &str = r#"# Agenti AI di DbCraft (la chat che aiuta con le query e con i database).
#
# Tutti i provider parlano l'API in formato OpenAI (/chat/completions).
# Vengono provati IN ORDINE: se il primo non risponde (spento, chiave
# mancante, errore) si passa al successivo. Per cambiare priorità basta
# spostare i blocchi [[provider]]; per escluderne uno: enabled = false.
#
# Campi:
#   name          nome mostrato
#   url           indirizzo base dell'API (quello che finisce con /v1 o simile)
#   model         modello per la chat. Vuoto = scelto da /models
#   vision        false = il provider non legge immagini (riceve solo testo)
#   key           chiave API, MAI in chiaro:
#                   "env:NOME"       variabile d'ambiente
#                   "keychain:NOME"  Portachiavi (su macOS: security add-generic-password -a "$USER" -s NOME -w 'la-chiave')
#                   le due insieme:  "env:NOME | keychain:NOME"
#                   "dsh:NOME"       chiave di DeepSeek Harness (~/.dsh/.credentials.yaml)
#                 Se la chiave non si trova il provider viene saltato.
#   timeout_s     secondi massimi di attesa della risposta
#   temperature   facoltativa (es. 0.2)
#   no_temperature  true = non mandare mai la temperatura (Claude)
#   max_tokens    facoltativo
#   tools         false = non offrire strumenti (il database) a questo provider
#   context       contesto massimo in token (0 = da /models se il modello è
#                 scelto da lì, altrimenti 32768): serve all'indicatore e alla
#                 compattazione della chat

# 1) LM Studio su questo computer (Developer → Start Server).
[[provider]]
name = "LM Studio"
url = "http://localhost:1234/v1"
model = ""
key = ""
timeout_s = 300


# 2) Ollama su questo computer (ollama serve).
[[provider]]
name = "Ollama"
url = "http://localhost:11434/v1"
model = ""
key = ""
timeout_s = 300

# 3) DeepSeek, API ufficiale (la stessa di DSH). deepseek-flash legge anche
#    le immagini (https://api-docs.deepseek.com/guides/vision/).
[[provider]]
name = "DeepSeek"
url = "https://api.deepseek.com/v1"
model = "deepseek-flash"
context = 131072
key = "dsh:DEEPSEEK_API_KEY | env:DEEPSEEK_API_KEY | keychain:DEEPSEEK_API_KEY"
timeout_s = 180

# 4) Google Gemini (endpoint compatibile OpenAI).
[[provider]]
name = "Google Gemini"
url = "https://generativelanguage.googleapis.com/v1beta/openai"
model = "gemini-3.8-flash"
key = "env:GEMINI_API_KEY | keychain:GEMINI_API_KEY"
context = 1048576
timeout_s = 180

# 5) Anthropic Claude (endpoint compatibile OpenAI).
[[provider]]
name = "Anthropic Claude"
url = "https://api.anthropic.com/v1"
model = "claude-sonnet-4-6"
key = "env:ANTHROPIC_API_KEY | keychain:ANTHROPIC_API_KEY"
no_temperature = true

# 6) OpenAI.
[[provider]]
name = "OpenAI"
url = "https://api.openai.com/v1"
model = "gpt-5.1"
key = "env:OPENAI_API_KEY | keychain:OPENAI_API_KEY"
timeout_s = 180
context = 400000

# Istruzioni proprie (facoltative) al posto di quelle interne: vanno scritte
# IN CIMA al file, prima del primo [[provider]]:
#   chat_prompt = """..."""

[chat]
compact_at = 80   # compatta da sola la conversazione all'80% del contesto (0 = mai)
keep_recent = 4   # messaggi più recenti tenuti parola per parola

# Il database: cosa può fare l'AI con le connessioni di DbCraft.
#   allow_write  false = l'AI legge soltanto (SELECT, SHOW, EXPLAIN…); true = può anche
#                modificare dati e struttura, ma ogni modifica chiede prima il tuo sì in chat
#   max_rows     righe al massimo che l'AI riceve da una query
[db]
allow_write = true
max_rows = 200
"#;

/// Intestazione del file quando lo scrive la pagina «Impostazioni AI».
const SAVED_HEADER: &str = "# Agenti AI di DbCraft: si modificano comodamente dalla pagina
# «Impostazioni AI» (Strumenti › Impostazioni AI, o il pulsante nella chat), oppure qui a mano.
# Gli agenti si provano in ordine; il primo che risponde vince.
# key: \"dsh:NOME\" (chiave di DeepSeek Harness), \"keychain:NOME\" (Portachiavi),
#      \"env:NOME\" (variabile d'ambiente), anche in alternativa con |
# context: contesto massimo in token (0 = automatico)
";

/// Istruzioni della chat di DbCraft.
pub const CHAT_PROMPT: &str = r#"Sei l'assistente AI di DbCraft, un client per database (MySQL, MariaDB, PostgreSQL, SQL Server, SQLite). Aiuti l'utente a scrivere, spiegare, correggere e ottimizzare query SQL e a capire i suoi database: struttura, relazioni, dati, indici, prestazioni. Rispondi sempre nella lingua dell'ultimo messaggio dell'utente, in Markdown, anche se queste istruzioni e i risultati degli strumenti sono in italiano. Scrivi il codice SQL in blocchi ```sql, nel dialetto del database di cui si parla. Quando serve conoscere tabelle, colonne o dati, usali con gli strumenti invece di indovinare. Non inventare nomi di tabelle o colonne."#;

// ---------------------------------------------------------------------------
// Configurazione
// ---------------------------------------------------------------------------

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Provider {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub vision_model: String,
    #[serde(default = "yes")]
    pub vision: bool,
    #[serde(default)]
    pub key: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "default_timeout")]
    pub timeout_s: u64,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub no_temperature: bool,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// Tutti i modelli di questo server leggono le immagini (es. MTPLX con
    /// Qwen3.8): la scelta automatica non guarda il nome.
    #[serde(default)]
    pub all_vision: bool,
    /// false = a questo provider non si offrono strumenti (ricerca web…).
    #[serde(default = "yes")]
    pub tools: bool,
    /// Contesto massimo del modello in token (0 = lo si chiede a /models,
    /// altrimenti 32768).
    #[serde(default)]
    pub context: u32,
}


impl Provider {
    /// Server che mandano il conteggio dei token anche in streaming se
    /// glielo si chiede (`stream_options`); agli altri non si chiede, per
    /// non rischiare un rifiuto: i token si stimano.
    fn stream_usage(&self) -> bool {
        ["api.deepseek.com", "googleapis.com", "api.openai.com", "openrouter.ai"].iter().any(|h| self.url.contains(h))
    }

}

/// Opzioni della chat.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ChatOpts {
    /// Compattazione automatica quando la conversazione arriva a questa
    /// percentuale del contesto (0 = mai).
    #[serde(default = "default_compact_at")]
    pub compact_at: u8,
    /// Messaggi più recenti tenuti parola per parola quando si compatta.
    #[serde(default = "default_keep_recent")]
    pub keep_recent: usize,
}

/// Cosa può fare l'AI con i database (`[db]`).
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct DbOpts {
    /// L'AI può anche modificare dati e struttura (ogni volta col sì dell'utente).
    #[serde(default = "yes")]
    pub allow_write: bool,
    /// Righe al massimo restituite all'AI da una query.
    #[serde(default = "default_max_rows")]
    pub max_rows: usize,
}

fn default_max_rows() -> usize {
    200
}

impl Default for DbOpts {
    fn default() -> Self {
        Self { allow_write: true, max_rows: default_max_rows() }
    }
}

fn default_compact_at() -> u8 {
    80
}
fn default_keep_recent() -> usize {
    4
}

impl Default for ChatOpts {
    fn default() -> Self {
        Self { compact_at: default_compact_at(), keep_recent: default_keep_recent() }
    }
}

fn yes() -> bool {
    true
}
fn default_timeout() -> u64 {
    180
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct AiConfig {
    #[serde(default, rename = "provider")]
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub chat_prompt: Option<String>,
    #[serde(default)]
    pub chat: ChatOpts,
    /// Cosa può fare l'AI con i database.
    #[serde(default)]
    pub db: DbOpts,
    /// Agente scelto in chat: risponde solo lui. `None` = catena.
    #[serde(skip)]
    pub answer_with: Option<String>,
}

impl AiConfig {
    /// Legge `ai.toml` (lo crea al primo uso). Riletto a ogni richiesta: le
    /// modifiche valgono subito, senza riavviare.
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !std::path::Path::new(&path).exists() {
            std::fs::write(&path, DEFAULT_TOML).with_context(|| format!("creazione di {path}"))?;
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("lettura di {path}"))?;
        Self::parse(&text)
    }

    /// Riscrive `ai.toml` (dalla pagina «Impostazioni AI»), tenendo una
    /// copia della versione precedente in `ai.toml.bak`.
    pub fn save(&self) -> Result<()> {
        if self.providers.is_empty() {
            bail!("serve almeno un agente");
        }
        for p in &self.providers {
            if p.name.trim().is_empty() || p.url.trim().is_empty() {
                bail!("ogni agente deve avere nome e indirizzo");
            }
            if !(p.url.starts_with("http://") || p.url.starts_with("https://")) {
                bail!("«{}»: l'indirizzo deve iniziare con http:// o https://", p.name);
            }
        }
        let body = toml::to_string_pretty(self).map_err(|e| anyhow!("{e}"))?;
        let text = format!("{SAVED_HEADER}\n{body}");
        Self::parse(&text)?; // deve rileggersi uguale
        let path = config_path();
        if std::path::Path::new(&path).exists() {
            let _ = std::fs::copy(&path, format!("{path}.bak"));
        }
        crate::web::write_atomic(std::path::Path::new(&path), text.as_bytes()).with_context(|| format!("scrittura di {path}"))?;
        Ok(())
    }

    pub fn parse(text: &str) -> Result<Self> {
        let cfg: AiConfig = toml::from_str(text).map_err(|e| anyhow!("{CONFIG_FILE}: {e}"))?;
        if cfg.providers.is_empty() {
            bail!("{CONFIG_FILE}: nessun [[provider]]");
        }
        Ok(cfg)
    }

    pub fn chat_prompt(&self) -> &str {
        self.chat_prompt.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or(CHAT_PROMPT)
    }
}


/// Risolve la chiave: `None` = richiesta ma non trovata (provider saltato),
/// `Some("")` = nessuna chiave necessaria.
pub fn resolve_key(spec: &str) -> Option<String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Some(String::new());
    }
    for alt in spec.split('|').map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(name) = alt.strip_prefix("env:") {
            if let Ok(v) = std::env::var(name.trim())
                && !v.trim().is_empty()
            {
                return Some(v.trim().to_string());
            }
        } else if let Some(name) = alt.strip_prefix("keychain:") {
            if let Some(v) = keychain(name.trim()) {
                return Some(v);
            }
        } else if let Some(name) = alt.strip_prefix("dsh:") {
            if let Some(v) = dsh_key(name.trim()) {
                return Some(v);
            }
        } else {
            // chiave scritta in chiaro (sconsigliato, ma accettato)
            return Some(alt.to_string());
        }
    }
    None
}

/// Chiave salvata da DeepSeek Harness (`~/.dsh/.credentials.yaml`, sezione
/// `refs`): così le chiavi già date a DSH non vanno ripetute.
fn dsh_key(name: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = std::env::var("WACOM_DSH_CREDENTIALS").unwrap_or_else(|_| format!("{home}/.dsh/.credentials.yaml"));
    dsh_key_from(&std::fs::read_to_string(path).ok()?, name)
}

/// Legge `refs: { NOME: valore }` da un YAML semplice (senza librerie).
fn dsh_key_from(text: &str, name: &str) -> Option<String> {
    let mut in_refs = false;
    for line in text.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if !indented {
            in_refs = line.trim_end() == "refs:";
            continue;
        }
        if !in_refs {
            continue;
        }
        let Some((k, v)) = line.trim().split_once(':') else { continue };
        if k.trim().trim_matches(|c| c == '"' || c == '\'') == name {
            let v = v.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            return (!v.is_empty()).then(|| v.to_string());
        }
    }
    None
}


/// Una chiave nel Portachiavi. Su macOS con `security`, come in OpenSlate (così le chiavi già
/// salvate per OpenSlate valgono anche qui); altrove con il portachiavi del sistema.
fn keychain(service: &str) -> Option<String> {
    if cfg!(target_os = "macos") {
        let out = std::process::Command::new("security").args(["find-generic-password", "-s", service, "-w"]).output().ok()?;
        let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return (out.status.success() && !v.is_empty()).then_some(v);
    }
    let v = keyring::Entry::new(service, "dbcraft").ok()?.get_password().ok()?;
    (!v.trim().is_empty()).then(|| v.trim().to_string())
}

// ---------------------------------------------------------------------------
// Messaggi
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Msg {
    pub role: String,
    pub text: String,
    /// Immagine PNG allegata (solo ai provider con visione).
    pub image: Option<Arc<Vec<u8>>>,
    /// Immagini incollate nella chat (PNG o JPEG), dopo il foglio.
    pub images: Vec<Arc<Vec<u8>>>,
}

impl Msg {
    pub fn new(role: &str, text: impl Into<String>) -> Self {
        Self { role: role.into(), text: text.into(), image: None, images: Vec::new() }
    }
    /// Con le immagini incollate.
    pub fn with_images(mut self, imgs: Vec<Arc<Vec<u8>>>) -> Self {
        self.images = imgs;
        self
    }
    /// Ha immagini?
    pub fn has_images(&self) -> bool {
        self.image.is_some() || !self.images.is_empty()
    }
}

/// Cosa sta succedendo (per mostrarlo in chat).
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Si prova questo provider con questo modello.
    Trying { provider: String, model: String, context: u32 },
    /// Provider saltato o fallito: si passa al prossimo.
    Skipped { provider: String, why: String },
    /// Pezzo di risposta (streaming).
    Delta(String),
    /// Pezzo del ragionamento del modello (`reasoning_content` o `<think>`),
    /// mostrato a parte e richiudibile.
    Thinking(String),
    /// Il modello usa uno strumento (es. «Cerco in rete: …»).
    Tool { label: String },
    /// Strumento finito, con l'esito breve.
    ToolDone { label: String, result: String },
    /// L'AI ha aperto una query nell'editor di DbCraft.
    OpenedQuery { title: String },
    /// Token usati dalla conversazione (esatti se il server li dice,
    /// altrimenti stimati), token della risposta e contesto massimo del modello.
    Usage { used: u32, max: u32, exact: bool, output: u32 },
}

#[derive(Clone, Debug)]
pub struct Reply {
    pub text: String,
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Task {
    /// Chat: le immagini vanno solo a chi le legge.
    Chat,
    /// Una domanda secca (prova di un agente, riassunto): niente strumenti.
    Plain,
}


/// Prova i provider in ordine. `on_event` riceve tentativi e pezzi di
/// risposta; se ritorna `false` la richiesta si interrompe.
pub fn run(cfg: &AiConfig, task: Task, msgs: &[Msg], on_event: &mut dyn FnMut(Event) -> bool) -> Result<Reply> {
    run_with(cfg, task, msgs, false, on_event)
}


/// Prova i provider della catena (o solo quello scelto), con gli strumenti del database se
/// `use_tools`. Le immagini incollate vanno solo a chi le legge.
pub fn run_with(cfg: &AiConfig, task: Task, msgs: &[Msg], use_tools: bool, on_event: &mut dyn FnMut(Event) -> bool) -> Result<Reply> {
    let tool_ctx = (task == Task::Chat && use_tools).then(|| ToolCtx::full(cfg, msgs));
    let has_image = msgs.iter().any(Msg::has_images);
    let mut fails: Vec<String> = Vec::new();
    let answering = |p: &&Provider| match &cfg.answer_with {
        Some(name) => &p.name == name,
        None => p.enabled,
    };
    for p in cfg.providers.iter().filter(answering) {
        let mut skip = |why: String, on_event: &mut dyn FnMut(Event) -> bool| {
            fails.push(format!("{}: {why}", p.name));
            on_event(Event::Skipped { provider: p.name.clone(), why })
        };
        let Some(key) = resolve_key(&p.key) else {
            if !skip(format!("chiave non trovata ({})", p.key.trim()), on_event) {
                bail!("interrotto");
            }
            continue;
        };
        let mut send_image = has_image && p.vision;
        let mut picked = pick_model(p, &key, send_image);
        if picked.is_err() && send_image {
            // senza modello visivo: si risponde al testo
            if let Ok(m) = pick_model(p, &key, false) {
                send_image = false;
                picked = Ok(m);
            }
        }
        let model = match picked {
            Ok(m) => m,
            Err(e) => {
                if !skip(short_err(&e), on_event) {
                    bail!("interrotto");
                }
                continue;
            }
        };
        let context = context_for(p, &key, &model);
        if !on_event(Event::Trying { provider: p.name.clone(), model: model.clone(), context }) {
            bail!("interrotto");
        }
        // «iniziata» anche quando ha solo usato gli strumenti: un altro agente
        // ripartirebbe da capo e rifarebbe le modifiche
        let mut started = false;
        let tools = tool_ctx.as_ref().filter(|_| p.tools);
        let res = call(p, &key, &model, task, msgs, send_image, tools, &mut |e| {
            if matches!(e, Event::Delta(_) | Event::ToolDone { .. } | Event::OpenedQuery { .. }) {
                started = true;
            }
            on_event(e)
        });
        match res {
            Ok(text) if !text.trim().is_empty() => {
                return Ok(Reply { text, provider: p.name.clone(), model });
            }
            Ok(_) if started => {
                let text: String = crate::i18n::t("Fatto (i passaggi sono qui sopra).").into();
                on_event(Event::Delta(text.clone()));
                return Ok(Reply { text, provider: p.name.clone(), model });
            }
            Ok(_) => {
                if !skip("risposta vuota".into(), on_event) {
                    bail!("interrotto");
                }
            }
            // A risposta già iniziata non si può ripartire da un altro provider.
            Err(e) if started => bail!("{} si è interrotto: {}", p.name, short_err(&e)),
            Err(e) => {
                if e.to_string() == "interrotto" || !skip(short_err(&e), on_event) {
                    bail!("interrotto");
                }
            }
        }
    }
    if fails.is_empty() {
        bail!("nessun provider attivo in {CONFIG_FILE}");
    }
    bail!("nessun agente AI disponibile:\n• {}", fails.join("\n• "))
}

fn short_err(e: &anyhow::Error) -> String {
    let s = format!("{e:#}");
    let s = s.trim();
    if s.chars().count() > 300 {
        format!("{}…", s.chars().take(300).collect::<String>())
    } else {
        s.to_string()
    }
}

/// Un agente nel cloud (non questo Mac né la rete di casa).
fn is_cloud(p: &Provider) -> bool {
    let rest = p.url.split("://").nth(1).unwrap_or("");
    let hostport = rest.split('/').next().unwrap_or("");
    let host = if let Some(h) = hostport.strip_prefix('[') { h.split(']').next().unwrap_or("") } else { hostport.rsplit_once(':').map(|(h, _)| h).unwrap_or(hostport) };
    let host = host.to_ascii_lowercase();
    let local = host == "localhost"
        || host.ends_with(".local")
        || !host.contains('.') && host.parse::<std::net::IpAddr>().is_err() // nome della rete di casa («mac-studio»)
        || host.parse::<std::net::IpAddr>().is_ok_and(|ip| match ip {
            std::net::IpAddr::V4(v) => {
                let o = v.octets();
                // anche 100.64/10 (Tailscale e simili)
                v.is_loopback() || v.is_private() || v.is_link_local() || (o[0] == 100 && (o[1] & 0xc0) == 64)
            }
            std::net::IpAddr::V6(v) => v.is_loopback() || (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80,
        });
    !local
}

/// Silenzio massimo dal server: in attesa della risposta e fra un pezzo e
/// l'altro. Nel cloud basta un minuto (chi pensa a lungo manda comunque il
/// ragionamento); un modello sul Mac può impiegare minuti a leggere un
/// contesto lungo, lì vale il `timeout_s` dell'agente.
fn idle_secs(p: &Provider) -> u64 {
    let t = p.timeout_s.max(5);
    if is_cloud(p) {
        t.min(90)
    } else {
        t
    }
}

/// Attesa massima della prima risposta (e di ogni lettura): nel cloud al più
/// due minuti (un modello che pensa senza mandare niente ha il tempo), sul
/// Mac il `timeout_s` dell'agente.
fn first_secs(p: &Provider) -> u64 {
    let t = p.timeout_s.max(5);
    if is_cloud(p) {
        t.min(120)
    } else {
        t
    }
}

fn agent(p: &Provider) -> ureq::Agent {
    ureq::AgentBuilder::new()
        // un server spento deve far passare subito al prossimo
        .timeout_connect(Duration::from_secs(4))
        .timeout_read(Duration::from_secs(first_secs(p)))
        .build()
}

fn base(p: &Provider) -> &str {
    p.url.trim().trim_end_matches('/')
}

fn with_auth(req: ureq::Request, key: &str) -> ureq::Request {
    if key.is_empty() {
        req
    } else {
        req.set("Authorization", &format!("Bearer {key}"))
    }
}

/// Modello da usare: quello configurato, altrimenti scelto da `/models`
/// (per le immagini preferisce i modelli con visione).
pub(crate) fn pick_model(p: &Provider, key: &str, vision: bool) -> Result<String> {
    let configured = if vision && !p.vision_model.trim().is_empty() { &p.vision_model } else { &p.model };
    if !configured.trim().is_empty() {
        return Ok(configured.trim().to_string());
    }
    let models = list_models(p, key)?;
    if let Ok(mut c) = CONTEXTS.lock() {
        for (id, ctx) in &models {
            if let Some(n) = ctx {
                c.retain(|(u, m, _)| !(u == &p.url && m == id));
                c.push((p.url.clone(), id.clone(), *n));
            }
        }
    }
    let ids: Vec<String> = models.into_iter().map(|(id, _)| id).collect();
    choose_model_why(&ids, vision && !p.all_vision).map_err(|e| anyhow!("{e}"))
}

/// Contesto se non si sa altro.
pub const DEFAULT_CONTEXT: u32 = 32768;

static CONTEXTS: Mutex<Vec<(String, String, u32)>> = Mutex::new(Vec::new());

fn context_cached(url: &str, model: &str) -> Option<u32> {
    CONTEXTS.lock().ok()?.iter().find(|(u, m, _)| u == url && m == model).map(|x| x.2)
}

/// Contesto massimo del modello: quello scritto in ai.toml, altrimenti quello
/// dichiarato da `/models` quando il modello è scelto da lì (LM Studio, MTPLX,
/// vLLM, OpenRouter…), altrimenti 32768.
fn context_for(p: &Provider, key: &str, model: &str) -> u32 {
    if p.context > 0 {
        if let Ok(mut c) = CONTEXTS.lock() {
            c.retain(|(u, m, _)| !(u == &p.url && m == model));
            c.push((p.url.clone(), model.to_string(), p.context));
        }
        return p.context;
    }
    if let Some(c) = context_cached(&p.url, model) {
        return c;
    }
    // (con il modello scelto da /models il contesto è già nella cache;
    // per un modello scritto a mano si usa context di ai.toml)
    let _ = key;
    DEFAULT_CONTEXT
}

/// Contesto dichiarato in una voce di `/models`, se c'è.
pub fn context_of_model_entry(m: &serde_json::Value) -> Option<u32> {
    for k in ["context_length", "context_window", "max_context_length", "loaded_context_length", "max_model_len", "max_input_tokens", "inputTokenLimit"] {
        if let Some(n) = m[k].as_u64().filter(|&n| n > 0) {
            return Some(n.min(u32::MAX as u64) as u32);
        }
    }
    m["top_provider"]["context_length"].as_u64().map(|n| n as u32)
}

/// Modelli di un server, con il contesto se lo dichiara.
pub fn list_models(p: &Provider, key: &str) -> Result<Vec<(String, Option<u32>)>> {
    let url = format!("{}/models", base(p));
    let mut r = http(p, key, &url, None).map_err(|e| anyhow!("non raggiungibile ({e})"))?;
    if r.status >= 400 {
        bail!("{url}: {}", http_error(&mut r, key));
    }
    let v: serde_json::Value = serde_json::from_reader(r.body).map_err(|e| anyhow!("{url}: risposta non valida ({e})"))?;
    Ok(v["data"]
        .as_array()
        .or(v["models"].as_array())
        .map(|a| {
            a.iter()
                .filter_map(|m| Some((m["id"].as_str().or(m["name"].as_str())?.to_string(), context_of_model_entry(m))))
                .collect()
        })
        .unwrap_or_default())
}

/// Stima grossolana dei token di un testo (italiano e LaTeX: ~3,2 caratteri
/// per token).
pub fn estimate_text(s: &str) -> u32 {
    (s.chars().count() as f32 / 3.2).ceil() as u32
}

/// Token stimati di un'immagine allegata.
pub const IMAGE_TOKENS: u32 = 1200;

fn estimate_wire(wire: &[serde_json::Value]) -> u32 {
    wire.iter()
        .map(|m| match &m["content"] {
            serde_json::Value::String(t) => estimate_text(t) + 4,
            serde_json::Value::Array(parts) => parts
                .iter()
                .map(|p| if p["type"] == "image_url" { IMAGE_TOKENS } else { estimate_text(p["text"].as_str().unwrap_or("")) })
                .sum::<u32>(),
            _ => estimate_text(&m.to_string()),
        })
        .sum()
}

/// Istruzioni per riassumere la conversazione (compattazione).
const COMPACT_PROMPT: &str = "Riassumi la conversazione che segue in modo che possa continuare senza perdere nulla di importante. \
Scrivi nella lingua della conversazione, in Markdown (formule in LaTeX con $...$ e $$...$$, se ce ne sono). Conserva: l'argomento \
e i materiali di cui si parla (pagine, documenti, appunti: il contenuto essenziale), i risultati raggiunti, le decisioni prese, \
gli errori trovati e le correzioni, le fonti citate (con link), le domande ancora aperte. Togli saluti, ripetizioni e passaggi superati. Non aggiungere nulla che non sia nella conversazione. \
Inizia direttamente con il riassunto.";

/// Compatta una conversazione: ritorna il riassunto e chi l'ha scritto.
pub fn compact(cfg: &AiConfig, history: &[(String, String)]) -> Result<Reply> {
    let mut transcript = String::new();
    for (role, text) in history {
        let who = match role.as_str() {
            "assistant" => "AI",
            "summary" => "Riassunto precedente",
            _ => "Utente",
        };
        transcript.push_str(&format!("### {who}\n{}\n\n", text.trim()));
    }
    let msgs = vec![Msg::new("system", COMPACT_PROMPT), Msg::new("user", transcript)];
    let mut r = run(cfg, Task::Chat, &msgs, &mut |_| true)?;
    r.text = clean_reply(&r.text);
    Ok(r)
}


/// Dimensione dichiarata nel nome, in miliardi di parametri
/// («qwen3-vl-8b-instruct» → 8, «qwen3-30b-a3b» → 30). Sconosciuta = None.
pub fn model_size(id: &str) -> Option<f32> {
    id.to_lowercase()
        .split(['-', '_', ':', '/', ' ', '@'])
        .filter_map(|t| t.strip_suffix('b')?.parse::<f32>().ok())
        .reduce(f32::max)
}

/// Sotto questa soglia (miliardi) un modello scelto da solo non si usa:
/// risponde male e legge male la scrittura a mano.
const MIN_AUTO_SIZE: f32 = 3.0;

/// Come `choose_model`, ma spiega perché non ne ha trovato uno.
/// Fra i candidati preferisce il più grande (dimensione letta dal nome).
pub fn choose_model_why(ids: &[String], vision: bool) -> Result<String, String> {
    const VISION_HINTS: [&str; 11] = ["vl", "vision", "ocr", "llava", "pixtral", "gemma-3", "gemma3", "minicpm-v", "omni", "kimi-vl", "qwen3.5"];
    let usable: Vec<&String> = ids
        .iter()
        .filter(|id| {
            let l = id.to_lowercase();
            !l.contains("embed") && !l.contains("rerank") && !l.contains("whisper") && !l.contains("tts")
        })
        .collect();
    if usable.is_empty() {
        return Err("nessun modello caricato".into());
    }
    let is_vision = |id: &str| {
        let l = id.to_lowercase();
        VISION_HINTS.iter().any(|h| l.contains(h))
    };
    // per le immagini solo i modelli con visione; in chat va bene qualunque
    let cands: Vec<&String> = usable.iter().copied().filter(|id| !vision || is_vision(id)).collect();
    if cands.is_empty() {
        return Err(format!(
            "nessun modello con visione caricato (ci sono: {}). Carica ad es. Qwen3-VL-8B",
            usable.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
    let size = |id: &str| model_size(id).unwrap_or(7.0);
    let Some(best) = cands.iter().copied().max_by(|a, b| size(a).total_cmp(&size(b))) else {
        return Err("nessun modello caricato".into());
    };
    if size(best) < MIN_AUTO_SIZE {
        return Err(format!(
            "solo modelli troppo piccoli per rispondere bene ({best}): caricane uno più grande o scrivilo in «model» di ai.toml"
        ));
    }
    Ok(best.to_string())
}

fn transport_msg(e: &ureq::Error) -> String {
    match e {
        ureq::Error::Transport(t) => {
            let s = t.to_string();
            if s.contains("Connection refused") {
                "connessione rifiutata: server spento?".into()
            } else if s.contains("os error 65") || s.contains("No route to host") || s.contains("os error 113") {
                // macOS dà questo errore anche quando manca il permesso «Rete locale»
                "computer non raggiungibile (No route to host): spento o in stop, indirizzo IP cambiato, \
                 oppure macOS non dà il permesso «Rete locale» all'app da cui parte DbCraft \
                 (Impostazioni di Sistema → Privacy e sicurezza → Rete locale)"
                    .into()
            } else if s.to_lowercase().contains("timed out") {
                "nessuna risposta (tempo scaduto)".into()
            } else {
                s
            }
        }
        other => other.to_string(),
    }
}

fn message_json(m: &Msg, send_image: bool) -> serde_json::Value {
    use base64::Engine;
    if !send_image || !m.has_images() {
        return serde_json::json!({ "role": m.role, "content": m.text });
    }
    let mut parts = Vec::new();
    if !m.text.is_empty() {
        parts.push(serde_json::json!({ "type": "text", "text": m.text }));
    }
    for img in m.image.iter().chain(m.images.iter()) {
        let b64 = base64::engine::general_purpose::STANDARD.encode(img.as_slice());
        parts.push(serde_json::json!({
            "type": "image_url",
            "image_url": { "url": format!("data:{};base64,{b64}", image_mime(img)) }
        }));
    }
    serde_json::json!({ "role": m.role, "content": parts })
}

/// Il tipo di un'immagine dai primi byte (PNG di serie).
pub fn image_mime(b: &[u8]) -> &'static str {
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if b.starts_with(b"GIF8") {
        "image/gif"
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/png"
    }
}

/// Strumenti offerti al modello in questa richiesta: quelli del database (`dbagent`).
pub struct ToolCtx {
    defs: Vec<serde_json::Value>,
    note: String,
    rounds: usize,
    db: crate::dbagent::DbAccess,
}

/// Esito di uno strumento per il giro con il modello.
struct ToolRun {
    text: String,
    short: String,
    opened: Option<String>,
}

impl ToolCtx {
    /// Gli strumenti della chat di DbCraft.
    pub fn full(cfg: &AiConfig, msgs: &[Msg]) -> Self {
        let db = crate::dbagent::DbAccess { allow_write: cfg.db.allow_write, max_rows: cfg.db.max_rows.clamp(1, 5000) };
        let mut note = format!("\n\nOggi è il {}. Hai a disposizione degli strumenti:", chrono::Local::now().format("%d/%m/%Y"));
        note.push_str(&crate::dbagent::system_note(&db));
        // una domanda nuova: le conferme chieste prima ora valgono, e l'SQL mostrato
        // nell'ultima risposta vale come approvato se l'utente ha risposto di sì
        let last_user = msgs.iter().rev().find(|m| m.role == "user").map(|m| m.text.as_str()).unwrap_or("");
        let shown = msgs.iter().rev().skip_while(|m| m.role != "user").skip(1).find(|m| m.role == "assistant").map(|m| m.text.as_str()).unwrap_or("");
        crate::dbagent::new_turn(shown, last_user);
        Self { defs: crate::dbagent::definitions(&db), note, rounds: 16, db }
    }

    /// Esegue uno strumento chiesto dal modello.
    fn run_tool(&self, name: &str, args: &serde_json::Value) -> ToolRun {
        let o = crate::dbagent::execute(&self.db, name, args);
        ToolRun { text: o.text, short: o.short, opened: o.opened }
    }
}

/// Giri massimi domanda → strumenti → risposta.
const MAX_ROUNDS: usize = 6;

/// Chi risponde, detto al modello: il servizio (con l'indirizzo) e il modello.
fn identity(p: &Provider, model: &str) -> String {
    let host = p.url.split("://").nth(1).unwrap_or(&p.url).split('/').next().unwrap_or("");
    format!(
        "Chi sei: in questa chat risponde il modello «{model}» del servizio «{}» ({host}), scelto dall'utente nelle impostazioni dell'app. \
         Se ti chiedono chi sei o che modello sei, rispondi con questo. Non dire di essere un altro modello o di un'altra azienda: \
         se di questo modello non sai altro, dillo invece di inventare.",
        p.name
    )
}

/// Una richiesta a un provider (in streaming), con gli strumenti se offerti:
/// il modello può chiedere i dati del database, le si esegue e si continua finché
/// risponde. Ritorna il testo pulito.
#[allow(clippy::too_many_arguments)]
fn call(
    p: &Provider,
    key: &str,
    model: &str,
    task: Task,
    msgs: &[Msg],
    send_image: bool,
    tools: Option<&ToolCtx>,
    on_event: &mut dyn FnMut(Event) -> bool,
) -> Result<String> {
    let context = context_cached(&p.url, model).unwrap_or(DEFAULT_CONTEXT);
    let url = format!("{}/chat/completions", base(p));
    let mut wire: Vec<serde_json::Value> = msgs.iter().map(|m| message_json(m, send_image)).collect();
    if let Some(t) = tools {
        match wire.first_mut() {
            Some(first) if first["role"] == "system" => {
                let s = format!("{}{}", first["content"].as_str().unwrap_or(""), t.note);
                first["content"] = serde_json::json!(s);
            }
            _ => wire.insert(0, serde_json::json!({ "role": "system", "content": t.note.trim() })),
        }
    }
    // in chat il modello sa chi è: da solo spesso non lo sa (DeepSeek a volte
    // dice di essere Claude) e lo inventa
    if task == Task::Chat {
        let who = identity(p, model);
        match wire.first_mut() {
            Some(first) if first["role"] == "system" => {
                let s = format!("{}\n\n{who}", first["content"].as_str().unwrap_or(""));
                first["content"] = serde_json::json!(s);
            }
            _ => wire.insert(0, serde_json::json!({ "role": "system", "content": who })),
        }
    }
    let max_rounds = tools.map_or(MAX_ROUNDS, |t| t.rounds);
    let mut tools = tools;
    let mut answer = String::new();
    let mut round = 0;
    while round < max_rounds {
        let mut body = serde_json::json!({ "model": model, "stream": true, "messages": wire });
        if p.stream_usage() {
            body["stream_options"] = serde_json::json!({ "include_usage": true });
        }
        let temp = p.temperature;
        if let (Some(t), false) = (temp, p.no_temperature) {
            body["temperature"] = serde_json::json!(t);
        }
        if let Some(n) = p.max_tokens {
            body["max_tokens"] = serde_json::json!(n);
        }
        if let Some(t) = tools {
            body["tools"] = serde_json::json!(t.defs);
            // all'ultimo giro deve rispondere, non cercare ancora
            body["tool_choice"] = serde_json::json!(if round + 1 < max_rounds { "auto" } else { "none" });
        }
        let sep = !answer.is_empty();
        let mut first = true;
        let out = stream_round(p, key, &url, &body, &mut |think, d| {
            if think {
                return on_event(Event::Thinking(d));
            }
            let d = if first && sep { format!("\n\n{d}") } else { d };
            first = false;
            on_event(Event::Delta(d))
        });
        let out = match out {
            Ok(o) => o,
            // server che non accetta gli strumenti: si riprova senza
            Err(e) if tools.is_some() && round == 0 && answer.is_empty() && e.to_string().to_lowercase().contains("tool") => {
                eprintln!("{}: strumenti non accettati ({e}), riprovo senza", p.name);
                tools = None;
                continue;
            }
            Err(e) => return Err(e),
        };
        if !out.text.is_empty() {
            if sep {
                answer.push_str("\n\n");
            }
            answer.push_str(&out.text);
        }
        // all'ultimo giro gli strumenti non si eseguono più (anche se il modello li chiede)
        let last = round + 1 >= max_rounds;
        if last && !out.calls.is_empty() && answer.trim().is_empty() {
            answer = crate::i18n::t("Mi sono fermato: servivano troppi passaggi con gli strumenti. Dimmi se continuo.").into();
            on_event(Event::Delta(answer.clone()));
        }
        let Some(t) = tools.filter(|_| !out.calls.is_empty() && !last) else {
            // token della conversazione: quelli del server se li dice
            let (used, exact) = match out.usage {
                Some(u) => (u, true),
                None => (estimate_wire(&wire) + estimate_text(&out.raw), false),
            };
            let output = out.output.unwrap_or_else(|| estimate_text(&answer) + estimate_text(&out.reasoning));
            on_event(Event::Usage { used, max: context, exact, output });
            return Ok(clean_reply(&answer));
        };
        // Il modello ha chiesto degli strumenti: si eseguono e si continua.
        let calls: Vec<serde_json::Value> = out
            .calls
            .iter()
            .map(|c| serde_json::json!({ "id": c.id, "type": "function", "function": { "name": c.name, "arguments": c.args } }))
            .collect();
        let mut asst = serde_json::json!({ "role": "assistant", "content": out.raw, "tool_calls": calls });
        if !out.reasoning.is_empty() {
            asst["reasoning_content"] = serde_json::json!(out.reasoning);
        }
        wire.push(asst);
        let mut pending_images: Vec<serde_json::Value> = Vec::new();
        for c in &out.calls {
            let args: serde_json::Value = serde_json::from_str(&c.args).unwrap_or(serde_json::json!({}));
            let label = crate::dbagent::label(&c.name, &args);
            if !on_event(Event::Tool { label: label.clone() }) {
                bail!("interrotto");
            }
            let run = t.run_tool(&c.name, &args);
            if !on_event(Event::ToolDone { label, result: run.short }) {
                bail!("interrotto");
            }
            if let Some(title) = run.opened {
                on_event(Event::OpenedQuery { title });
            }
            wire.push(serde_json::json!({ "role": "tool", "tool_call_id": c.id, "content": cap_tool_text(run.text, context) }));
        }
        // le immagini vanno dopo tutte le risposte degli strumenti
        wire.append(&mut pending_images);
        round += 1;
        // contesto quasi pieno: niente altri strumenti, deve rispondere
        if estimate_wire(&wire) as u64 * 100 > context as u64 * 85 {
            eprintln!("{}: contesto quasi pieno, l'ultimo giro risponde", p.name);
            round = round.max(max_rounds.saturating_sub(1));
        }
    }
    Ok(clean_reply(&answer))
}

/// Il risultato di uno strumento, al massimo una parte del contesto del
/// modello (una pagina web, un documento lungo, un server MCP): il resto si
/// taglia e glielo si dice.
fn cap_tool_text(text: String, context: u32) -> String {
    // ~4 caratteri per token, al massimo un quarto del contesto (e 30 000)
    let cap = (context as usize).clamp(8_192, 120_000).min(30_000);
    if text.chars().count() <= cap {
        return text;
    }
    let cut: String = text.chars().take(cap).collect();
    format!(
        "{cut}\n\n[… risultato tagliato: altri {} caratteri. Se serve il resto, chiedine un pezzo alla volta.]",
        text.chars().count() - cap
    )
}

struct ToolCall {
    id: String,
    name: String,
    args: String,
}

struct RoundOut {
    /// Token totali (domanda + risposta) se il server li dice.
    usage: Option<u32>,
    /// Token della risposta (con il ragionamento) se il server li dice.
    output: Option<u32>,
    /// Testo visibile (senza ragionamento).
    text: String,
    /// Testo così come arrivato (per rimandarlo al modello).
    raw: String,
    reasoning: String,
    calls: Vec<ToolCall>,
}

/// Un giro: invia la richiesta e legge la risposta (testo e strumenti chiesti).
fn stream_round(
    p: &Provider,
    key: &str,
    url: &str,
    body: &serde_json::Value,
    on_delta: &mut dyn FnMut(bool, String) -> bool,
) -> Result<RoundOut> {
    let mut resp = http(p, key, url, Some(body))?;
    if resp.status >= 400 {
        bail!("{}", http_error(&mut resp, key));
    }

    // con gli strumenti, le chiamate scritte come testo non si mostrano (vedi `text_calls`)
    let mut filt = ThinkFilter { cut_calls: body.get("tools").is_some(), ..Default::default() };
    let mut reasoning = String::new();
    let mut calls: Vec<ToolCall> = Vec::new();
    let mut usage: Option<u32> = None;
    let mut output: Option<u32> = None;
    let read_output = |v: &serde_json::Value| v["usage"]["completion_tokens"].as_u64().map(|x| x as u32).filter(|&x| x > 0);
    let read_usage = |v: &serde_json::Value| -> Option<u32> {
        let u = &v["usage"];
        u["total_tokens"]
            .as_u64()
            .or_else(|| Some(u["prompt_tokens"].as_u64()? + u["completion_tokens"].as_u64().unwrap_or(0)))
            .map(|x| x as u32)
            .filter(|&x| x > 0)
    };
    let add_call = |i: usize, c: &serde_json::Value, calls: &mut Vec<ToolCall>| {
        while calls.len() <= i {
            calls.push(ToolCall { id: String::new(), name: String::new(), args: String::new() });
        }
        let t = &mut calls[i];
        if let Some(id) = c["id"].as_str() {
            t.id = id.to_string();
        }
        if let Some(n) = c["function"]["name"].as_str() {
            t.name.push_str(n);
        }
        match &c["function"]["arguments"] {
            serde_json::Value::String(a) => t.args.push_str(a),
            serde_json::Value::Null => {}
            other => t.args.push_str(&other.to_string()),
        }
    };
    // Alcuni server ignorano "stream": risposta JSON unica.
    if resp.ctype.contains("json") {
        let v: serde_json::Value = serde_json::from_reader(resp.body)?;
        usage = read_usage(&v);
        output = read_output(&v);
        let m = &v["choices"][0]["message"];
        if let Some(r) = m["reasoning_content"].as_str().filter(|r| !r.is_empty())
            && !on_delta(true, r.to_string())
        {
            bail!("interrotto");
        }
        if let Some(text) = m["content"].as_str() {
            let (d, t) = filt.push2(text);
            if let Some(t) = t
                && !on_delta(true, t)
            {
                bail!("interrotto");
            }
            if let Some(d) = d
                && !on_delta(false, d)
            {
                bail!("interrotto");
            }
        } else if m["tool_calls"].is_null() {
            bail!("risposta inattesa: {v}");
        }
        if let Some(r) = m["reasoning_content"].as_str() {
            reasoning.push_str(r);
        }
        for (i, c) in m["tool_calls"].as_array().into_iter().flatten().enumerate() {
            add_call(i, c, &mut calls);
        }
    } else {
        // righe di sola compagnia (": keep-alive") senza dati per troppo tempo = fermo
        // prima dei primi dati si aspetta come per la prima risposta
        let (idle, first) = (Duration::from_secs(idle_secs(p)), Duration::from_secs(first_secs(p)));
        let mut last_data = std::time::Instant::now();
        let mut got_data = false;
        for line in resp.body.lines() {
            let line = line.map_err(|e| anyhow!("lettura: {e}"))?;
            let Some(data) = line.strip_prefix("data:") else {
                let limit = if got_data { idle } else { first };
                if last_data.elapsed() > limit {
                    bail!("nessuna risposta da {} s", limit.as_secs());
                }
                continue;
            };
            last_data = std::time::Instant::now();
            got_data = true;
            let data = data.trim();
            if data == "[DONE]" {
                break;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else { continue };
            if let Some(msg) = v["error"]["message"].as_str() {
                bail!("{msg}");
            }
            if let Some(u) = read_usage(&v) {
                usage = Some(u);
            }
            if let Some(o) = read_output(&v) {
                output = Some(o);
            }
            let delta = &v["choices"][0]["delta"];
            if let Some(r) = delta["reasoning_content"].as_str().filter(|r| !r.is_empty()) {
                reasoning.push_str(r);
                if !on_delta(true, r.to_string()) {
                    bail!("interrotto");
                }
            }
            if let Some(piece) = delta["content"].as_str() {
                let (d, t) = filt.push2(piece);
                if let Some(t) = t
                    && !on_delta(true, t)
                {
                    bail!("interrotto");
                }
                if let Some(d) = d
                    && !on_delta(false, d)
                {
                    bail!("interrotto");
                }
            }
            for (j, c) in delta["tool_calls"].as_array().into_iter().flatten().enumerate() {
                let i = c["index"].as_u64().map(|x| x as usize).unwrap_or(j);
                add_call(i, c, &mut calls);
            }
        }
    }
    calls.retain(|c| !c.name.is_empty());
    // chiamate scritte come testo (DeepSeek DSML, <tool_call>, [TOOL_CALLS]…): si eseguono lo stesso,
    // e al modello si rimanda il testo senza il markup
    let mut raw = filt.full.clone();
    if calls.is_empty() && filt.cut_calls {
        calls = text_calls(&filt.full);
        if !calls.is_empty() {
            raw = filt.visible().to_string();
        }
    }
    for (i, c) in calls.iter_mut().enumerate() {
        if c.id.is_empty() {
            c.id = format!("call_{i}");
        }
        if c.args.trim().is_empty() {
            c.args = "{}".into();
        }
    }
    Ok(RoundOut { usage, output, text: clean_reply(filt.visible()), raw, reasoning, calls })
}

// ---------------------------------------------------------------------------
// HTTP: ureq, oppure curl di sistema
// ---------------------------------------------------------------------------

/// Risposta HTTP con il corpo da leggere (anche a righe, per lo streaming).
struct HttpResp {
    status: u16,
    ctype: String,
    body: Box<dyn BufRead + Send>,
    /// Con curl: il processo e il file temporaneo, chiusi a fine lettura.
    _curl: Option<CurlGuard>,
}

/// «HTTP 401: messaggio del server», con un suggerimento quando il server
/// vuole una chiave e non ne è stata data nessuna.
fn http_error(resp: &mut HttpResp, key: &str) -> String {
    let code = resp.status;
    let mut txt = String::new();
    let _ = std::io::Read::read_to_string(&mut resp.body, &mut txt);
    let msg = serde_json::from_str::<serde_json::Value>(&txt)
        .ok()
        .and_then(|v| {
            v["error"]["message"]
                .as_str()
                .or(v[0]["error"]["message"].as_str())
                .or(v["message"].as_str())
                .or(v["detail"].as_str())
                .map(String::from)
        })
        .unwrap_or(txt);
    let msg: String = msg.trim().chars().take(300).collect();
    let mut out = if msg.is_empty() { format!("HTTP {code}") } else { format!("HTTP {code}: {msg}") };
    if matches!(code, 401 | 403) && key.trim().is_empty() {
        out.push_str(" — il server chiede una chiave API: impostala in «Chiave»");
    }
    out
}

/// GET (senza corpo) o POST JSON verso un provider.
///
/// Su macOS un programma compilato in proprio (come questo, lanciato da
/// cargo) senza il permesso «Rete locale» riceve «No route to host» verso
/// gli altri computer di casa, mentre `/usr/bin/curl` di Apple passa. In quel
/// caso si ripete la richiesta con curl: il server locale risponde lo stesso.
fn http(p: &Provider, key: &str, url: &str, body: Option<&serde_json::Value>) -> Result<HttpResp> {
    let req = match body {
        Some(_) => with_auth(agent(p).post(url), key).set("Accept", "text/event-stream"),
        None => with_auth(agent(p).get(url), key),
    };
    let res = match body {
        Some(b) => req.send_json(b.clone()),
        None => req.call(),
    };
    let to_resp = |r: ureq::Response| HttpResp {
        status: r.status(),
        ctype: r.content_type().to_string(),
        body: Box::new(BufReader::new(r.into_reader())),
        _curl: None,
    };
    match res {
        Ok(r) => Ok(to_resp(r)),
        Err(ureq::Error::Status(_, r)) => Ok(to_resp(r)),
        Err(e) if no_route(&e) && curl_available() => {
            // avvisa una volta sola, poi in silenzio
            static ONCE: std::sync::Once = std::sync::Once::new();
            ONCE.call_once(|| {
                eprintln!("macOS blocca la rete locale a questo programma (permesso «Rete locale»): uso curl")
            });
            http_curl(url, key, body, first_secs(p))
                .map_err(|ce| anyhow!("{} — anche con curl: {ce}", transport_msg(&e)))
        }
        Err(e) => bail!("{}", transport_msg(&e)),
    }
}

fn no_route(e: &ureq::Error) -> bool {
    let s = e.to_string();
    s.contains("os error 65") || s.contains("No route to host") || s.contains("os error 113")
}

fn curl_available() -> bool {
    std::path::Path::new(CURL).exists()
}

const CURL: &str = "/usr/bin/curl";

struct CurlGuard {
    child: std::process::Child,
    body_file: Option<std::path::PathBuf>,
}

impl Drop for CurlGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(f) = &self.body_file {
            let _ = std::fs::remove_file(f);
        }
    }
}

/// Stringa per il file di configurazione di curl (tra virgolette).
fn curl_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// La stessa richiesta fatta da curl. La chiave passa nella configurazione
/// letta da stdin (non compare fra i processi), il corpo da un file temporaneo.
fn http_curl(url: &str, key: &str, body: Option<&serde_json::Value>, timeout_s: u64) -> Result<HttpResp> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    let mut cfg = format!("url = {}\n", curl_quote(url));
    if !key.is_empty() {
        cfg.push_str(&format!("header = {}\n", curl_quote(&format!("Authorization: Bearer {key}"))));
    }
    let mut body_file = None;
    if let Some(b) = body {
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let f = std::env::temp_dir().join(format!("openslate-ai-{}-{n}.json", std::process::id()));
        {
            let mut o = std::fs::OpenOptions::new();
            o.write(true).create(true).truncate(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
            o.open(&f)?.write_all(b.to_string().as_bytes())?;
        }
        cfg.push_str("request = \"POST\"\n");
        cfg.push_str("header = \"Content-Type: application/json\"\n");
        cfg.push_str("header = \"Accept: text/event-stream\"\n");
        cfg.push_str(&format!("data-binary = {}\n", curl_quote(&format!("@{}", f.display()))));
        body_file = Some(f);
    }
    let mut child = Command::new(CURL)
        .args(["-sS", "-i", "-N", "--connect-timeout", "4", "--speed-limit", "1", "--speed-time"])
        .arg(timeout_s.max(5).to_string())
        .args(["-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("avvio di curl")?;
    child.stdin.take().ok_or_else(|| anyhow!("curl senza stdin"))?.write_all(cfg.as_bytes())?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("curl senza stdout"))?;
    let stderr = child.stderr.take();
    let mut guard = CurlGuard { child, body_file };
    let mut reader = BufReader::new(stdout);

    // Intestazioni (-i): si saltano le risposte intermedie «100 Continue».
    let (mut status, mut ctype) = (0u16, String::new());
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            let _ = guard.child.wait();
            let mut err = String::new();
            if let Some(mut e) = stderr {
                let _ = std::io::Read::read_to_string(&mut e, &mut err);
            }
            let err = err.trim().trim_start_matches("curl: ").to_string();
            bail!("{}", if err.is_empty() { "nessuna risposta".to_string() } else { err });
        }
        let l = line.trim_end();
        if l.starts_with("HTTP/") {
            status = l.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
        } else if let Some((k, v)) = l.split_once(':') {
            if k.eq_ignore_ascii_case("content-type") {
                ctype = v.trim().to_lowercase();
            }
        } else if l.is_empty() && status >= 200 {
            break;
        }
    }
    Ok(HttpResp { status, ctype, body: Box::new(reader), _curl: Some(guard) })
}

/// Nasconde il ragionamento `<think>…</think>` mentre arriva in streaming
/// (e lo passa a parte).
#[derive(Default)]
struct ThinkFilter {
    /// Taglia il testo dove cominciano chiamate agli strumenti scritte come testo.
    cut_calls: bool,
    full: String,
    sent: usize,
    think_sent: usize,
}

impl ThinkFilter {
    fn visible(&self) -> &str {
        let t = self.after_think();
        if !self.cut_calls {
            return t;
        }
        match calls_start(t) {
            Some(i) => t[..i].trim_end(),
            // la fine potrebbe essere l'inizio di un segno di chiamata: si aspetta il resto
            None => &t[..t.len() - partial_marker(t)],
        }
    }

    fn after_think(&self) -> &str {
        let t = self.full.trim_start();
        if let Some(rest) = t.strip_prefix("<think>") {
            match rest.find("</think>") {
                Some(i) => rest[i + "</think>".len()..].trim_start(),
                None => "",
            }
        } else if "<think>".starts_with(t) {
            "" // potrebbe essere l'inizio di <think>
        } else {
            t
        }
    }

    /// Il ragionamento dentro `<think>` arrivato finora.
    fn thinking(&self) -> &str {
        let t = self.full.trim_start();
        match t.strip_prefix("<think>") {
            Some(rest) => match rest.find("</think>") {
                Some(i) => &rest[..i],
                // la fine potrebbe essere a metà di «</think>»
                None => {
                    let keep = (1..="</think>".len()).rev().find(|&k| rest.ends_with(&"</think>"[..k])).unwrap_or(0);
                    &rest[..rest.len() - keep]
                }
            },
            None => "",
        }
    }

    /// Aggiunge un pezzo; ritorna il nuovo testo visibile e il nuovo
    /// ragionamento (se ci sono).
    fn push2(&mut self, piece: &str) -> (Option<String>, Option<String>) {
        let vis = self.push(piece);
        let th = self.thinking();
        let t = if th.len() > self.think_sent && th.is_char_boundary(self.think_sent) {
            let d = th[self.think_sent..].to_string();
            self.think_sent = th.len();
            Some(d)
        } else {
            None
        };
        (vis, t)
    }

    /// Aggiunge un pezzo; ritorna il nuovo testo visibile (se c'è).
    fn push(&mut self, piece: &str) -> Option<String> {
        self.full.push_str(piece);
        let vis = self.visible();
        if vis.len() > self.sent {
            let d = vis[self.sent..].to_string();
            self.sent = vis.len();
            Some(d)
        } else {
            None
        }
    }
}

/// Dove i modelli scrivono le chiamate agli strumenti come testo, quando il server non le
/// traduce in `tool_calls`: DeepSeek (DSML e il formato precedente), Qwen/Hermes, Mistral.
const CALL_MARKERS: [&str; 5] = ["<｜tool▁calls▁begin｜>", "<｜tool▁call▁begin｜>", "<tool_call>", "[TOOL_CALLS]", "<function_calls>"];

/// Inizio della prima chiamata scritta come testo, se c'è.
fn calls_start(t: &str) -> Option<usize> {
    let mut best = CALL_MARKERS.iter().filter_map(|m| t.find(m)).min();
    // DSML: «<｜DSML｜function_calls>», a volte con le barre doppie o con spazi
    let mut from = 0;
    while let Some(k) = t[from..].find("DSML") {
        let at = from + k;
        let mut h = at.saturating_sub(12);
        while !t.is_char_boundary(h) {
            h += 1;
        }
        if let Some(lt) = t[h..at].rfind('<') {
            let i = h + lt;
            best = Some(best.map_or(i, |b| b.min(i)));
            break;
        }
        from = at + 4;
    }
    best
}

/// Byte finali che potrebbero essere l'inizio di un segno di chiamata (da non mostrare ancora).
fn partial_marker(t: &str) -> usize {
    let mut keep = 0;
    for m in CALL_MARKERS.iter().copied().chain(["<｜DSML｜", "<｜｜DSML"]) {
        for (k, _) in m.char_indices().skip(1) {
            if k > keep && t.ends_with(&m[..k]) {
                keep = k;
            }
        }
    }
    keep
}

/// Le chiamate agli strumenti scritte come testo (vedi [`CALL_MARKERS`]).
fn text_calls(full: &str) -> Vec<ToolCall> {
    let Some(start) = calls_start(full) else { return Vec::new() };
    let t = &full[start..];
    let mut out = Vec::new();
    let mut push = |name: &str, args: String| {
        let name = name.trim();
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
            out.push(ToolCall { id: String::new(), name: name.to_string(), args });
        }
    };
    // DSML e simili: <invoke name="db_query"><parameter name="sql" string="true">…</parameter></invoke>
    if t.contains("invoke name=\"") {
        for inv in t.split("invoke name=\"").skip(1) {
            let Some(q) = inv.find('"') else { continue };
            let mut args = serde_json::Map::new();
            for par in inv.split("parameter name=\"").skip(1) {
                let (Some(q2), Some(gt)) = (par.find('"'), par.find('>')) else { continue };
                let attrs = &par[q2..gt];
                let body = &par[gt + 1..];
                let end = body.find("parameter>").and_then(|e| body[..e].rfind("</")).unwrap_or(body.len());
                let raw = &body[..end];
                let v = if attrs.contains("string=\"false\"") { serde_json::from_str(raw.trim()).unwrap_or_else(|_| serde_json::json!(raw)) } else { serde_json::json!(raw) };
                args.insert(par[..q2].to_string(), v);
            }
            push(&inv[..q], serde_json::Value::Object(args).to_string());
        }
        return out;
    }
    // DeepSeek: <｜tool▁call▁begin｜>function<｜tool▁sep｜>nome\n```json\n{…}\n```<｜tool▁call▁end｜>
    if t.contains("<｜tool▁sep｜>") {
        for c in t.split("<｜tool▁sep｜>").skip(1) {
            let c = c.split("<｜tool▁call▁end｜>").next().unwrap_or(c);
            let name = c.lines().next().unwrap_or("");
            let args = match (c.find('{'), c.rfind('}')) {
                (Some(a), Some(b)) if b > a => c[a..=b].to_string(),
                _ => "{}".into(),
            };
            push(name, args);
        }
        return out;
    }
    // Qwen/Hermes: <tool_call>{"name": …, "arguments": {…}}</tool_call>; Mistral: [TOOL_CALLS][{…}]
    let mut objs: Vec<serde_json::Value> = Vec::new();
    for c in t.split("<tool_call>").skip(1) {
        let c = c.split("</tool_call>").next().unwrap_or(c);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(c.trim()) {
            objs.push(v);
        }
    }
    if let Some(rest) = t.strip_prefix("[TOOL_CALLS]")
        && let Ok(serde_json::Value::Array(a)) = serde_json::from_str::<serde_json::Value>(rest.trim())
    {
        objs.extend(a);
    }
    for v in objs {
        let args = match &v["arguments"] {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => v["parameters"].to_string(),
            a => a.to_string(),
        };
        push(v["name"].as_str().unwrap_or(""), if args == "null" { "{}".into() } else { args });
    }
    out
}

/// Pulisce la risposta finale: ragionamento, ``` attorno.
pub fn clean_reply(s: &str) -> String {
    let s = match s.rfind("</think>") {
        Some(i) => &s[i + "</think>".len()..],
        None => s,
    };
    s.trim().to_string()
}



// ---------------------------------------------------------------------------
// Contesto della chat: dove si trova l'utente in DbCraft
// ---------------------------------------------------------------------------

/// Il database selezionato in DbCraft (connessione, database, schema), detto alla chat.
#[derive(Clone, Debug, Default)]
pub struct ChatContext {
    pub title: String,
    pub conn: String,
    pub db: String,
    pub schema: Option<String>,
    /// Cresce a ogni cambio (la pagina web se ne accorge).
    pub rev: u64,
}

static CONTEXT: Mutex<Option<ChatContext>> = Mutex::new(None);

/// Il database selezionato nell'albero (lo chiama l'app quando cambia).
pub fn set_context(title: &str, conn: &str, db: &str, schema: Option<&str>) {
    let Ok(mut g) = CONTEXT.lock() else { return };
    if g.as_ref().is_some_and(|c| c.conn == conn && c.db == db && c.schema.as_deref() == schema && c.title == title) {
        return;
    }
    let rev = g.as_ref().map(|c| c.rev + 1).unwrap_or(1);
    *g = Some(ChatContext { title: title.into(), conn: conn.into(), db: db.into(), schema: schema.map(String::from), rev });
}

pub fn context() -> Option<ChatContext> {
    CONTEXT.lock().ok()?.clone()
}

/// Messaggi per il modello: istruzioni, database selezionato, conversazione.
/// (ruolo, testo, immagini incollate) di un messaggio della conversazione.
pub type HistoryMsg = (String, String, Vec<Arc<Vec<u8>>>);

pub fn chat_messages_with_images(cfg: &AiConfig, history: &[HistoryMsg], ctx: Option<&ChatContext>) -> Vec<Msg> {
    let mut system = cfg.chat_prompt().to_string();
    if let Some(c) = ctx.filter(|c| !c.conn.is_empty()) {
        system.push_str(&format!(
            "\n\nIn DbCraft l'utente ha selezionato: {}. Se non dice altro, le domande sono su questo database.",
            c.title
        ));
    }
    let mut msgs = vec![Msg::new("system", system)];
    for (role, text, pasted) in history {
        // il riassunto di una compattazione arriva come messaggio dell'utente
        let (role, text) = if role == "summary" {
            ("user", format!("[Riassunto della conversazione precedente, compattata per spazio]\n\n{text}"))
        } else {
            (role.as_str(), text.clone())
        };
        msgs.push(Msg::new(role, text).with_images(pasted.clone()));
    }
    msgs
}


/// Stato dei provider (per la pagina della chat).
pub fn describe(cfg: &AiConfig) -> Vec<(String, String)> {
    cfg.providers
        .iter()
        .map(|p| {
            let state = if !p.enabled {
                "disattivato".to_string()
            } else if resolve_key(&p.key).is_none() {
                format!("chiave non trovata ({})", p.key.trim())
            } else {
                let m = if p.model.trim().is_empty() { "automatico" } else { p.model.trim() };
                format!("pronto — modello {m}")
            };
            (p.name.clone(), state)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(pieces: &[&str]) -> (String, ThinkFilter) {
        let mut f = ThinkFilter { cut_calls: true, ..Default::default() };
        let shown: String = pieces.iter().filter_map(|p| f.push(p)).collect();
        (shown, f)
    }

    #[test]
    fn dsml_calls_hidden_and_parsed() {
        let text = "Vedo 35 tabelle.\n\n<｜DSML｜function_calls>\n<｜DSML｜invoke name=\"db_query\">\n<｜DSML｜parameter name=\"sql\" string=\"true\">SELECT COUNT(*) n FROM t;</｜DSML｜parameter>\n<｜DSML｜parameter name=\"limit\" string=\"false\">5</｜DSML｜parameter>\n</｜DSML｜invoke>\n<｜DSML｜invoke name=\"db_tables\">\n</｜DSML｜invoke>\n</｜DSML｜function_calls>";
        // a pezzi piccoli, anche a metà dei caratteri del segno
        let pieces: Vec<String> = text.chars().collect::<Vec<_>>().chunks(3).map(|c| c.iter().collect()).collect();
        let refs: Vec<&str> = pieces.iter().map(String::as_str).collect();
        let (shown, f) = stream(&refs);
        assert_eq!(shown.trim_end(), "Vedo 35 tabelle.");
        let calls = text_calls(&f.full);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "db_query");
        let a: serde_json::Value = serde_json::from_str(&calls[0].args).unwrap_or_default();
        assert_eq!(a["sql"], "SELECT COUNT(*) n FROM t;");
        assert_eq!(a["limit"], 5);
        assert_eq!(calls[1].name, "db_tables");
    }

    #[test]
    fn dsml_double_bars() {
        let t = "Ok.\n<｜｜DSML｜｜ calls>\n<｜｜DSML｜｜ invoke name=\"db_query\">\n<｜｜DSML｜｜ parameter name=\"sql\" string=\"true\">SELECT 1;</｜｜DSML｜｜ parameter>\n</｜｜DSML｜｜ invoke>\n</｜｜DSML｜｜ calls>";
        let (shown, f) = stream(&[t]);
        assert_eq!(shown.trim_end(), "Ok.");
        let calls = text_calls(&f.full);
        assert_eq!(calls.len(), 1);
        assert!(calls[0].args.contains("SELECT 1;"));
    }

    #[test]
    fn hermes_deepseek_mistral() {
        let c = text_calls("x <tool_call>{\"name\": \"db_describe\", \"arguments\": {\"table\": \"users\"}}</tool_call>");
        assert_eq!((c.len(), c[0].name.as_str()), (1, "db_describe"));
        assert!(c[0].args.contains("users"));
        let c = text_calls("<｜tool▁calls▁begin｜><｜tool▁call▁begin｜>function<｜tool▁sep｜>db_query\n```json\n{\"sql\": \"SELECT 2\"}\n```<｜tool▁call▁end｜><｜tool▁calls▁end｜>");
        assert_eq!((c.len(), c[0].name.as_str()), (1, "db_query"));
        assert!(c[0].args.contains("SELECT 2"));
        let c = text_calls("[TOOL_CALLS][{\"name\": \"db_tables\", \"arguments\": {}}]");
        assert_eq!((c.len(), c[0].name.as_str()), (1, "db_tables"));
    }

    #[test]
    fn plain_text_untouched() {
        let (shown, f) = stream(&["a < b and ", "c > d"]);
        assert_eq!(shown, "a < b and c > d");
        assert!(text_calls(&f.full).is_empty());
        // senza strumenti nella richiesta il testo resta com'è
        let mut f = ThinkFilter::default();
        assert_eq!(f.push("<tool_call>{}</tool_call>").as_deref(), Some("<tool_call>{}</tool_call>"));
    }
}
