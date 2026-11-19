//! Interface languages, as in OpenSlate. The text in the code is Italian and is the key: the
//! tables `src/i18n/<lang>.json` say how it is written in the other languages. Keys with
//! placeholders (`"Eliminare la tabella \"{}\"?"`, `"{n} tabelle"`) also translate texts built
//! at run time: the formatted text is recognised from the pattern and the variable parts go into
//! the translation.
//!
//! Language: the system's (if it is one of ours, otherwise English) or the one chosen in
//! Visualizza → Lingua (`ui_lang` in settings.json).

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    It,
    En,
    Es,
    Fr,
    De,
    Uk,
    Zh,
    Ja,
    Ko,
}

impl Lang {
    pub const ALL: [Lang; 9] = [Lang::It, Lang::En, Lang::Es, Lang::Fr, Lang::De, Lang::Uk, Lang::Zh, Lang::Ja, Lang::Ko];

    /// Codice ISO (settings.conf, pagine web).
    pub fn code(self) -> &'static str {
        match self {
            Lang::It => "it",
            Lang::En => "en",
            Lang::Es => "es",
            Lang::Fr => "fr",
            Lang::De => "de",
            Lang::Uk => "uk",
            Lang::Zh => "zh",
            Lang::Ja => "ja",
            Lang::Ko => "ko",
        }
    }

    /// Nome nella lingua stessa (menu delle lingue).
    pub fn native(self) -> &'static str {
        match self {
            Lang::It => "Italiano",
            Lang::En => "English",
            Lang::Es => "Español",
            Lang::Fr => "Français",
            Lang::De => "Deutsch",
            Lang::Uk => "Українська",
            Lang::Zh => "中文",
            Lang::Ja => "日本語",
            Lang::Ko => "한국어",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        let c = code.trim().to_ascii_lowercase();
        let c = c.split(['-', '_', '.']).next().unwrap_or("");
        Lang::ALL.into_iter().find(|l| l.code() == c)
    }

    /// Scrittura che chiede un carattere CJK (cinese, giapponese, coreano).
    pub fn cjk(self) -> bool {
        matches!(self, Lang::Zh | Lang::Ja | Lang::Ko)
    }

    fn table(self) -> Option<&'static str> {
        Some(match self {
            Lang::It => return None,
            Lang::En => include_str!("i18n/en.json"),
            Lang::Es => include_str!("i18n/es.json"),
            Lang::Fr => include_str!("i18n/fr.json"),
            Lang::De => include_str!("i18n/de.json"),
            Lang::Uk => include_str!("i18n/uk.json"),
            Lang::Zh => include_str!("i18n/zh.json"),
            Lang::Ja => include_str!("i18n/ja.json"),
            Lang::Ko => include_str!("i18n/ko.json"),
        })
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn lang() -> Lang {
    Lang::ALL[CURRENT.load(Ordering::Relaxed) as usize % Lang::ALL.len()]
}

pub fn set_lang(l: Lang) {
    let i = Lang::ALL.iter().position(|x| *x == l).unwrap_or(0);
    CURRENT.store(i as u8, Ordering::Relaxed);
    if let Ok(mut c) = CACHE.lock() {
        *c = None;
    }
}

/// La lingua da usare per `ui_lang` di settings.json: `auto` = quella del
/// sistema se la conosciamo, altrimenti inglese.
pub fn resolve(setting: &str) -> Lang {
    match setting.trim() {
        "" | "auto" => system(),
        s => Lang::from_code(s).unwrap_or_else(system),
    }
}

/// Lingua del sistema (macOS: la prima delle lingue preferite).
pub fn system() -> Lang {
    sys_locale::get_locales().find_map(|l| Lang::from_code(&l)).unwrap_or(Lang::En)
}

// ---------------------------------------------------------------------------
// Tabelle
// ---------------------------------------------------------------------------

struct Table {
    exact: HashMap<String, String>,
    /// Le traduzioni stesse: un testo già tradotto non si ritraduce.
    done: std::collections::HashSet<String>,
    /// Chiavi con segnaposto, dalla più specifica (più testo fisso).
    patterns: Vec<(Pattern, String)>,
}

static TABLES: OnceLock<Vec<Option<Table>>> = OnceLock::new();
static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

fn tables() -> &'static [Option<Table>] {
    TABLES.get_or_init(|| {
        let parse = |l: Lang| -> Option<HashMap<String, String>> {
            match serde_json::from_str::<HashMap<String, String>>(l.table()?) {
                Ok(m) => Some(m.into_iter().filter(|(_, v)| !v.is_empty()).collect()),
                Err(e) => {
                    eprintln!("traduzioni {}: {e}", l.code());
                    None
                }
            }
        };
        let en = parse(Lang::En).unwrap_or_default();
        Lang::ALL
            .iter()
            .map(|&l| {
                l.table()?;
                // quello che manca in una lingua si mostra in inglese
                let map: HashMap<String, String> = if l == Lang::En {
                    en.clone()
                } else {
                    let mut m = en.clone();
                    m.extend(parse(l).unwrap_or_default());
                    m
                };
                let mut patterns: Vec<(Pattern, String)> = map
                    .iter()
                    .filter(|(_, v)| !v.is_empty())
                    .filter_map(|(k, v)| Pattern::new(k).map(|p| (p, v.clone())))
                    .collect();
                patterns.sort_by_key(|(p, _)| std::cmp::Reverse(p.fixed_len));
                let exact: HashMap<String, String> = map.into_iter().filter(|(_, v)| !v.is_empty()).collect();
                let done = exact.values().map(|v| v.trim().to_string()).collect();
                Some(Table { exact, done, patterns })
            })
            .collect()
    })
}

fn table() -> Option<&'static Table> {
    let i = Lang::ALL.iter().position(|x| *x == lang())?;
    tables().get(i)?.as_ref()
}

/// Testo fisso: la sua traduzione (o lui stesso, in italiano o se manca).
pub fn t(it: &'static str) -> &'static str {
    match table().and_then(|tb| tb.exact.get(it)) {
        Some(s) => s.as_str(),
        None => it,
    }
}

/// Qualsiasi testo, anche composto a runtime: traduzione esatta, altrimenti
/// col modello che gli corrisponde, altrimenti com'è.
pub fn tr(s: &str) -> Cow<'_, str> {
    let Some(tb) = table() else { return Cow::Borrowed(s) };
    if s.is_empty() {
        return Cow::Borrowed(s);
    }
    if let Some(v) = tb.exact.get(s) {
        return Cow::Borrowed(v.as_str());
    }
    if tb.done.contains(s.trim()) {
        return Cow::Borrowed(s);
    }
    if let Ok(mut c) = CACHE.lock() {
        let c = c.get_or_insert_with(HashMap::new);
        if let Some(v) = c.get(s) {
            return Cow::Owned(v.clone());
        }
        let out = by_pattern(tb, s, 2).or_else(|| compound(tb, s)).unwrap_or_else(|| {
            missing(s);
            s.to_string()
        });
        if c.len() > 4000 {
            c.clear();
        }
        c.insert(s.to_string(), out.clone());
        return Cow::Owned(out);
    }
    Cow::Borrowed(s)
}

/// Col modello che corrisponde; anche i pezzi variabili si traducono, se sono
/// testi noti («Dal pad: {} nell'archivio» con dentro «2 pagine nuove»).
fn by_pattern(tb: &Table, s: &str, depth: u8) -> Option<String> {
    tb.patterns.iter().find_map(|(p, v)| {
        p.apply_with(s, v, &|c: &str| {
            if c.trim().is_empty() || !c.chars().any(|ch| ch.is_alphabetic()) {
                return None;
            }
            tb.exact.get(c).cloned().or_else(|| if depth > 0 { by_pattern(tb, c, depth - 1) } else { None })
        })
    })
}

/// Messaggi composti che nessuna chiave copre per intero: «⚠ …» e catene di
/// errori «a: b: c» (ogni pezzo si traduce da sé).
fn compound(tb: &Table, s: &str) -> Option<String> {
    let one = |p: &str| -> Option<String> { tb.exact.get(p).cloned().or_else(|| by_pattern(tb, p, 1)) };
    if let Some(rest) = s.strip_prefix("⚠ ") {
        let r = one(rest).or_else(|| compound(tb, rest)).unwrap_or_else(|| rest.to_string());
        return Some(format!("⚠ {r}"));
    }
    if !s.contains(": ") {
        return None;
    }
    let parts: Vec<&str> = s.split(": ").collect();
    let mut changed = false;
    let out: Vec<String> = parts
        .iter()
        .map(|p| match one(p) {
            Some(t) => {
                changed = true;
                t
            }
            None => p.to_string(),
        })
        .collect();
    changed.then(|| out.join(": "))
}

/// Per chi traduce: con `DBCRAFT_I18N_MISSING=<file>` i testi mostrati
/// senza traduzione finiscono in quel file (uno per riga, senza ripetizioni).
fn missing(s: &str) {
    static SEEN: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);
    let Some(path) = std::env::var_os("DBCRAFT_I18N_MISSING") else { return };
    if !s.chars().any(|c| c.is_alphabetic()) {
        return;
    }
    let Ok(mut g) = SEEN.lock() else { return };
    if g.get_or_insert_with(Default::default).insert(s.to_string()) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{}", s.replace('\n', "\\n"));
        }
    }
}

/// La tabella della lingua attuale in JSON, per le pagine web (chat, impostazioni AI).
pub fn table_json() -> String {
    match table() {
        Some(tb) => serde_json::to_string(&tb.exact).unwrap_or_else(|_| "{}".into()),
        None => "{}".into(),
    }
}

/// Come `tr`, per una `String` già fatta (es. `format!`).
pub fn tf(s: String) -> String {
    // (a borrowed result may be the exact translation, not `s` itself)
    tr(&s).into_owned()
}

// ---------------------------------------------------------------------------
// Modelli con segnaposto
// ---------------------------------------------------------------------------

/// Una chiave come `"Salvati {n} file in {}"`: pezzi fissi e segnaposto.
struct Pattern {
    /// Pezzi fissi, uno in più dei segnaposto (anche vuoti).
    fixed: Vec<String>,
    /// Nomi dei segnaposto (`""` per `{}`, `"n"` per `{n}`, `":.0"` per `{:.0}`).
    holes: Vec<String>,
    fixed_len: usize,
}

/// Divide un testo in stile `format!`: pezzi fissi e segnaposto.
fn split_format(s: &str) -> (Vec<String>, Vec<String>) {
    let mut fixed = vec![String::new()];
    let mut holes = Vec::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                if let Some(f) = fixed.last_mut() {
                    f.push('{');
                }
            }
            '}' if it.peek() == Some(&'}') => {
                it.next();
                if let Some(f) = fixed.last_mut() {
                    f.push('}');
                }
            }
            '{' => {
                let mut name = String::new();
                for d in it.by_ref() {
                    if d == '}' {
                        break;
                    }
                    name.push(d);
                }
                holes.push(name);
                fixed.push(String::new());
            }
            _ => {
                if let Some(f) = fixed.last_mut() {
                    f.push(c);
                }
            }
        }
    }
    (fixed, holes)
}

impl Pattern {
    fn new(key: &str) -> Option<Pattern> {
        if !key.contains('{') {
            return None;
        }
        let (fixed, holes) = split_format(key);
        if holes.is_empty() {
            return None;
        }
        // abbastanza testo fisso da non confondersi con altro
        let letters = fixed.iter().flat_map(|f| f.chars()).filter(|c| c.is_alphabetic()).count();
        if letters < 3 {
            return None;
        }
        let fixed_len = fixed.iter().map(|f| f.len()).sum();
        Some(Pattern { fixed, holes, fixed_len })
    }

    /// I pezzi variabili di `s`, se `s` è fatto con questo modello.
    fn captures(&self, s: &str) -> Option<Vec<String>> {
        let first = &self.fixed[0];
        let last = self.fixed.last()?;
        if !s.starts_with(first.as_str()) || !s.ends_with(last.as_str()) || s.len() < first.len() + last.len() {
            return None;
        }
        let body = &s[first.len()..s.len() - last.len()];
        let mut caps = Vec::with_capacity(self.holes.len());
        let mut rest = body;
        for mid in &self.fixed[1..self.fixed.len() - 1] {
            let i = if mid.is_empty() { 0 } else { rest.find(mid.as_str())? };
            caps.push(rest[..i].to_string());
            rest = &rest[i + mid.len()..];
        }
        caps.push(rest.to_string());
        Some(caps)
    }

    /// Traduzione di `s` col modello tradotto `to` (stessi segnaposto).
    #[cfg(test)]
    fn apply(&self, s: &str, to: &str) -> Option<String> {
        self.apply_with(s, to, &|_| None)
    }

    /// Come `apply`; `inner` traduce i pezzi variabili quando li conosce.
    fn apply_with(&self, s: &str, to: &str, inner: &dyn Fn(&str) -> Option<String>) -> Option<String> {
        let caps: Vec<String> = self.captures(s)?.into_iter().map(|c| inner(&c).unwrap_or(c)).collect();
        let (tfixed, tholes) = split_format(to);
        let mut used = vec![false; caps.len()];
        let mut out = String::with_capacity(s.len() + 16);
        for (k, piece) in tfixed.iter().enumerate() {
            out.push_str(piece);
            if let Some(name) = tholes.get(k) {
                // stesso nome (anche ripetuto) o, per i `{}`, nello stesso ordine
                let j = (0..caps.len())
                    .find(|&j| !used[j] && self.holes[j] == *name && !name.is_empty())
                    .or_else(|| (0..caps.len()).find(|&j| self.holes[j] == *name && !name.is_empty()))
                    .or_else(|| (0..caps.len()).find(|&j| !used[j]))?;
                used[j] = true;
                out.push_str(&caps[j]);
            }
        }
        Some(out)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn modelli() {
        let p = Pattern::new("Salvati {n} file in {}").unwrap();
        assert_eq!(p.apply("Salvati 3 file in /tmp/x", "Saved {n} files to {}").unwrap(), "Saved 3 files to /tmp/x");
        let p = Pattern::new("«{t}» sulla lavagna: puoi scriverci sopra").unwrap();
        assert_eq!(p.apply("«Note» sulla lavagna: puoi scriverci sopra", "“{t}” on the board").unwrap(), "“Note” on the board");
        assert!(p.apply("altro testo", "x").is_none());
        // ordine diverso nella traduzione: si va per nome
        let p = Pattern::new("la tabella `{}` (database `{}`, connessione «{}»)").unwrap();
        assert_eq!(
            p.apply("la tabella `elenco_localita` (database `legacy_mga`, connessione «crm-mga»)", "the table `{}` (database `{}`, connection «{}»)").unwrap(),
            "the table `elenco_localita` (database `legacy_mga`, connection «crm-mga»)"
        );
        let p = Pattern::new("{a} di {b} pagine").unwrap();
        assert_eq!(p.apply("2 di 5 pagine", "{b}ページ中{a}").unwrap(), "5ページ中2");
        // parentesi raddoppiate
        let (f, h) = split_format("a {{b}} {c}");
        assert_eq!((f, h), (vec!["a {b} ".to_string(), String::new()], vec!["c".to_string()]));
        assert!(Pattern::new("{} {}").is_none());
    }

    #[test]
    fn tf_traduce_anche_le_chiavi_esatte() {
        // a text built at runtime that is a key as it is («ogni ora» from `Schedule::describe`)
        set_lang(Lang::En);
        assert_eq!(tf("ogni ora".to_string()), "every hour");
        assert_eq!(tf("ogni 15 minuti".to_string()), "every 15 minutes");
    }

    #[test]
    fn tabelle_valide() {
        // ogni tabella si legge e i segnaposto coincidono con quelli della chiave
        for l in Lang::ALL {
            let Some(raw) = l.table() else { continue };
            let map: HashMap<String, String> = serde_json::from_str(raw).unwrap_or_else(|e| panic!("{}: {e}", l.code()));
            for (k, v) in &map {
                if v.is_empty() {
                    continue;
                }
                let (_, mut hk) = split_format(k);
                let (_, mut hv) = split_format(v);
                hk.sort();
                hv.sort();
                assert_eq!(hk, hv, "{}: segnaposto diversi in «{k}» → «{v}»", l.code());
            }
        }
    }

    #[test]
    fn stesse_chiavi() {
        // ogni lingua traduce tutti i testi dell'inglese (e niente di più)
        let keys = |l: Lang| -> std::collections::BTreeSet<String> {
            let map: HashMap<String, String> = serde_json::from_str(l.table().unwrap()).unwrap();
            map.into_keys().collect()
        };
        let en = keys(Lang::En);
        for l in Lang::ALL.iter().copied().filter(|l| l.table().is_some() && *l != Lang::En) {
            let k = keys(l);
            let manca: Vec<_> = en.difference(&k).take(5).collect();
            let in_piu: Vec<_> = k.difference(&en).take(5).collect();
            assert!(manca.is_empty() && in_piu.is_empty(), "{}: mancano {manca:?}, in più {in_piu:?}", l.code());
        }
    }

    #[test]
    fn testi_del_codice_tradotti() {
        // every `i18n::t("…")` of the code is in the English table (technical words aside)
        let same = ["SQL", "DISTINCT", "WHERE", "NULL", "OK", ":memory:", "⌘S", "a, b, c", "github.com/rmpaolillo/DbCraft", "https://example.com/dbcraft-tunnel.php", "~/.ssh/id_ed25519"];
        let en: HashMap<String, String> = serde_json::from_str(include_str!("i18n/en.json")).unwrap();
        let mut missing = Vec::new();
        let mut dirs = vec![std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("i18n.rs") {
                    let src = std::fs::read_to_string(&p).unwrap();
                    for part in src.split("i18n::t(\"").skip(1) {
                        let Some(end) = part.find("\")") else { continue };
                        let key = part[..end].replace("\\\"", "\"").replace("\\n", "\n");
                        if !en.contains_key(&key) && !same.contains(&key.as_str()) {
                            missing.push(format!("{}: {key}", p.display()));
                        }
                    }
                }
            }
        }
        assert!(missing.is_empty(), "senza traduzione: {missing:#?}");
    }

    #[test]
    fn codici() {
        assert_eq!(Lang::from_code("en-US"), Some(Lang::En));
        assert_eq!(Lang::from_code("zh_Hans_CN"), Some(Lang::Zh));
        assert_eq!(Lang::from_code("uk-UA"), Some(Lang::Uk));
        assert_eq!(Lang::from_code("pt-BR"), None);
        assert_eq!(resolve("ja"), Lang::Ja);
    }
}
