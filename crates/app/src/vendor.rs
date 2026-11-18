//! KaTeX 0.16.11, marked 12.0.2 (MIT) and DOMPurify 3.4.16 (Apache-2.0 / MPL-2.0) inside the
//! program, as in OpenSlate (see web/vendor/LICENSE-*): the chat renders Markdown and formulas
//! without internet. Inter (SIL OFL 1.1, web/vendor/inter/OFL-Inter.txt) is the pages' font.

/// (path after /vendor/, type, content)
pub static FILES: &[(&str, &str, &[u8])] = &[
    ("katex.min.js", "application/javascript", include_bytes!("../web/vendor/katex.min.js")),
    ("katex.min.css", "text/css", include_bytes!("../web/vendor/katex.min.css")),
    ("marked.min.js", "application/javascript", include_bytes!("../web/vendor/marked.min.js")),
    ("purify.min.js", "application/javascript", include_bytes!("../web/vendor/purify.min.js")),
    ("inter/Inter-Regular.ttf", "font/ttf", include_bytes!("../web/vendor/inter/Inter-Regular.ttf")),
    ("inter/Inter-Medium.ttf", "font/ttf", include_bytes!("../web/vendor/inter/Inter-Medium.ttf")),
    ("inter/Inter-SemiBold.ttf", "font/ttf", include_bytes!("../web/vendor/inter/Inter-SemiBold.ttf")),
    ("fonts/KaTeX_AMS-Regular.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_AMS-Regular.woff2")),
    ("fonts/KaTeX_Caligraphic-Bold.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Caligraphic-Bold.woff2")),
    ("fonts/KaTeX_Caligraphic-Regular.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Caligraphic-Regular.woff2")),
    ("fonts/KaTeX_Fraktur-Bold.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Fraktur-Bold.woff2")),
    ("fonts/KaTeX_Fraktur-Regular.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Fraktur-Regular.woff2")),
    ("fonts/KaTeX_Main-Bold.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Main-Bold.woff2")),
    ("fonts/KaTeX_Main-BoldItalic.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Main-BoldItalic.woff2")),
    ("fonts/KaTeX_Main-Italic.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Main-Italic.woff2")),
    ("fonts/KaTeX_Main-Regular.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Main-Regular.woff2")),
    ("fonts/KaTeX_Math-BoldItalic.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Math-BoldItalic.woff2")),
    ("fonts/KaTeX_Math-Italic.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Math-Italic.woff2")),
    ("fonts/KaTeX_SansSerif-Bold.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_SansSerif-Bold.woff2")),
    ("fonts/KaTeX_SansSerif-Italic.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_SansSerif-Italic.woff2")),
    ("fonts/KaTeX_SansSerif-Regular.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_SansSerif-Regular.woff2")),
    ("fonts/KaTeX_Script-Regular.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Script-Regular.woff2")),
    ("fonts/KaTeX_Size1-Regular.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Size1-Regular.woff2")),
    ("fonts/KaTeX_Size2-Regular.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Size2-Regular.woff2")),
    ("fonts/KaTeX_Size3-Regular.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Size3-Regular.woff2")),
    ("fonts/KaTeX_Size4-Regular.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Size4-Regular.woff2")),
    ("fonts/KaTeX_Typewriter-Regular.woff2", "font/woff2", include_bytes!("../web/vendor/fonts/KaTeX_Typewriter-Regular.woff2")),
];

pub fn get(path: &str) -> Option<(&'static str, &'static [u8])> {
    FILES.iter().find(|(p, _, _)| *p == path).map(|(_, t, b)| (*t, *b))
}
