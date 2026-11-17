//! Date / time picker for date-like columns: a month calendar (Monday first, Italian labels),
//! hour / minute / second fields and Today / Now shortcuts. Values are read and written in the
//! ISO form databases accept (`YYYY-MM-DD`, `HH:MM:SS`, `YYYY-MM-DD HH:MM:SS`); a fractional part
//! or a time zone suffix of the original value is kept as it was.

use chrono::{Datelike, Local, NaiveDate, NaiveTime, Timelike};
use egui::{Align2, Color32, Id, Key, Rect, RichText, Sense, Stroke, vec2};

use crate::model::ColumnKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateKind {
    Date,
    Time,
    DateTime,
}

impl DateKind {
    pub fn of(kind: ColumnKind) -> Option<DateKind> {
        match kind {
            ColumnKind::Date => Some(DateKind::Date),
            ColumnKind::Time => Some(DateKind::Time),
            ColumnKind::DateTime => Some(DateKind::DateTime),
            _ => None,
        }
    }
    fn has_date(self) -> bool {
        self != DateKind::Time
    }
    fn has_time(self) -> bool {
        self != DateKind::Date
    }
}

/// A date and/or time read from text, with whatever followed the seconds (fraction, zone).
#[derive(Clone, Debug, PartialEq)]
pub struct Parsed {
    pub date: Option<NaiveDate>,
    pub time: Option<NaiveTime>,
    pub rest: String,
}

fn num(s: &str) -> Option<u32> {
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten()
}

/// `HH:MM[:SS]` at the start of `s`; returns the time and the remaining text.
fn parse_time(s: &str) -> Option<(NaiveTime, &str)> {
    let h = num(s.get(0..2)?)?;
    if s.get(2..3)? != ":" {
        return None;
    }
    let m = num(s.get(3..5)?)?;
    let (sec, rest) = match (s.get(5..6), s.get(6..8)) {
        (Some(":"), Some(x)) if num(x).is_some() => (num(x)?, s.get(8..).unwrap_or("")),
        _ => (0, s.get(5..).unwrap_or("")),
    };
    Some((NaiveTime::from_hms_opt(h, m, sec.min(59))?, rest))
}

pub fn parse(s: &str) -> Parsed {
    let t = s.trim();
    let mut out = Parsed { date: None, time: None, rest: String::new() };
    let date = t.get(0..10).filter(|d| d.as_bytes().get(4) == Some(&b'-') && d.as_bytes().get(7) == Some(&b'-')).and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    let after = match date {
        Some(d) => {
            out.date = Some(d);
            t.get(10..).unwrap_or("").trim_start_matches(['T', ' '])
        }
        None => t,
    };
    match parse_time(after) {
        Some((time, rest)) => {
            out.time = Some(time);
            out.rest = rest.to_string();
        }
        None if out.date.is_some() => out.rest = String::new(),
        None => {}
    }
    out
}

pub fn format(kind: DateKind, date: NaiveDate, time: NaiveTime, rest: &str) -> String {
    let d = date.format("%Y-%m-%d").to_string();
    let t = format!("{:02}:{:02}:{:02}{rest}", time.hour(), time.minute(), time.second());
    match kind {
        DateKind::Date => d,
        DateKind::Time => t,
        DateKind::DateTime => format!("{d} {t}"),
    }
}

const MONTHS: [&str; 12] = ["Gennaio", "Febbraio", "Marzo", "Aprile", "Maggio", "Giugno", "Luglio", "Agosto", "Settembre", "Ottobre", "Novembre", "Dicembre"];
const WEEKDAYS: [&str; 7] = ["Lu", "Ma", "Me", "Gi", "Ve", "Sa", "Do"];

#[derive(Clone, Debug)]
pub struct PickerState {
    pub kind: DateKind,
    pub date: NaiveDate,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub rest: String,
    view_year: i32,
    view_month: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PickerOutcome {
    Pick(String),
    Null,
    Cancel,
}

impl PickerState {
    /// Starts from the cell's text, or from now when it is empty or unreadable.
    pub fn from_text(text: &str, kind: DateKind) -> Self {
        let now = Local::now().naive_local();
        let p = parse(text);
        let date = p.date.unwrap_or(now.date());
        let time = p.time.unwrap_or(if p.date.is_some() { NaiveTime::MIN } else { now.time() });
        PickerState { kind, date, hour: time.hour(), minute: time.minute(), second: time.second(), rest: p.rest, view_year: date.year(), view_month: date.month() }
    }

    pub fn text(&self) -> String {
        let time = NaiveTime::from_hms_opt(self.hour.min(23), self.minute.min(59), self.second.min(59)).unwrap_or(NaiveTime::MIN);
        format(self.kind, self.date, time, &self.rest)
    }

    fn shift_month(&mut self, delta: i32) {
        let m = self.view_year * 12 + self.view_month as i32 - 1 + delta;
        self.view_year = m.div_euclid(12);
        self.view_month = (m.rem_euclid(12) + 1) as u32;
    }

    /// Calendar, time fields and buttons. Returns the outcome once the user confirms or cancels.
    pub fn ui(&mut self, ui: &mut egui::Ui, allow_null: bool) -> Option<PickerOutcome> {
        let mut out = None;
        let accent = Color32::from_rgb(0x1F, 0x6F, 0xD0);
        if self.kind.has_date() {
            ui.horizontal(|ui| {
                if ui.small_button("◀").on_hover_text("Mese precedente").clicked() {
                    self.shift_month(-1);
                }
                let month = MONTHS.get(self.view_month.saturating_sub(1) as usize).copied().unwrap_or("");
                egui::ComboBox::from_id_salt("dp-month").width(96.0).selected_text(month).show_ui(ui, |ui| {
                    for (i, m) in MONTHS.iter().enumerate() {
                        ui.selectable_value(&mut self.view_month, i as u32 + 1, *m);
                    }
                });
                ui.add(egui::DragValue::new(&mut self.view_year).range(1..=9999).speed(0.2));
                if ui.small_button("▶").on_hover_text("Mese successivo").clicked() {
                    self.shift_month(1);
                }
            });
            let first = NaiveDate::from_ymd_opt(self.view_year, self.view_month, 1).unwrap_or(self.date);
            let offset = first.weekday().num_days_from_monday() as i64;
            let start = first - chrono::Duration::days(offset);
            let today = Local::now().date_naive();
            let cell = vec2(30.0, 24.0);
            egui::Grid::new("dp-days").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
                for w in WEEKDAYS {
                    ui.add_sized(cell, egui::Label::new(RichText::new(w).small().weak()));
                }
                ui.end_row();
                for week in 0..6 {
                    for wd in 0..7 {
                        let day = start + chrono::Duration::days(week * 7 + wd);
                        let (rect, resp) = ui.allocate_exact_size(cell, Sense::click());
                        let selected = day == self.date;
                        let in_month = day.month() == self.view_month;
                        if selected {
                            ui.painter().rect_filled(rect, 4.0, accent);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
                        }
                        if day == today && !selected {
                            ui.painter().rect_stroke(rect.shrink(1.0), 4.0, Stroke::new(1.0, accent), egui::StrokeKind::Inside);
                        }
                        let color = if selected {
                            Color32::WHITE
                        } else if !in_month {
                            ui.visuals().weak_text_color().gamma_multiply(0.6)
                        } else if wd >= 5 {
                            Color32::from_rgb(0xC4, 0x2B, 0x1C)
                        } else {
                            ui.visuals().text_color()
                        };
                        ui.painter().text(rect.center(), Align2::CENTER_CENTER, day.day().to_string(), egui::FontId::proportional(13.0), color);
                        if resp.clicked() {
                            self.date = day;
                            if !in_month {
                                self.view_year = day.year();
                                self.view_month = day.month();
                            }
                        }
                        if resp.double_clicked() {
                            self.date = day;
                            out = Some(PickerOutcome::Pick(self.text()));
                        }
                    }
                    ui.end_row();
                }
            });
        }
        if self.kind.has_time() {
            ui.horizontal(|ui| {
                ui.label("Ora");
                ui.add(egui::DragValue::new(&mut self.hour).range(0..=23).speed(0.1).custom_formatter(|v, _| format!("{v:02}")));
                ui.label(":");
                ui.add(egui::DragValue::new(&mut self.minute).range(0..=59).speed(0.2).custom_formatter(|v, _| format!("{v:02}")));
                ui.label(":");
                ui.add(egui::DragValue::new(&mut self.second).range(0..=59).speed(0.2).custom_formatter(|v, _| format!("{v:02}")));
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            let label = match self.kind {
                DateKind::Date => "Oggi",
                _ => "Adesso",
            };
            if ui.button(label).clicked() {
                let now = Local::now().naive_local();
                self.date = now.date();
                self.view_year = self.date.year();
                self.view_month = self.date.month();
                self.hour = now.hour();
                self.minute = now.minute();
                self.second = now.second();
            }
            if allow_null && ui.button("NULL").clicked() {
                out = Some(PickerOutcome::Null);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("OK").color(Color32::WHITE).strong()).fill(accent)).clicked() {
                    out = Some(PickerOutcome::Pick(self.text()));
                }
                if ui.button("Annulla").clicked() {
                    out = Some(PickerOutcome::Cancel);
                }
            });
        });
        ui.label(RichText::new(self.text()).monospace().small().weak());
        out
    }
}

/// A picker in a floating popup under `anchor`; its state lives in egui memory under `id`.
/// Returns the outcome once (then the popup closes). Escape or a click outside cancels.
pub fn popup(ctx: &egui::Context, id: Id, anchor: Rect, kind: DateKind, current: &str, allow_null: bool) -> Option<PickerOutcome> {
    let mut st = ctx.data_mut(|d| d.get_temp::<PickerState>(id)).filter(|s| s.kind == kind).unwrap_or_else(|| PickerState::from_text(current, kind));
    let area = egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(anchor.left_bottom() + vec2(0.0, 2.0)).constrain(true).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_max_width(250.0);
            st.ui(ui, allow_null)
        })
        .inner
    });
    let mut out = area.inner;
    let (esc, enter, clicked_outside) = ctx.input(|i| {
        let outside = i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !anchor.contains(p));
        (i.key_pressed(Key::Escape), i.key_pressed(Key::Enter), outside)
    });
    if out.is_none() && enter {
        out = Some(PickerOutcome::Pick(st.text()));
    }
    if out.is_none() && (esc || clicked_outside) {
        out = Some(PickerOutcome::Cancel);
    }
    if out.is_some() {
        ctx.data_mut(|d| d.remove::<PickerState>(id));
    } else {
        ctx.data_mut(|d| d.insert_temp(id, st));
    }
    out
}

/// Small calendar glyph (for the button on date cells).
pub fn paint_calendar_glyph(p: &egui::Painter, rect: Rect, color: Color32) {
    let r = rect.shrink(rect.width() * 0.18);
    p.rect_stroke(r, 1.5, Stroke::new(1.0, color), egui::StrokeKind::Inside);
    p.rect_filled(Rect::from_min_max(r.min, egui::pos2(r.right(), r.top() + r.height() * 0.3)), 1.5, color);
    for k in 0..3 {
        let x = r.left() + r.width() * (0.25 + 0.25 * k as f32);
        p.circle_filled(egui::pos2(x, r.top() + r.height() * 0.65), 0.9, color);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }
    fn t(h: u32, m: u32, s: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, s).unwrap()
    }

    #[test]
    fn parses_database_forms() {
        assert_eq!(parse("2026-11-06 20:34:09"), Parsed { date: Some(d(2026, 11, 6)), time: Some(t(20, 34, 9)), rest: String::new() });
        assert_eq!(parse("2020-01-01T08:00:00.123+02:00").rest, ".123+02:00");
        assert_eq!(parse("2020-01-01 08:00:00+00").rest, "+00");
        assert_eq!(parse("2020-01-01"), Parsed { date: Some(d(2020, 1, 1)), time: None, rest: String::new() });
        assert_eq!(parse("10:00"), Parsed { date: None, time: Some(t(10, 0, 0)), rest: String::new() });
        assert_eq!(parse("10:00:05.25").rest, ".25");
        assert_eq!(parse("not a date"), Parsed { date: None, time: None, rest: String::new() });
        assert_eq!(parse("2020-02-30").date, None);
    }

    #[test]
    fn formats_per_kind_and_keeps_suffix() {
        let (dd, tt) = (d(2026, 3, 9), t(7, 5, 0));
        assert_eq!(format(DateKind::Date, dd, tt, ""), "2026-03-09");
        assert_eq!(format(DateKind::Time, dd, tt, ""), "07:05:00");
        assert_eq!(format(DateKind::DateTime, dd, tt, "+00"), "2026-03-09 07:05:00+00");
    }

    #[test]
    fn state_round_trip_and_month_navigation() {
        let mut st = PickerState::from_text("2026-12-31 23:59:58.5", DateKind::DateTime);
        assert_eq!(st.text(), "2026-12-31 23:59:58.5");
        st.shift_month(1);
        assert_eq!((st.view_year, st.view_month), (2027, 1));
        st.shift_month(-13);
        assert_eq!((st.view_year, st.view_month), (2025, 12));
        let st = PickerState::from_text("2026-05-04", DateKind::DateTime);
        assert_eq!(st.text(), "2026-05-04 00:00:00");
    }
}
