//! The update bar under the toolbar (as in ArtCraft's launcher and OpenSlate): "È disponibile…"
//! with "Aggiorna", then the progress, then "Riavvia ora". Plus the "Aggiornamenti" menu.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use egui::{Color32, RichText, Stroke};

use crate::icons::{self, Db, G};
use crate::selfupdate::{self, Phase, Ready};
use crate::store::Settings;
use crate::update::{self, Release};
use crate::i18n;

enum Event {
    Checked { result: Result<Option<Release>, String>, manual: bool },
    Phase(Phase),
    Done(Result<Ready, String>),
}

pub struct Updater {
    pub settings: Settings,
    update: Option<Release>,
    hidden: bool,
    phase: Option<Phase>,
    ready: Option<Ready>,
    error: Option<String>,
    /// Next automatic check (the first a few seconds after start).
    next: Option<Instant>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    /// Messages for the notices (text, is error).
    pub notices: Vec<(String, bool)>,
}

impl Updater {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Updater {
            settings: Settings::load(),
            update: None,
            hidden: false,
            phase: None,
            ready: None,
            error: None,
            next: Some(Instant::now() + std::time::Duration::from_secs(4)),
            tx,
            rx,
            notices: Vec::new(),
        }
    }

    /// Asks GitHub, in a thread, whether there is a new version.
    pub fn check(&mut self, ctx: &egui::Context, manual: bool) {
        self.next = None;
        if manual {
            self.notices.push(("Cerco aggiornamenti…".into(), false));
        }
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = update::check();
            let _ = tx.send(Event::Checked { result, manual });
            ctx.request_repaint();
        });
    }

    /// Downloads and installs the new version (in a thread).
    pub fn install(&mut self, ctx: &egui::Context) {
        let Some(r) = self.update.clone() else { return };
        let Some(install) = selfupdate::current() else {
            selfupdate::open_url(&r.download);
            return;
        };
        if self.phase.is_some() {
            return;
        }
        self.error = None;
        self.phase = Some(Phase::Downloading { done: 0, total: 0 });
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let res = selfupdate::prepare(&install, &r, &mut |p| {
                let _ = tx.send(Event::Phase(p));
                ctx.request_repaint();
            });
            let _ = tx.send(Event::Done(res));
            ctx.request_repaint();
        });
    }

    /// Handles the results of the threads and starts the periodic check.
    pub fn tick(&mut self, ctx: &egui::Context) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                Event::Checked { result, manual } => self.checked(ctx, result, manual),
                Event::Phase(p) => self.phase = Some(p),
                Event::Done(r) => {
                    self.phase = None;
                    self.hidden = false;
                    match r {
                        Ok(ready) => self.ready = Some(ready),
                        Err(e) => {
                            eprintln!("aggiornamento: {e}");
                            self.error = Some(e);
                        }
                    }
                }
            }
        }
        // update notice: at start and then every now and then
        if self.settings.update_check && update::from_release() {
            match self.next {
                Some(t) if Instant::now() >= t => self.check(ctx, false),
                Some(t) => ctx.request_repaint_after(t.saturating_duration_since(Instant::now())),
                None => {}
            }
        }
    }

    fn checked(&mut self, ctx: &egui::Context, result: Result<Option<Release>, String>, manual: bool) {
        self.next = Some(Instant::now() + update::EVERY);
        match result {
            Ok(Some(r)) => {
                if manual {
                    self.notices.push((format!("È disponibile DbCraft {}", r.version), false));
                }
                // a skipped version comes back only when asked from the menu
                let already = self.ready.as_ref().is_some_and(|x| x.version == r.version) || self.phase.is_some();
                if (manual || r.version != self.settings.update_skip) && !already {
                    if self.update.as_ref() != Some(&r) {
                        self.hidden = false;
                        self.error = None;
                    }
                    self.update = Some(r);
                    // install by itself: downloaded and installed without asking
                    if self.settings.update_auto && self.error.is_none() && selfupdate::current().is_some_and(|i| selfupdate::silent(&i)) {
                        self.install(ctx);
                    }
                }
            }
            Ok(None) => {
                if manual {
                    self.notices.push((format!("DbCraft è aggiornato (versione {})", update::current()), false));
                }
            }
            Err(e) => {
                eprintln!("aggiornamenti: {e}");
                if manual {
                    self.notices.push((format!("Aggiornamenti: {e}"), true));
                }
            }
        }
    }

    /// The "Aggiornamenti" menu (toolbar).
    pub fn menu(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(i18n::tf(format!("DbCraft {}", update::current()))).strong());
        if !update::from_release() {
            ui.label(RichText::new(i18n::t("Compilato dai sorgenti: aggiorna con git pull")).small().weak());
        }
        ui.separator();
        if icons::menu_item(ui, Db::Update, "Cerca aggiornamenti").clicked() {
            self.check(ui.ctx(), true);
            ui.close();
        }
        if let Some(r) = self.update.clone()
            && icons::menu_item(ui, G::Info, &format!("Novità della {}", r.version)).clicked()
        {
            selfupdate::open_url(&r.page);
            ui.close();
        }
        ui.separator();
        let mut changed = crate::theme::checkbox(ui, &mut self.settings.update_check, i18n::t("Avvisa quando c'è una versione nuova")).changed();
        changed |= ui
            .add_enabled(self.settings.update_check, crate::theme::check(&mut self.settings.update_auto, i18n::t("Installala da sola (si usa dal prossimo avvio)")))
            .changed();
        ui.label(RichText::new(i18n::t("Chiede a GitHub l'ultima release di DbCraft all'avvio;\nconnessioni e impostazioni restano anche aggiornando.")).small().weak());
        if changed {
            self.settings.save();
            if self.settings.update_check && self.next.is_none() && self.phase.is_none() {
                self.next = Some(Instant::now());
            }
        }
    }

    /// The bar under the toolbar. Returns true when the user asks to restart now.
    pub fn banner(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(r) = self.update.clone() else { return false };
        if self.hidden {
            return false;
        }
        let dark = ui.visuals().dark_mode;
        let (fill, edge) = if dark {
            (Color32::from_rgb(0x1d, 0x2a, 0x42), Color32::from_rgb(0x2f, 0x4a, 0x7a))
        } else {
            (Color32::from_rgb(0xE9, 0xF0, 0xFD), Color32::from_rgb(0xBF, 0xD3, 0xF7))
        };
        let accent = Color32::from_rgb(0x25, 0x63, 0xEB);
        let red = Color32::from_rgb(0xD1, 0x34, 0x38);
        let can_self = selfupdate::current().is_some();
        let phase = self.phase.clone();
        let ready = self.ready.clone();
        let error = self.error.clone();
        let mut restart = false;
        egui::Panel::top("update_banner")
            .exact_size(30.0)
            .frame(egui::Frame::NONE.fill(fill).stroke(Stroke::new(1.0, edge)).inner_margin(egui::Margin { left: 14, right: 10, top: 2, bottom: 2 }))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let (r_icon, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                    if ready.is_some() {
                        icons::paint(ui.painter(), r_icon, G::Check, accent);
                    } else if error.is_some() {
                        icons::paint(ui.painter(), r_icon, G::Info, red);
                    } else {
                        icons::paint(ui.painter(), r_icon, Db::Update, accent);
                    }
                    let keep = "Connessioni e impostazioni restano: stanno nella cartella di configurazione, fuori dall'app, e le password nel portachiavi.";
                    let text = if let Some(rd) = &ready {
                        if rd.on_exit {
                            format!("DbCraft {} si installa quando chiudi l'app.", rd.version)
                        } else {
                            format!("DbCraft {} è installato: si apre col prossimo avvio.", rd.version)
                        }
                    } else if let Some(p) = &phase {
                        match p {
                            Phase::Downloading { done, total } if *total > 0 => {
                                format!("Aggiorno DbCraft a {}: scarico {:.0} di {:.0} MB…", r.version, *done as f64 / 1e6, *total as f64 / 1e6)
                            }
                            Phase::Downloading { .. } => format!("Aggiorno DbCraft a {}: scarico…", r.version),
                            Phase::Verifying => format!("Aggiorno DbCraft a {}: controllo il file…", r.version),
                            Phase::Installing => format!("Aggiorno DbCraft a {}: installo…", r.version),
                        }
                    } else if let Some(e) = &error {
                        format!("Aggiornamento a DbCraft {} non riuscito: {}", r.version, e.lines().next().unwrap_or(""))
                    } else {
                        format!("È disponibile DbCraft {} (questa è la {}).", r.version, update::current())
                    };
                    let color = ui.visuals().text_color();
                    let lbl = ui.add(egui::Label::new(RichText::new(i18n::tf(text)).color(color)).truncate());
                    match &error {
                        Some(e) => lbl.on_hover_text(e.as_str()),
                        None => lbl.on_hover_text(i18n::t(keep)),
                    };
                    if let Some(Phase::Downloading { done, total }) = &phase
                        && *total > 0
                    {
                        ui.add(egui::ProgressBar::new(*done as f32 / *total as f32).desired_width(120.0).desired_height(8.0));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let primary = |ui: &mut egui::Ui, label: &str| ui.add(egui::Button::new(RichText::new(i18n::tr(label)).color(Color32::WHITE)).fill(accent).small());
                        if phase.is_some() {
                            return; // while downloading and installing: no buttons
                        }
                        if crate::app::close_button(ui).on_hover_text(i18n::t("Più tardi (torna al prossimo avvio)")).clicked() {
                            self.hidden = true;
                        }
                        if let Some(rd) = &ready {
                            if !rd.on_exit
                                && primary(ui, "Riavvia ora")
                                    .on_hover_text(i18n::t("Chiude DbCraft e lo riapre aggiornato (le modifiche non salvate nelle schede vanno perse)."))
                                    .clicked()
                            {
                                restart = true;
                            }
                            return;
                        }
                        if error.is_some() && ui.small_button(i18n::t("Scarica")).clicked() {
                            selfupdate::open_url(&r.download);
                        }
                        if error.is_none() && ui.small_button(i18n::t("Salta questa versione")).clicked()
                            && let Some(r) = self.update.take()
                        {
                            self.settings.update_skip = r.version;
                            self.settings.save();
                        }
                        if ui.small_button(i18n::t("Novità")).clicked() {
                            selfupdate::open_url(&r.page);
                        }
                        if can_self {
                            let label = if error.is_some() { "Riprova" } else { "Aggiorna ora" };
                            if primary(ui, label)
                                .on_hover_text(i18n::t("Scarica la nuova versione, controlla il file e la installa al posto di questa. Connessioni e impostazioni restano."))
                                .clicked()
                            {
                                self.install(ui.ctx());
                            }
                        } else if primary(ui, "Scarica")
                            .on_hover_text(i18n::t("Scarica la nuova versione dal sito di GitHub: si installa come la prima volta, sostituendo l'app. Connessioni e impostazioni restano."))
                            .clicked()
                        {
                            selfupdate::open_url(&r.download);
                        }
                    });
                });
            });
        restart
    }

    /// "Riavvia ora": starts the helper that reopens the app; the caller then closes it.
    pub fn restart(&mut self) -> bool {
        let Some(r) = &self.ready else { return false };
        match selfupdate::relaunch_after_exit(r) {
            Ok(()) => true,
            Err(e) => {
                self.notices.push((e, true));
                false
            }
        }
    }
}
