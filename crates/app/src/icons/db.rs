//! Database icons drawn for DbCraft in the style of the GridCraft set (16×16 grid, 1.15-pt
//! strokes, category palette, paper fills): connection, database, schema, objects, keys, users,
//! backup, automation, model, BI and the editing actions a database client needs.

use egui::epaint::{PathShape, PathStroke};
use egui::{Color32, Painter, Pos2, Rect, vec2};

use super::gridcraft::{G, Palette};
use super::palette::{Cat, Tokens};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Db {
    Connection,
    Database,
    DatabaseOpen,
    Schema,
    Table,
    NewTable,
    View,
    Function,
    Procedure,
    Trigger,
    Query,
    QueryBuilder,
    User,
    Backup,
    Automation,
    Model,
    Bi,
    Key,
    Index,
    Column,
    ForeignKey,
    Import,
    Export,
    Refresh,
    Transaction,
    Commit,
    Rollback,
    CellEditor,
    Columns,
    FormView,
    TextView,
    GridView,
    Design,
    Run,
    RunSelection,
    Sun,
    Moon,
    Objects,
    Beautify,
    Snippet,
    Explain,
    Keyword,
    Update,
    /// Favorites: a star.
    Star,
    /// "Altri" (other objects): three small shapes.
    Others,
    /// The AI chat: a sparkle, as in OpenSlate.
    Sparkle,
}

impl Db {
    pub const ALL: [Db; 46] = [Db::Connection, Db::Database, Db::DatabaseOpen, Db::Schema, Db::Table, Db::NewTable, Db::View, Db::Function, Db::Procedure, Db::Trigger, Db::Query, Db::QueryBuilder, Db::User, Db::Backup, Db::Automation, Db::Model, Db::Bi, Db::Key, Db::Index, Db::Column, Db::ForeignKey, Db::Import, Db::Export, Db::Refresh, Db::Transaction, Db::Commit, Db::Rollback, Db::CellEditor, Db::Columns, Db::FormView, Db::TextView, Db::GridView, Db::Design, Db::Run, Db::RunSelection, Db::Sun, Db::Moon, Db::Objects, Db::Beautify, Db::Snippet, Db::Explain, Db::Keyword, Db::Update, Db::Star, Db::Others, Db::Sparkle];
}

#[allow(clippy::too_many_arguments)]
fn ellipse(g: &G<'_>, cx: f32, cy: f32, rx: f32, ry: f32, fill: Option<Color32>, stroke: Option<Color32>) {
    let pts: Vec<Pos2> = (0..28).map(|i| std::f32::consts::TAU * i as f32 / 28.0).map(|a| g.pt(cx + rx * a.cos(), cy + ry * a.sin())).collect();
    if let Some(f) = fill {
        g.p.add(PathShape::convex_polygon(pts.clone(), f, egui::Stroke::NONE));
    }
    if let Some(c) = stroke {
        g.p.add(PathShape::closed_line(pts, PathStroke::new(1.15 * g.s.clamp(1.0, 2.0), c)));
    }
}

/// A database cylinder; `body` fills it.
fn cylinder(g: &G<'_>, x: f32, y: f32, w: f32, h: f32, body: Color32, c: Color32) {
    let rx = w / 2.0;
    let ry = 1.6_f32.min(h / 4.0);
    let cx = x + rx;
    g.rect(x, y + ry, w, h - 2.0 * ry, Some(body), None);
    ellipse(g, cx, y + h - ry, rx, ry, Some(body), None);
    ellipse(g, cx, y + ry, rx, ry, Some(body.gamma_multiply(1.25)), Some(c));
    g.stroke(&[(x, y + ry), (x, y + h - ry)], c);
    g.stroke(&[(x + w, y + ry), (x + w, y + h - ry)], c);
    // Bottom rim (front half).
    let pts: Vec<(f32, f32)> = (0..=14).map(|i| std::f32::consts::PI * i as f32 / 14.0).map(|a| (cx + rx * a.cos(), y + h - ry + ry * a.sin())).collect();
    g.stroke(&pts, c);
}

/// A small table: header band and a 2×2 grid.
#[allow(clippy::too_many_arguments)]
fn table(g: &G<'_>, x: f32, y: f32, w: f32, h: f32, head: Color32, paper: Color32, c: Color32) {
    g.rect(x, y, w, h, Some(paper), Some(c));
    g.rect(x, y, w, h * 0.3, Some(head), None);
    g.rect(x, y, w, h, None, Some(c));
    let ym = y + h * 0.3 + (h * 0.7) / 2.0;
    g.stroke(&[(x, ym), (x + w, ym)], c.gamma_multiply(0.6));
    g.stroke(&[(x + w / 2.0, y + h * 0.3), (x + w / 2.0, y + h)], c.gamma_multiply(0.6));
}

fn badge_plus(g: &G<'_>, x: f32, y: f32, color: Color32) {
    g.circle(x, y, 3.2, Some(color), None);
    g.line(&[(x - 1.7, y), (x + 1.7, y)], Color32::WHITE, 1.2);
    g.line(&[(x, y - 1.7), (x, y + 1.7)], Color32::WHITE, 1.2);
}

fn arrow_down(g: &G<'_>, x: f32, y0: f32, y1: f32, c: Color32) {
    g.line(&[(x, y0), (x, y1 - 1.0)], c, 1.4);
    g.poly(&[(x - 2.6, y1 - 2.6), (x + 2.6, y1 - 2.6), (x, y1 + 0.4)], c);
}

fn arrow_up(g: &G<'_>, x: f32, y0: f32, y1: f32, c: Color32) {
    g.line(&[(x, y0), (x, y1 + 1.0)], c, 1.4);
    g.poly(&[(x - 2.6, y1 + 2.6), (x + 2.6, y1 + 2.6), (x, y1 - 0.4)], c);
}

/// An arc of a circle centred at (cx, cy) from angle a0 to a1 (radians, clockwise from +x).
#[allow(clippy::too_many_arguments)]
fn arc(g: &G<'_>, cx: f32, cy: f32, r: f32, a0: f32, a1: f32, c: Color32, w: f32) -> (f32, f32) {
    let n = 16;
    let pts: Vec<(f32, f32)> = (0..=n).map(|i| a0 + (a1 - a0) * i as f32 / n as f32).map(|a| (cx + r * a.cos(), cy + r * a.sin())).collect();
    g.line(&pts, c, w);
    pts.last().copied().unwrap_or((cx, cy))
}

pub fn paint(p: &Painter, rect: Rect, icon: Db, c: Color32) {
    let tokens = Tokens::get(p.ctx());
    let pal = Palette::of(&tokens);
    let side = rect.width().min(rect.height());
    let r = Rect::from_center_size(rect.center(), vec2(side, side));
    let g = G { p, r, s: side / 16.0 };
    let paper = pal.paper;
    let blue = pal.cat(Cat::Blue);
    let green = pal.cat(Cat::Green);
    let red = pal.cat(Cat::Red);
    let orange = pal.cat(Cat::Orange);
    let purple = pal.cat(Cat::Purple);
    let teal = pal.cat(Cat::Teal);
    let amber = pal.cat(Cat::Amber);
    let slate = pal.cat(Cat::Slate);
    let light = c.gamma_multiply(0.45);
    let db_body = blue.gamma_multiply(0.35);
    match icon {
        Db::Connection => {
            // Plug: body, two prongs and the cable.
            g.rect(4.5, 5.0, 7.0, 5.5, Some(slate), None);
            g.line(&[(6.3, 5.0), (6.3, 1.8)], c, 1.4);
            g.line(&[(9.7, 5.0), (9.7, 1.8)], c, 1.4);
            g.line(&[(8.0, 10.5), (8.0, 12.0), (10.5, 14.5)], c, 1.4);
        }
        Db::Database => cylinder(&g, 2.5, 1.5, 11.0, 13.0, db_body, c),
        Db::DatabaseOpen => {
            cylinder(&g, 2.5, 1.5, 11.0, 13.0, blue.gamma_multiply(0.6), c);
        }
        Db::Schema => {
            g.poly(&[(1.5, 3.5), (6.0, 3.5), (7.5, 5.0), (14.5, 5.0), (14.5, 13.5), (1.5, 13.5)], amber.gamma_multiply(0.85));
            g.rect(5.0, 7.0, 7.5, 5.0, Some(paper), Some(c));
            g.stroke(&[(5.0, 8.7), (12.5, 8.7)], light);
            g.stroke(&[(8.7, 8.7), (8.7, 12.0)], light);
        }
        Db::Table => table(&g, 1.5, 2.5, 13.0, 11.0, blue, paper, c),
        Db::NewTable => {
            table(&g, 1.0, 2.0, 11.5, 10.0, blue, paper, c);
            badge_plus(&g, 12.0, 12.0, green);
        }
        Db::View => {
            table(&g, 1.5, 1.5, 11.0, 8.5, teal, paper, c);
            // Glasses.
            g.circle(5.5, 12.5, 2.3, Some(paper), Some(c));
            g.circle(11.0, 12.5, 2.3, Some(paper), Some(c));
            g.stroke(&[(7.8, 12.3), (8.7, 12.3)], c);
        }
        Db::Function => g.text(8.0, 8.0, "ƒ(x)", 9.0, purple, (false, true)),
        Db::Procedure => {
            g.text(8.0, 8.0, "{ }", 10.0, c, (false, false));
            g.circle(12.5, 12.5, 2.4, Some(orange), None);
            g.circle(12.5, 12.5, 0.9, Some(paper), None);
        }
        Db::Trigger => g.poly(&[(9.5, 1.5), (3.5, 9.0), (7.5, 9.0), (6.0, 14.5), (12.5, 6.5), (8.5, 6.5)], amber),
        Db::Query => {
            g.rect(3.0, 1.5, 10.0, 13.0, Some(paper), Some(c));
            g.text(8.0, 6.0, "SQL", 5.0, blue, (true, false));
            for y in [9.5, 11.5] {
                g.stroke(&[(5.0, y), (11.0, y)], light);
            }
        }
        Db::QueryBuilder => {
            g.rect(1.0, 2.0, 6.0, 5.5, Some(paper), Some(c));
            g.rect(1.0, 2.0, 6.0, 1.8, Some(blue), None);
            g.rect(9.0, 8.5, 6.0, 5.5, Some(paper), Some(c));
            g.rect(9.0, 8.5, 6.0, 1.8, Some(blue), None);
            g.line(&[(7.0, 5.5), (8.0, 5.5), (8.0, 11.0), (9.0, 11.0)], orange, 1.3);
        }
        Db::User => {
            g.circle(8.0, 5.0, 3.2, Some(amber.gamma_multiply(0.8)), Some(c));
            let pts: Vec<(f32, f32)> = (0..=16).map(|i| std::f32::consts::PI + std::f32::consts::PI * i as f32 / 16.0).map(|a| (8.0 + 5.5 * a.cos(), 14.5 + 5.5 * a.sin())).collect();
            let v: Vec<Pos2> = pts.iter().map(|(x, y)| g.pt(*x, *y)).collect();
            g.p.add(PathShape::convex_polygon(v, blue, egui::Stroke::NONE));
        }
        Db::Backup => {
            cylinder(&g, 1.5, 2.0, 9.0, 11.0, db_body, c);
            g.circle(11.5, 11.5, 3.6, Some(paper), Some(green));
            let end = arc(&g, 11.5, 11.5, 2.0, -2.6, 1.9, green, 1.2);
            g.poly(&[(end.0 - 1.4, end.1 - 0.6), (end.0 + 1.0, end.1 - 1.2), (end.0, end.1 + 1.2)], green);
        }
        Db::Automation => {
            // Robot head with antenna.
            g.rect(3.0, 5.0, 10.0, 8.0, Some(teal.gamma_multiply(0.5)), Some(c));
            g.circle(6.0, 8.8, 1.2, Some(c), None);
            g.circle(10.0, 8.8, 1.2, Some(c), None);
            g.stroke(&[(6.0, 11.2), (10.0, 11.2)], c);
            g.stroke(&[(8.0, 5.0), (8.0, 2.8)], c);
            g.circle(8.0, 2.2, 1.0, Some(red), None);
        }
        Db::Model => {
            g.rect(1.0, 1.5, 6.0, 4.5, Some(orange), None);
            g.rect(9.0, 1.5, 6.0, 4.5, Some(orange), None);
            g.rect(5.0, 10.0, 6.0, 4.5, Some(amber), None);
            g.line(&[(4.0, 6.0), (4.0, 8.0), (8.0, 8.0), (8.0, 10.0)], c, 1.1);
            g.line(&[(12.0, 6.0), (12.0, 8.0), (8.0, 8.0)], c, 1.1);
        }
        Db::Bi => {
            for (x, h, col) in [(2.0, 7.0, purple), (6.0, 11.0, red), (10.0, 9.0, purple)] {
                g.rect(x, 14.0 - h, 3.2, h, Some(col), None);
            }
            g.stroke(&[(1.0, 14.5), (15.0, 14.5)], c);
        }
        Db::Key | Db::ForeignKey => {
            let col = if icon == Db::Key { amber } else { slate };
            g.circle(5.0, 8.0, 3.0, None, Some(col));
            g.line(&[(8.0, 8.0), (14.5, 8.0)], col, 1.5);
            g.line(&[(12.5, 8.0), (12.5, 10.5)], col, 1.5);
            g.line(&[(14.5, 8.0), (14.5, 10.0)], col, 1.5);
            if icon == Db::ForeignKey {
                g.stroke(&[(2.0, 13.5), (6.0, 13.5)], blue);
                g.poly(&[(6.0, 12.0), (8.5, 13.5), (6.0, 15.0)], blue);
            }
        }
        Db::Index => {
            for (i, y) in [3.0f32, 6.5, 10.0, 13.5].iter().enumerate() {
                g.circle(3.0, *y, 1.0, Some(blue), None);
                let w = [10.0, 7.0, 9.0, 6.0].get(i).copied().unwrap_or(8.0);
                g.stroke(&[(5.5, *y), (5.5 + w, *y)], c);
            }
        }
        Db::Column => {
            g.rect(5.0, 1.5, 6.0, 13.0, Some(paper), Some(c));
            g.rect(5.0, 1.5, 6.0, 3.0, Some(blue), None);
            for y in [7.5, 10.5] {
                g.stroke(&[(5.0, y), (11.0, y)], light);
            }
        }
        Db::Import => {
            g.line(&[(2.0, 9.0), (2.0, 14.0), (14.0, 14.0), (14.0, 9.0)], c, 1.3);
            arrow_down(&g, 8.0, 1.5, 11.0, green);
        }
        Db::Export => {
            g.line(&[(2.0, 9.0), (2.0, 14.0), (14.0, 14.0), (14.0, 9.0)], c, 1.3);
            arrow_up(&g, 8.0, 11.5, 2.0, blue);
        }
        Db::Refresh => {
            let e1 = arc(&g, 8.0, 8.0, 5.0, -2.6, 0.2, blue, 1.4);
            g.poly(&[(e1.0 - 2.2, e1.1 - 0.6), (e1.0 + 2.2, e1.1 - 0.6), (e1.0, e1.1 + 2.2)], blue);
            let e2 = arc(&g, 8.0, 8.0, 5.0, 0.55, 3.35, blue, 1.4);
            g.poly(&[(e2.0 - 2.2, e2.1 + 0.6), (e2.0 + 2.2, e2.1 + 0.6), (e2.0, e2.1 - 2.2)], blue);
        }
        Db::Transaction => {
            cylinder(&g, 1.5, 2.0, 9.0, 11.0, db_body, c);
            g.circle(11.8, 11.8, 3.4, Some(orange), None);
            g.poly(&[(10.7, 10.0), (13.6, 11.8), (10.7, 13.6)], Color32::WHITE);
        }
        Db::Commit => {
            cylinder(&g, 1.5, 2.0, 9.0, 11.0, db_body, c);
            g.circle(11.8, 11.8, 3.4, Some(green), None);
            g.line(&[(10.2, 11.9), (11.4, 13.1), (13.5, 10.6)], Color32::WHITE, 1.3);
        }
        Db::Rollback => {
            let end = arc(&g, 8.5, 9.0, 5.0, -1.2, 2.6, red, 1.5);
            let _ = end;
            g.poly(&[(1.5, 3.8), (6.6, 3.0), (4.6, 7.4)], red);
        }
        Db::CellEditor => {
            g.rect(1.5, 4.0, 10.0, 8.0, Some(paper), Some(c));
            g.stroke(&[(3.5, 8.0), (7.5, 8.0)], light);
            g.line(&[(8.0, 13.5), (14.5, 7.0)], blue, 2.2);
            g.poly(&[(8.6, 14.4), (7.0, 15.0), (7.6, 13.4)], c);
        }
        Db::Columns => {
            for x in [2.0, 6.5, 11.0] {
                g.rect(x, 2.0, 3.5, 12.0, Some(paper), Some(c));
                g.rect(x, 2.0, 3.5, 2.6, Some(blue), None);
            }
        }
        Db::FormView => {
            for y in [2.5f32, 6.5, 10.5] {
                g.rect(1.5, y, 4.0, 2.5, Some(light), None);
                g.rect(7.0, y, 7.5, 2.5, Some(paper), Some(c));
            }
        }
        Db::TextView => {
            for (i, y) in [3.0f32, 6.0, 9.0, 12.0].iter().enumerate() {
                let w = if i % 2 == 0 { 12.0 } else { 9.0 };
                g.stroke(&[(2.0, *y), (2.0 + w, *y)], c);
            }
        }
        Db::GridView => table(&g, 1.5, 2.0, 13.0, 12.0, slate, paper, c),
        Db::Design => {
            table(&g, 1.0, 1.5, 10.0, 9.5, blue, paper, c);
            g.line(&[(8.0, 14.5), (14.5, 8.0)], orange, 2.2);
            g.poly(&[(8.6, 15.2), (6.8, 15.6), (7.4, 13.8)], c);
        }
        Db::Run => g.poly(&[(4.0, 2.5), (13.0, 8.0), (4.0, 13.5)], green),
        Db::RunSelection => {
            g.poly(&[(2.5, 3.5), (9.5, 8.0), (2.5, 12.5)], green);
            for y in [4.5, 8.0, 11.5] {
                g.stroke(&[(11.0, y), (14.5, y)], c);
            }
        }
        Db::Sun => {
            g.circle(8.0, 8.0, 3.0, Some(amber), None);
            for k in 0..8 {
                let a = std::f32::consts::TAU * k as f32 / 8.0;
                g.line(&[(8.0 + 4.8 * a.cos(), 8.0 + 4.8 * a.sin()), (8.0 + 6.5 * a.cos(), 8.0 + 6.5 * a.sin())], amber, 1.2);
            }
        }
        Db::Moon => {
            g.circle(8.0, 8.0, 5.5, Some(c), None);
            g.circle(10.5, 6.0, 4.8, Some(paper), None);
        }
        Db::Beautify => {
            // A wand with sparkles.
            g.line(&[(2.5, 13.5), (10.0, 6.0)], c, 2.0);
            g.line(&[(10.0, 6.0), (11.6, 4.4)], amber, 2.2);
            for (x, y, r) in [(13.0, 2.5, 1.6), (6.0, 2.5, 1.1), (13.5, 9.0, 1.1)] {
                g.line(&[(x - r, y), (x + r, y)], amber, 1.1);
                g.line(&[(x, y - r), (x, y + r)], amber, 1.1);
            }
        }
        Db::Snippet => {
            // { } around a line of code.
            g.line(&[(5.0, 2.5), (3.8, 3.2), (3.8, 7.0), (2.2, 8.0), (3.8, 9.0), (3.8, 12.8), (5.0, 13.5)], blue, 1.3);
            g.line(&[(11.0, 2.5), (12.2, 3.2), (12.2, 7.0), (13.8, 8.0), (12.2, 9.0), (12.2, 12.8), (11.0, 13.5)], blue, 1.3);
            g.stroke(&[(6.3, 6.5), (9.7, 6.5)], c);
            g.stroke(&[(6.3, 9.5), (8.7, 9.5)], c);
        }
        Db::Explain => {
            // A plan tree: root with two children.
            g.rect(5.5, 1.5, 5.0, 3.5, Some(orange), None);
            g.rect(1.5, 11.0, 5.0, 3.5, Some(blue), None);
            g.rect(9.5, 11.0, 5.0, 3.5, Some(green), None);
            g.line(&[(8.0, 5.0), (8.0, 8.0), (4.0, 8.0), (4.0, 11.0)], c, 1.1);
            g.line(&[(8.0, 8.0), (12.0, 8.0), (12.0, 11.0)], c, 1.1);
        }
        Db::Keyword => {
            g.rect(1.5, 4.0, 13.0, 8.0, Some(purple.gamma_multiply(0.25)), Some(purple));
            g.p.text(g.pt(8.0, 8.0), egui::Align2::CENTER_CENTER, "K", egui::FontId::proportional(8.5 * g.s), purple);
        }
        Db::Update => {
            // A circle with an arrow up: a newer version.
            g.circle(8.0, 8.0, 6.3, Some(blue), None);
            g.line(&[(8.0, 12.0), (8.0, 6.0)], Color32::WHITE, 1.5);
            g.line(&[(5.2, 8.2), (8.0, 5.0), (10.8, 8.2)], Color32::WHITE, 1.5);
        }
        Db::Star => {
            let pts: Vec<(f32, f32)> = (0..10)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 5.0;
                    let r = if i % 2 == 0 { 6.8 } else { 2.9 };
                    (8.0 + r * a.cos(), 8.4 + r * a.sin())
                })
                .collect();
            // a star is not convex: a fan of triangles from the centre
            for i in 0..10 {
                let (a, b) = (pts[i], pts[(i + 1) % 10]);
                g.poly(&[(8.0, 8.4), a, b], amber);
            }
            let mut ring = pts.clone();
            ring.push(pts[0]);
            g.stroke(&ring, amber.gamma_multiply(0.8));
        }
        Db::Sparkle => {
            // a big four-point star and a small one
            let star = |cx: f32, cy: f32, r: f32, w: f32, col: Color32| {
                g.poly(&[(cx, cy - r), (cx + w, cy - w), (cx + r, cy), (cx + w, cy + w), (cx, cy + r), (cx - w, cy + w), (cx - r, cy), (cx - w, cy - w)], col);
            };
            star(7.0, 9.0, 6.0, 1.5, purple);
            star(12.8, 3.6, 2.8, 0.8, blue);
            let _ = c;
        }
        Db::Others => {
            g.rect(1.5, 1.5, 5.5, 5.5, Some(blue), None);
            g.circle(11.8, 4.3, 2.9, Some(teal), None);
            g.poly(&[(4.3, 9.0), (7.4, 14.5), (1.2, 14.5)], orange);
            g.rect(9.0, 9.0, 5.5, 5.5, Some(purple), None);
            let _ = c;
        }
        Db::Objects => {
            for (y, col) in [(2.0f32, blue), (6.5, teal), (11.0, purple)] {
                g.rect(1.5, y, 3.0, 3.0, Some(col), None);
                g.stroke(&[(6.0, y + 1.5), (14.5, y + 1.5)], c);
            }
        }
    }
}
