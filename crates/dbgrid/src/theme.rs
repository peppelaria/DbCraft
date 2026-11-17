//! Grid colour tokens. The palette layout follows GridCraft `theme::Tokens` (MIT OR Apache-2.0),
//! reduced to what the data grid paints, with a blue accent.

use egui::Color32;

#[derive(Clone, Debug)]
pub struct GridTheme {
    pub dark: bool,
    pub grid_bg: Color32,
    pub alt_row: Color32,
    pub gridline: Color32,
    pub header_bg: Color32,
    pub header_text: Color32,
    pub header_type_text: Color32,
    pub header_line: Color32,
    pub header_sel_bg: Color32,
    pub header_sel_text: Color32,
    pub sel_fill: Color32,
    pub active_row: Color32,
    pub sel_border: Color32,
    pub cell_text: Color32,
    pub null_text: Color32,
    pub dirty_fill: Color32,
    pub inserted_fill: Color32,
    pub deleted_fill: Color32,
    pub pk: Color32,
    pub scroll_track: Color32,
    pub scroll_thumb: Color32,
}

const fn pick(dark: bool, light: Color32, d: Color32) -> Color32 {
    if dark { d } else { light }
}

impl GridTheme {
    pub fn get(ctx: &egui::Context) -> GridTheme {
        GridTheme::palette(ctx.global_style().visuals.dark_mode)
    }

    pub fn palette(dark: bool) -> GridTheme {
        GridTheme {
            dark,
            grid_bg: pick(dark, Color32::WHITE, Color32::from_rgb(0x1E, 0x1E, 0x1E)),
            alt_row: pick(dark, Color32::from_rgb(0xF7, 0xF9, 0xFC), Color32::from_rgb(0x24, 0x24, 0x26)),
            gridline: pick(dark, Color32::from_rgb(0xE1, 0xE1, 0xE1), Color32::from_rgb(0x38, 0x38, 0x38)),
            header_bg: pick(dark, Color32::from_rgb(0xF8, 0xF8, 0xF8), Color32::from_rgb(0x30, 0x30, 0x30)),
            header_text: pick(dark, Color32::from_rgb(0x24, 0x24, 0x24), Color32::from_rgb(0xDD, 0xDD, 0xDD)),
            header_type_text: pick(dark, Color32::from_rgb(0x80, 0x80, 0x80), Color32::from_rgb(0x90, 0x90, 0x90)),
            header_line: pick(dark, Color32::from_rgb(0xD6, 0xD6, 0xD6), Color32::from_rgb(0x45, 0x45, 0x45)),
            header_sel_bg: pick(dark, Color32::from_rgb(0xDC, 0xE8, 0xF8), Color32::from_rgb(0x2D, 0x3E, 0x55)),
            header_sel_text: pick(dark, Color32::from_rgb(0x0B, 0x4A, 0x9E), Color32::from_rgb(0x8F, 0xBE, 0xF5)),
            sel_fill: Color32::from_rgba_unmultiplied(0x1F, 0x6F, 0xD0, 0x26),
            active_row: pick(dark, Color32::from_rgba_unmultiplied(0x1F, 0x6F, 0xD0, 0x10), Color32::from_rgba_unmultiplied(0x8F, 0xBE, 0xF5, 0x18)),
            sel_border: Color32::from_rgb(0x1F, 0x6F, 0xD0),
            cell_text: pick(dark, Color32::BLACK, Color32::from_rgb(0xE6, 0xE6, 0xE6)),
            null_text: pick(dark, Color32::from_rgb(0xA0, 0xA0, 0xA0), Color32::from_rgb(0x70, 0x70, 0x70)),
            dirty_fill: pick(dark, Color32::from_rgb(0xFF, 0xF4, 0xCE), Color32::from_rgb(0x4A, 0x3F, 0x1E)),
            inserted_fill: pick(dark, Color32::from_rgb(0xE3, 0xF6, 0xE8), Color32::from_rgb(0x1E, 0x3D, 0x28)),
            deleted_fill: pick(dark, Color32::from_rgb(0xFB, 0xE3, 0xE1), Color32::from_rgb(0x4A, 0x22, 0x22)),
            pk: Color32::from_rgb(0xD9, 0xA4, 0x06),
            scroll_track: pick(dark, Color32::from_rgb(0xF0, 0xF0, 0xF0), Color32::from_rgb(0x2A, 0x2A, 0x2A)),
            scroll_thumb: pick(dark, Color32::from_rgb(0xC2, 0xC2, 0xC2), Color32::from_rgb(0x5A, 0x5A, 0x5A)),
        }
    }
}
