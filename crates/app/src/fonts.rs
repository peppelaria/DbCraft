//! Fonts: egui's own, plus what the interface languages need. Chinese, Japanese and Korean take a
//! font of the system (as in OpenSlate); the language names in the menu (中文 日本語 한국어) come
//! from a tiny subset of Noto Sans CJK (SIL OFL 1.1, `assets/fonts/OFL-NotoSansCJK.txt`), so they
//! show even without one.

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

use crate::i18n::{self, Lang};

const F_CJK: &str = "cjk";
const F_LANGNAMES: &str = "cjk-langnames";

/// Sets the fonts for the current interface language.
pub fn apply(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(F_LANGNAMES.into(), Arc::new(FontData::from_static(include_bytes!("../assets/fonts/NotoSansCJK-langnames.otf"))));
    let lang = i18n::lang();
    let cjk = if lang.cjk() { cjk_font(lang) } else { None };
    if lang.cjk() && cjk.is_none() {
        eprintln!("nessun carattere {} trovato nel sistema", lang.code());
    }
    if let Some((bytes, index)) = cjk {
        let mut data = FontData::from_owned(bytes);
        data.index = index;
        fonts.font_data.insert(F_CJK.into(), Arc::new(data));
    }
    for fam in [FontFamily::Proportional, FontFamily::Monospace] {
        let list = fonts.families.entry(fam).or_default();
        if fonts.font_data.contains_key(F_CJK) {
            list.push(F_CJK.into());
        }
        list.push(F_LANGNAMES.into());
    }
    ctx.set_fonts(fonts);
}

/// A system font for Chinese, Japanese or Korean (bytes and index in the .ttc collection).
fn cjk_font(lang: Lang) -> Option<(Vec<u8>, u32)> {
    // Noto Sans CJK (Linux): in the collection 0 = Japanese, 1 = Korean, 2 = Simplified Chinese
    let noto = match lang {
        Lang::Ja => 0,
        Lang::Ko => 1,
        _ => 2,
    };
    let mut cands: Vec<(String, u32)> = Vec::new();
    let mut add = |p: &str, i: u32| cands.push((p.to_string(), i));
    if cfg!(target_os = "macos") {
        match lang {
            Lang::Ja => {
                add("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", 0);
                add("/System/Library/Fonts/Hiragino Sans GB.ttc", 0);
            }
            Lang::Ko => add("/System/Library/Fonts/AppleSDGothicNeo.ttc", 0),
            _ => {
                add("/System/Library/Fonts/PingFang.ttc", 0);
                add("/System/Library/Fonts/Hiragino Sans GB.ttc", 0);
                add("/System/Library/Fonts/STHeiti Light.ttc", 0);
            }
        }
        add("/System/Library/Fonts/Supplemental/Arial Unicode.ttf", 0);
        add("/Library/Fonts/Arial Unicode.ttf", 0);
    } else if cfg!(target_os = "windows") {
        match lang {
            Lang::Ja => {
                add("C:\\Windows\\Fonts\\YuGothM.ttc", 0);
                add("C:\\Windows\\Fonts\\meiryo.ttc", 0);
                add("C:\\Windows\\Fonts\\msgothic.ttc", 0);
            }
            Lang::Ko => add("C:\\Windows\\Fonts\\malgun.ttf", 0),
            _ => {
                add("C:\\Windows\\Fonts\\msyh.ttc", 0);
                add("C:\\Windows\\Fonts\\simsun.ttc", 0);
            }
        }
    } else {
        for dir in ["/usr/share/fonts/opentype/noto", "/usr/share/fonts/noto-cjk", "/usr/share/fonts/google-noto-cjk", "/usr/share/fonts/opentype/noto-cjk"] {
            add(&format!("{dir}/NotoSansCJK-Regular.ttc"), noto);
        }
        add("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0);
        add("/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf", 0);
    }
    cands.into_iter().find_map(|(p, i)| std::fs::read(p).ok().map(|b| (b, i)))
}
