//! DbCraft's look for the basic controls, drawn through egui's widget themes ([`egui::theme::StyleProvider`]):
//! white fields with a clear border and a blue focus ring, checkboxes that turn solid blue when checked,
//! buttons with a border, and a blue «primary» button for the main action of a dialog or tab.
//! Everything else (layout, sizes) stays egui's own, so the screens don't change.

use egui::class::HasClasses as _;
use egui::theme::{DefaultStyle, StyleProvider};
use egui::widget_style::{ButtonStyle, CheckboxStyle, StyleArgs, TextEditStyle, WidgetState};
use egui::{Button, Color32, ComboBox, Frame, Margin, Stroke, TextEdit};

/// A checkbox that is checked (egui's style doesn't know the value, so [`checkbox`] tells it).
const CHECKED: &str = "dbcraft::checked";
/// The main action: «Salva», «OK», «Connetti»… (see [`primary`]).
const PRIMARY: &str = "dbcraft::primary";

/// The blue of focus rings, checked boxes and primary buttons.
pub fn accent(dark: bool) -> Color32 {
    if dark { Color32::from_rgb(59, 130, 246) } else { Color32::from_rgb(37, 99, 235) }
}

/// Installs the theme (for both light and dark: the colours follow `visuals.dark_mode`).
pub fn install(ctx: &egui::Context) {
    ctx.add_widget_theme::<ButtonStyle>(DbStyle);
    ctx.add_widget_theme::<TextEditStyle>(DbStyle);
    ctx.add_widget_theme::<CheckboxStyle>(DbStyle);
}

/// `ui.checkbox`, with the look of a checked box when it is checked.
pub fn checkbox<'a>(ui: &mut egui::Ui, checked: &'a mut bool, text: impl egui::IntoAtoms<'a>) -> egui::Response {
    ui.add(check(checked, text))
}

/// [`egui::Checkbox::new`] with that look (for `ui.add_enabled` and the like).
pub fn check<'a>(checked: &'a mut bool, text: impl egui::IntoAtoms<'a>) -> egui::Checkbox<'a> {
    let on = *checked;
    egui::Checkbox::new(checked, text).with_class_if(CHECKED, on)
}

/// A button for the main action, painted in the accent colour.
pub fn primary<'a>(text: impl egui::IntoAtoms<'a>) -> Button<'a> {
    Button::new(text).with_class(PRIMARY)
}

struct DbStyle;

/// Border, hovered border, fill of a field and of a button (inactive / hovered / pressed).
struct Palette {
    border: Color32,
    border_hover: Color32,
    field: Color32,
    button: [Color32; 3],
}

fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            border: Color32::from_gray(100),
            border_hover: Color32::from_gray(140),
            field: Color32::from_gray(24),
            button: [Color32::from_gray(52), Color32::from_gray(64), Color32::from_gray(78)],
        }
    } else {
        Palette {
            border: Color32::from_gray(160),
            border_hover: Color32::from_gray(115),
            field: Color32::WHITE,
            button: [Color32::from_gray(251), Color32::from_gray(240), Color32::from_gray(226)],
        }
    }
}

impl StyleProvider<ButtonStyle> for DbStyle {
    fn style(&mut self, args: &StyleArgs<'_>) -> ButtonStyle {
        let mut s: ButtonStyle = DefaultStyle.style(args);
        let dark = args.style.visuals.dark_mode;
        let frame = &mut s.atom_layout.frame;
        // a button without a visible frame (toolbar icons, menu entries, selected tabs) keeps egui's look
        // (and so do selectable labels: frameless until hovered)
        let framed = frame.fill != Color32::TRANSPARENT || (frame.stroke.width > 0.0 && frame.stroke.color != Color32::TRANSPARENT);
        if !framed
            || args.classes.has_class(Button::CLASS_SELECTED.as_str())
            || args.classes.has_class(Button::CLASS_HIDE_FRAME_WHEN_INACTIVE.as_str())
            || args.classes.has_class(ComboBox::CLASS_OPEN.as_str())
        {
            return s;
        }
        let p = palette(dark);
        if args.classes.has_class(PRIMARY) {
            let a = accent(dark);
            frame.fill = match args.state {
                WidgetState::Hovered => a.gamma_multiply_u8(230),
                WidgetState::Active => a.gamma_multiply_u8(200),
                _ => a,
            };
            frame.stroke.color = a;
            s.atom_layout.text_style.color = Color32::WHITE;
            s.atom_layout.image_tint = Color32::WHITE;
            return s;
        }
        let (fill, border) = match args.state {
            WidgetState::Hovered => (p.button[1], p.border_hover),
            WidgetState::Active => (p.button[2], p.border_hover),
            _ => (p.button[0], p.border),
        };
        frame.fill = fill;
        frame.stroke.color = border;
        s
    }
}

impl StyleProvider<TextEditStyle> for DbStyle {
    fn style(&mut self, args: &StyleArgs<'_>) -> TextEditStyle {
        let mut s: TextEditStyle = DefaultStyle.style(args);
        if args.classes.has_class(TextEdit::CLASS_READ_ONLY.as_str()) {
            return s;
        }
        let dark = args.style.visuals.dark_mode;
        let p = palette(dark);
        let visuals = args.style.visuals.widgets.state(args.state);
        // the focus ring is 2 px; the frame keeps the same outer size whatever the stroke
        let stroke = match args.state {
            WidgetState::Active => Stroke::new(2.0, accent(dark)),
            WidgetState::Hovered => Stroke::new(1.0, p.border_hover),
            _ => Stroke::new(1.0, p.border),
        };
        let margin: Margin = args.style.spacing.button_padding.into();
        s.atom_layout.frame = Frame { fill: p.field, corner_radius: visuals.corner_radius, inner_margin: margin, ..Default::default() }
            .apply_stroke_and_expansion_without_layout_shift(stroke, 0.0);
        s
    }
}

impl StyleProvider<CheckboxStyle> for DbStyle {
    fn style(&mut self, args: &StyleArgs<'_>) -> CheckboxStyle {
        let mut s: CheckboxStyle = DefaultStyle.style(args);
        let dark = args.style.visuals.dark_mode;
        let p = palette(dark);
        let hovered = matches!(args.state, WidgetState::Hovered | WidgetState::Active);
        let f = &mut s.checkbox_frame;
        if args.classes.has_class(CHECKED) {
            let a = accent(dark);
            f.fill = if hovered { a.gamma_multiply_u8(225) } else { a };
            f.stroke = Stroke::new(1.0, a);
            s.check_stroke = Stroke::new(2.0, Color32::WHITE);
        } else {
            f.fill = p.field;
            f.stroke = Stroke::new(1.0, if hovered { accent(dark) } else { p.border });
        }
        f.corner_radius = egui::CornerRadius::same(3);
        s
    }
}
