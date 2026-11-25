//! Das COSMIC-Panel-Applet: Multi-Provider-Quota-Anzeige.
//!
//! Panel: je konfiguriertem Anbieter (über der Panel-Grenze) Logo + farbige
//! Prozent-Segmente. Popup: je konfiguriertem Anbieter eine Sektion mit
//! Wertzeilen und Reset-Zeiten, plus Einstellungen/Beenden. Refresh:
//! selbst-planende Task-Kette — ein Sleep pro Zyklus (60 s), alle Anbieter
//! parallel. Config-Änderungen (Settings-Fenster) werden per 1-s-mtime-Poll
//! innerhalb ~1 s übernommen (kein Neustart).

use crate::cache::{self, Snapshots};
use crate::config::{self, AppConfig};
use crate::fl;
use crate::quota::{percent_label, usage_level, ProviderSnapshot, UsageLevel, WindowKind};
use chrono::{DateTime, Local};
use cosmic::app::{Core, Task};
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::{window, Alignment, Length, Rectangle};
use cosmic::widget::{autosize, button, column, container, divider, icon, row, space, text, Id};
use cosmic::{applet, theme, Action, Element};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

pub const APP_ID: &str = "net.d11n.CosmicTokenQuota";

const REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const POPUP_WIDTH: f32 = 420.0;
const SETTINGS_BINARY: &str = "cosmic-applet-token-quota-settings";

static AUTOSIZE_MAIN_ID: LazyLock<Id> = LazyLock::new(|| Id::new("autosize-main"));

/// Startet das Applet (aufgerufen aus `main`).
pub fn run() -> cosmic::iced::Result {
    cosmic::applet::run::<QuotaApplet>(())
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Fetch-Ergebnis für eine Anbieter-ID (Registry-Schlüssel).
    Fetched(String, Result<ProviderSnapshot, String>),
    /// Nächster Refresh-Zyklus.
    Tick,
    /// Config-Datei hat sich geändert (Settings-Fenster hat gespeichert).
    ConfigChanged(Option<std::time::SystemTime>),
    /// Button-Bounds (in Fenster-Koordinaten) für die Popup-Positionierung.
    TogglePopup(Rectangle),
    PopupClosed(window::Id),
    OpenSettings,
    Quit,
}

#[derive(Default)]
pub struct QuotaApplet {
    core: Core,
    cfg: AppConfig,
    snapshots: Snapshots,
    errors: HashMap<String, String>,
    /// mtime der Config beim letzten Beobachten (Basis für den Watcher).
    config_mtime: Option<std::time::SystemTime>,
    popup: Option<window::Id>,
}

/// Farbstufe des ANGEZEIGTEN (gerundeten) Verbrauchs.
fn level_color(level: UsageLevel) -> Option<cosmic::iced::Color> {
    let binding = cosmic::theme::active();
    let theme = binding.cosmic();
    match level {
        UsageLevel::Normal => None,
        UsageLevel::Warning => Some(theme.warning.base.into()),
        UsageLevel::Critical => Some(theme.destructive.base.into()),
    }
}

/// Panel-Segmente EINES Anbieters: Prozentwerte der Panel-Fenster (ohne
/// MCP), Trenner „ · ", Buchstaben W/M an Weekly/Monthly.
pub fn provider_panel_segments(snap: &ProviderSnapshot) -> Vec<(String, UsageLevel)> {
    let mut segments: Vec<(String, UsageLevel)> = Vec::new();
    for window in &snap.windows {
        if !crate::quota::in_panel(window.kind) {
            continue;
        }
        if !segments.is_empty() {
            segments.push((" · ".to_string(), UsageLevel::Normal));
        }
        let mut label = percent_label(window.percent_used);
        if let Some(letter) = crate::quota::panel_letter(window.kind) {
            label.push(' ');
            label.push_str(letter);
        }
        segments.push((label, usage_level(window.percent_used)));
    }
    segments
}

/// Popup-Wertzeilen EINES Anbieters: (Label, Prozent, Stufe, Reset-Text).
/// Die 5h-Zeile zeigt keinen Countdown, solange der angezeigte Wert 0 % ist.
pub fn provider_popup_rows(
    snap: &ProviderSnapshot,
) -> Vec<(String, String, UsageLevel, Option<String>)> {
    snap.windows
        .iter()
        .map(|window| {
            let label = match window.kind {
                WindowKind::FiveHours => fl!("popup-rolling"),
                WindowKind::Weekly => fl!("popup-week"),
                WindowKind::Monthly => fl!("popup-month"),
                WindowKind::McpMonthly => fl!("popup-mcp"),
            };
            let reset = if window.kind == WindowKind::FiveHours
                && window.percent_used.round() as i64 == 0
            {
                None
            } else {
                Some(fl!("resets-in", d = fmt_reset_in(window.reset_unix)))
            };
            (
                label,
                percent_label(window.percent_used),
                usage_level(window.percent_used),
                reset,
            )
        })
        .collect()
}

/// Fetch-Ziele aus der Config: nur Zeilen mit nicht-leerem Key.
/// Fetch-Ziele aus der Config: nur Zeilen mit nicht-leerem Key.
/// Liefert (Anbieter, Key, Instanz-ID) — Instanz-IDs machen
/// Mehrfach-Einträge desselben Anbieters routing-fähig.
pub fn configured_fetch_targets(
    cfg: &AppConfig,
) -> Vec<(crate::providers::ProviderKind, String, String)> {
    cfg.providers
        .iter()
        .enumerate()
        .filter(|(_, row)| !row.api_key.trim().is_empty())
        .map(|(i, row)| {
            (
                row.provider,
                row.api_key.trim().to_string(),
                crate::config::row_id(i, row.provider),
            )
        })
        .collect()
}

/// Ergebnis eines Fetch-Zyklus anwenden: Ok ersetzt den Snapshot und
/// löscht den Fehler; Err behält den letzten Snapshot und setzt den Fehler.
pub fn merge_fetched(
    snapshots: &mut Snapshots,
    errors: &mut HashMap<String, String>,
    id: &str,
    res: &Result<ProviderSnapshot, String>,
) {
    match res {
        Ok(snap) => {
            snapshots.insert(id.to_string(), snap.clone());
            errors.remove(id);
        }
        Err(message) => {
            errors.insert(id.to_string(), message.clone());
        }
    }
}

/// Zeit bis zum Reset ab Unix-Sekunden, relativ formatiert (z. B.
/// `3 Tagen 4 Stunden`).
pub fn fmt_reset_in(reset_unix: i64) -> String {
    let reset = DateTime::from_timestamp(reset_unix, 0).map(|d| d.with_timezone(&Local));
    match reset {
        Some(reset) => {
            let dur = reset.signed_duration_since(Local::now());
            format_duration(dur.num_seconds().max(0))
        }
        None => reset_unix.to_string(),
    }
}

/// Sekunden als lokalisierte Dauer mit korrekter Ein-/Mehrzahl,
/// z. B. `1 Stunde 30 Minuten` oder `3 Tagen 4 Stunden`.
pub fn format_duration(secs: i64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h >= 24 {
        let d = h / 24;
        let h_rem = h % 24;
        if h_rem > 0 {
            format!(
                "{} {}",
                fl!("duration-day", n = d),
                fl!("duration-hour", n = h_rem)
            )
        } else {
            fl!("duration-day", n = d)
        }
    } else if h > 0 {
        if m > 0 {
            format!(
                "{} {}",
                fl!("duration-hour", n = h),
                fl!("duration-minute", n = m)
            )
        } else {
            fl!("duration-hour", n = h)
        }
    } else if m > 0 {
        fl!("duration-minute", n = m)
    } else {
        fl!("duration-second", n = secs)
    }
}

pub fn fmt_updated(unix: i64) -> String {
    DateTime::from_timestamp(unix, 0)
        .map(|d| d.with_timezone(&Local).format("%H:%M:%S").to_string())
        .unwrap_or_else(|| unix.to_string())
}

// ============================================================================
// Fetch-Kette und Config-Watcher
// ============================================================================

/// Alle Fetch-Ziele parallel (eigener Client je Call — Alt-Muster).
fn fetch_all(cfg: &AppConfig) -> Task<Message> {
    let tasks: Vec<Task<Message>> = configured_fetch_targets(cfg)
        .into_iter()
        .map(|(kind, key, id)| {
            Task::perform(
                async move {
                    let client = reqwest::Client::new();
                    crate::providers::fetch(&client, kind, &key).await
                },
                move |res| Action::App(Message::Fetched(id.clone(), res.map_err(|e| e.message))),
            )
        })
        .collect();
    Task::batch(tasks)
}

/// Ein voller Zyklus: Fetches + EIN Sleep (kein Tick-Sturm).
fn cycle(cfg: &AppConfig) -> Task<Message> {
    Task::batch([
        fetch_all(cfg),
        Task::perform(tokio::time::sleep(REFRESH_INTERVAL), |_| {
            Action::App(Message::Tick)
        }),
    ])
}

/// Wacht alle 1 s und meldet eine geänderte Config-mtime (HA-Muster).
async fn wait_for_config_change(
    baseline: Option<std::time::SystemTime>,
) -> Option<std::time::SystemTime> {
    let path = config::config_path()?;
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if let Some(t) = mtime {
            if baseline.is_none_or(|b| t != b) {
                return Some(t);
            }
        }
    }
}

/// Pfad des Settings-Binarys neben dem laufenden Applet-Binary.
pub fn settings_binary_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    Some(dir.join(SETTINGS_BINARY).to_string_lossy().into_owned())
}

// ============================================================================
// Views
// ============================================================================

fn provider_logo(
    state: &QuotaApplet,
    kind: crate::providers::ProviderKind,
) -> Element<'_, Message> {
    let mut handle = icon::from_svg_bytes(kind.logo_svg());
    handle.symbolic = true;
    let (w, h) = state.core.applet.suggested_size(true);
    icon::icon(handle)
        .width(Length::Fixed(w as f32))
        .height(Length::Fixed(h as f32))
        .into()
}

/// Panel: je Panel-Anbieter (mit Key) Logo + Segmente; ohne Daten je
/// Anbieter `Err`/`–`; gar keine Gruppe → einzelner `–`-Platzhalter.
fn panel_element(state: &QuotaApplet) -> Element<'_, Message> {
    let mut groups: Vec<Element<'_, Message>> = Vec::new();
    let panel_count = state.cfg.panel_rows().len();
    for (i, row_cfg) in state.cfg.providers.iter().enumerate() {
        if i >= panel_count {
            break;
        }
        if row_cfg.api_key.trim().is_empty() {
            continue; // keyless: gar nicht im Panel
        }
        let id = crate::config::row_id(i, row_cfg.provider);
        let segments: Vec<(String, UsageLevel)> = match state.snapshots.get(id.as_str()) {
            Some(snap) => provider_panel_segments(snap),
            None => vec![(
                match state.errors.get(id.as_str()) {
                    Some(_) => fl!("panel-error"),
                    None => "–".to_string(),
                },
                UsageLevel::Normal,
            )],
        };
        let label: Element<'_, Message> = row(segments.into_iter().map(|(t, level)| {
            let t = state.core.applet.text(t);
            match level_color(level) {
                Some(c) => t.class(c).into(),
                None => t.into(),
            }
        }))
        .into();
        groups.push(
            row![provider_logo(state, row_cfg.provider), label]
                .spacing(4)
                .align_y(Alignment::Center)
                .into(),
        );
    }
    if groups.is_empty() {
        let t = state.core.applet.text("–");
        groups.push(t.into());
    }
    let content = if state.core.applet.is_horizontal() {
        Element::from(row(groups).spacing(12).align_y(Alignment::Center))
    } else {
        Element::from(column(groups).spacing(2).align_x(Alignment::Center))
    };
    content
}

/// Popup-Kopf einer Anbieter-Sektion: Name (+ Plan-Tier) links,
/// „Aktualisiert" rechts.
fn popup_section_header<'a>(
    state: &'a QuotaApplet,
    id: &str,
    kind: crate::providers::ProviderKind,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;
    let (space_m, space_xxs) = (spacing.space_m, spacing.space_xxs);
    let updated = state
        .snapshots
        .get(id)
        .map(|s| fl!("updated-at", t = fmt_updated(s.fetched_at_unix)));
    let mut title = kind.display().to_string();
    if let Some(level) = state.snapshots.get(id).and_then(|s| s.level.as_deref()) {
        let mut cap = level.to_string();
        if let Some(first) = cap.chars().next() {
            cap = first.to_uppercase().collect::<String>() + &cap[first.len_utf8()..];
        }
        title.push_str(" · ");
        title.push_str(&cap);
    }
    match updated {
        Some(u) => row![text::heading(title), space::horizontal(), text::body(u),]
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .padding([space_xxs, space_m, space_xxs, space_m])
            .into(),
        None => container(text::heading(title))
            .width(Length::Fill)
            .padding([space_xxs, space_m, space_xxs, space_m])
            .into(),
    }
}

fn popup_element(state: &QuotaApplet) -> Element<'_, Message> {
    let Spacing {
        space_xxs,
        space_s,
        space_m,
        ..
    } = cosmic::theme::active().cosmic().spacing;
    let padding_x = space_m;
    // Gesamtes Popup etwas breiter als den naturalen Inhalt (Rückmeldung
    // 2026-11-25): feste Zielbreite, die Fill-Zeilen spannen sich darein —
    // der Innenabstand (space_m an den Zeilen) bleibt unverändert.
    let mut col = column![]
        .width(Length::Fill)
        .padding([space_s, 0, space_xxs, 0])
        .spacing(space_xxs);

    for (i, row_cfg) in state.cfg.providers.iter().enumerate() {
        if i > 0 {
            // Sektionstrenner ZWISCHEN Anbietern (der Trennstrich direkt
            // unter dem Header ist entfallen — weniger vertikale Luft).
            col = col.push(container(divider::horizontal::default()).padding([space_xxs, 0]));
        }
        let id = crate::config::row_id(i, row_cfg.provider);
        col = col.push(popup_section_header(state, &id, row_cfg.provider));

        if row_cfg.api_key.trim().is_empty() {
            col = col.push(container(text::body(fl!("hint-no-key"))).padding([0, padding_x]));
            continue;
        }
        match state.snapshots.get(id.as_str()) {
            Some(snap) => {
                for (label, percent, level, reset) in provider_popup_rows(snap) {
                    let heading = text::heading(format!("{label}: "));
                    let value = match level_color(level) {
                        Some(c) => text::heading(percent).class(c),
                        None => text::heading(percent),
                    };
                    let mut line = row![heading, value, space::horizontal()]
                        .width(Length::Fill)
                        .padding([0, padding_x])
                        .align_y(Alignment::Center);
                    if let Some(reset_text) = reset {
                        line = line.push(text::body(reset_text));
                    }
                    col = col.push(line);
                }
                if let Some(error) = state.errors.get(id.as_str()) {
                    col = col.push(
                        container(text::body(fl!("error-line", e = error))).padding([0, padding_x]),
                    );
                }
            }
            None => {
                let msg = match state.errors.get(id.as_str()) {
                    Some(e) => fl!("error-line", e = e),
                    None => fl!("no-data"),
                };
                col = col.push(
                    container(text::body(msg))
                        .width(Length::Fill)
                        .padding([0, padding_x]),
                );
            }
        }
    }

    // Footer: Trennlinie + Einstellungen + Beenden.
    col = col.push(container(divider::horizontal::default()).padding([space_xxs, 0, 0, 0]));
    col =
        col.push(applet::menu_button(text::body(fl!("settings"))).on_press(Message::OpenSettings));
    col = col.push(applet::menu_button(text::body(fl!("quit"))).on_press(Message::Quit));
    // Gesamtes Popup auf Zielbreite spannen (Rückmeldung 2026-11-25) —
    // die Fill-Zeilen füllen sie, der Innenabstand bleibt space_m.
    container(col).width(Length::Fixed(POPUP_WIDTH)).into()
}

// ============================================================================
// Application-Impl
// ============================================================================

impl cosmic::Application for QuotaApplet {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn init(core: Core, _flags: ()) -> (Self, Task<Message>) {
        let cfg = config::load();
        let snapshots = config::config_path()
            .is_none()
            .then(Snapshots::new)
            .unwrap_or_else(|| cache::load_snapshots(cache::cache_path().as_deref()));
        let config_mtime = config::config_path()
            .and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        let cfg_for_cycle = cfg.clone();
        (
            Self {
                core,
                cfg,
                snapshots,
                errors: HashMap::new(),
                config_mtime,
                popup: None,
            },
            Task::batch([
                cycle(&cfg_for_cycle),
                Task::perform(wait_for_config_change(config_mtime), |mtime| {
                    Action::App(Message::ConfigChanged(mtime))
                }),
            ]),
        )
    }

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Fetched(id, res) => {
                merge_fetched(&mut self.snapshots, &mut self.errors, &id, &res);
                if let Err(error) = &res {
                    tracing::warn!("Fetch für {id} fehlgeschlagen: {error}");
                }
                if let Some(path) = cache::cache_path() {
                    if let Err(error) = cache::save_snapshots(Some(&path), &self.snapshots) {
                        tracing::warn!("Konnte Cache nicht schreiben: {error}");
                    }
                }
                Task::none()
            }
            Message::Tick => cycle(&self.cfg),
            Message::ConfigChanged(mtime) => {
                self.config_mtime = mtime;
                self.cfg = config::load();
                let cfg = self.cfg.clone();
                Task::batch([
                    cycle(&cfg),
                    Task::perform(wait_for_config_change(mtime), |m| {
                        Action::App(Message::ConfigChanged(m))
                    }),
                ])
            }
            Message::TogglePopup(bounds) => {
                if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    let new_id = window::Id::unique();
                    self.popup = Some(new_id);
                    let mut popup_settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    );
                    popup_settings.positioner.size = None;
                    // Anker = gesamtes Widget (nicht nur das Icon-Quadrat),
                    // damit das Popup zentriert darunter erscheint.
                    popup_settings.positioner.anchor_rect = Rectangle {
                        x: bounds.x as i32,
                        y: bounds.y as i32,
                        width: bounds.width.max(1.0) as i32,
                        height: bounds.height.max(1.0) as i32,
                    };
                    get_popup(popup_settings)
                }
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
                Task::none()
            }
            Message::OpenSettings => match settings_binary_path() {
                Some(path) => {
                    let _ = std::process::Command::new(&path).spawn();
                    Task::none()
                }
                None => {
                    tracing::warn!(
                        "Settings-Binary '{SETTINGS_BINARY}' nicht neben dem Applet gefunden"
                    );
                    Task::none()
                }
            },
            Message::Quit => {
                std::process::exit(0);
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let padding = self.core.applet.suggested_padding(true).0;
        let btn = button::custom(panel_element(self))
            .padding(if self.core.applet.is_horizontal() {
                [0, padding]
            } else {
                [padding, 0]
            })
            .class(theme::Button::AppletIcon)
            .on_press_with_rectangle(move |offset, bounds| {
                Message::TogglePopup(Rectangle {
                    x: bounds.x - offset.x,
                    y: bounds.y - offset.y,
                    width: bounds.width,
                    height: bounds.height,
                })
            });

        autosize::autosize(btn, AUTOSIZE_MAIN_ID.clone()).into()
    }

    fn view_window(&self, _id: window::Id) -> Element<'_, Message> {
        self.core.applet.popup_container(popup_element(self)).into()
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(applet::style())
    }
}
