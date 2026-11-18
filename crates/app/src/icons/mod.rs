//! DbCraft's icons: the GridCraft icon set (code-drawn, see `gridcraft.rs`) plus database icons
//! drawn in the same style (`db.rs`), and the small widgets that show them.

mod db;
mod gridcraft;

use egui::{Color32, Rect, Response, RichText, Sense, Ui, Vec2, vec2};

pub use db::Db;
pub use gridcraft::Icon as G;

/// The colours the icons draw with, from GridCraft's palette (`theme.rs`, MIT OR Apache-2.0).
pub(crate) mod palette {
    use egui::Color32;

    const fn rgb(v: u32) -> Color32 {
        Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Cat {
        Blue,
        Green,
        Orange,
        Red,
        Purple,
        Teal,
        Amber,
        Slate,
    }

    pub struct Tokens {
        pub paper: Color32,
        pub cat: [Color32; 8],
        pub recording: Color32,
    }

    impl Tokens {
        pub fn light() -> Tokens {
            Tokens::of(false)
        }
        pub fn get(ctx: &egui::Context) -> Tokens {
            Tokens::of(ctx.global_style().visuals.dark_mode)
        }
        fn of(dark: bool) -> Tokens {
            if dark {
                Tokens {
                    paper: rgb(0x454545),
                    cat: [rgb(0x6EA8F7), rgb(0x5CC587), rgb(0xF0965A), rgb(0xF07B6E), rgb(0xB394EE), rgb(0x4CC7CF), rgb(0xF2C14E), rgb(0xA9B6C4)],
                    recording: rgb(0xF1705F),
                }
            } else {
                Tokens {
                    paper: Color32::WHITE,
                    cat: [rgb(0x2F6FCF), rgb(0x1E8A4C), rgb(0xC8601A), rgb(0xC8372D), rgb(0x8350C8), rgb(0x13808A), rgb(0x9C6C00), rgb(0x5A6B7D)],
                    recording: rgb(0xC42B1C),
                }
            }
        }
    }
}

/// Any icon DbCraft shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ico {
    G(G),
    Db(Db),
}

impl From<G> for Ico {
    fn from(i: G) -> Self {
        Ico::G(i)
    }
}

impl From<Db> for Ico {
    fn from(i: Db) -> Self {
        Ico::Db(i)
    }
}

/// Draws `icon` into `rect` in the text colour `c` (coloured parts use the palette).
pub fn paint(p: &egui::Painter, rect: Rect, icon: impl Into<Ico>, c: Color32) {
    match icon.into() {
        Ico::G(i) => gridcraft::paint(p, rect, i, c),
        Ico::Db(i) => db::paint(p, rect, i, c),
    }
}

/// An icon on its own, sized like a line of text.
pub fn show(ui: &mut Ui, icon: impl Into<Ico>, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), rect, icon, ui.visuals().text_color());
    }
    resp
}

/// A toolbar button: icon then label, like a text button.
pub fn button(ui: &mut Ui, icon: impl Into<Ico>, label: &str) -> Response {
    button_ex(ui, icon, label, false, true)
}

/// A toggle (selected) or disabled button with an icon.
pub fn button_ex(ui: &mut Ui, icon: impl Into<Ico>, label: &str, selected: bool, enabled: bool) -> Response {
    let icon = icon.into();
    let label = crate::i18n::tr(label);
    let label = label.as_ref();
    let text = if label.is_empty() { RichText::new("") } else { RichText::new(label) };
    let galley = egui::WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Button);
    let pad = ui.spacing().button_padding;
    let icon_w = 16.0;
    let gap = if label.is_empty() { 0.0 } else { 5.0 };
    let size = vec2(pad.x * 2.0 + icon_w + gap + galley.size().x, (galley.size().y.max(icon_w) + pad.y * 2.0).max(ui.spacing().interact_size.y));
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let v = if enabled { ui.style().interact_selectable(&resp, selected) } else { ui.visuals().widgets.noninteractive };
        let fill = if selected { ui.visuals().selection.bg_fill } else if resp.hovered() || resp.is_pointer_button_down_on() { v.weak_bg_fill } else { ui.visuals().widgets.inactive.weak_bg_fill };
        ui.painter().rect(rect, v.corner_radius, fill, v.bg_stroke, egui::StrokeKind::Inside);
        let mut color = if selected { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
        if !enabled {
            color = color.gamma_multiply(0.4);
        }
        let ir = Rect::from_center_size(egui::pos2(rect.left() + pad.x + icon_w / 2.0, rect.center().y), Vec2::splat(icon_w));
        if enabled {
            paint(ui.painter(), ir, icon, color);
        } else {
            // Disabled: draw dimmed.
            let mut p = ui.painter().clone();
            p.set_opacity(0.4);
            paint(&p, ir, icon, color);
        }
        ui.painter().galley(egui::pos2(ir.right() + gap, rect.center().y - galley.size().y / 2.0), galley, color);
    }
    resp
}

/// Small square icon button (no label) with a tooltip.
pub fn small(ui: &mut Ui, icon: impl Into<Ico>, tip: &str, enabled: bool) -> Response {
    let r = button_ex(ui, icon, "", false, enabled);
    if tip.is_empty() { r } else { r.on_hover_text(crate::i18n::tr(tip).into_owned()) }
}

/// A large toolbar tile: 24-pt icon over a caption.
pub fn tile(ui: &mut Ui, icon: impl Into<Ico>, label: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(76.0, 52.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.style().interact(&resp);
        if resp.hovered() || resp.is_pointer_button_down_on() {
            ui.painter().rect_filled(rect, 4.0, v.weak_bg_fill);
        }
        let color = ui.visuals().text_color();
        paint(ui.painter(), Rect::from_center_size(rect.center_top() + vec2(0.0, 18.0), Vec2::splat(26.0)), icon, color);
        ui.painter().text(rect.center_bottom() - vec2(0.0, 9.0), egui::Align2::CENTER_CENTER, crate::i18n::tr(label), egui::FontId::proportional(11.0), color);
    }
    resp
}

/// A menu entry with an icon.
pub fn menu_item(ui: &mut Ui, icon: impl Into<Ico>, label: &str) -> Response {
    let icon = icon.into();
    let resp = ui.add(egui::Button::new(RichText::new(format!("      {}", crate::i18n::tr(label)))).frame(false));
    let r = Rect::from_center_size(egui::pos2(resp.rect.left() + 12.0, resp.rect.center().y), Vec2::splat(14.0));
    paint(ui.painter(), r, icon, ui.visuals().text_color());
    resp
}

/// Debug view of every icon (`--icon-gallery`), light and dark.
pub fn gallery(ui: &mut Ui) {
    let g_icons = [
        G::Table, G::Filter, G::SortAsc, G::SortDesc, G::Search, G::Find, G::Save, G::Undo, G::Redo, G::Insert, G::Delete, G::Plus, G::Minus,
        G::Close, G::Check, G::Folder, G::OpenFolder, G::NewFile, G::Play, G::StopSquare, G::Settings, G::Link, G::Function, G::Script, G::Lock,
        G::Calendar, G::Clock, G::Info, G::Print, G::Expand, G::Collapse, G::Copy, G::Paste, G::Pen, G::Share, G::Book, G::Chart, G::ChartBar,
    ];
    let all: Vec<(Ico, String)> = Db::ALL.iter().map(|i| (Ico::Db(*i), format!("{i:?}"))).chain(g_icons.iter().map(|i| (Ico::G(*i), format!("G::{i:?}")))).collect();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("Icone DbCraft (Db::*) e GridCraft (G::*)");
        egui::Grid::new("icons").num_columns(10).spacing([10.0, 10.0]).show(ui, |ui| {
            for (k, (i, name)) in all.iter().enumerate() {
                ui.vertical(|ui| {
                    ui.set_width(80.0);
                    show(ui, *i, 30.0);
                    ui.label(RichText::new(name).small());
                });
                if k % 10 == 9 {
                    ui.end_row();
                }
            }
        });
    });
}
