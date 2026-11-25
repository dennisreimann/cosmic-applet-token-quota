//! Eigenständiges Einstellungsfenster für das Applet.
//!
//! Das Applet selbst kann aus dem Panel-Applet-Kontext keine normalen
//! Fenster öffnen (privilegierter Panel-Wayland-Socket, nur
//! Layer-Surfaces). Das Applet spawnt daher dieses Binary
//! (`Message::OpenSettings` → `settings_binary_path()`).
//!
//! UI: sortierbare Anbieter-Liste (Dropdown + API-Key je Zeile, Logo
//! davor), Trennlinie = Panel-Grenze (Zeilen darüber erscheinen im Panel)
//! — Muster aus cosmic-applet-homeassistant.

use cosmic::app::{Core, Task};
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::{window, Alignment, Length};
use cosmic::widget::{button, column, container, divider, icon, row, scrollable, text, text_input};
use cosmic::{Application, Element};
use cosmic_applet_token_quota::config::{
    self, move_row_across, remove_row, AppConfig, ProviderRow,
};
use cosmic_applet_token_quota::fl;
use cosmic_applet_token_quota::providers::ProviderKind;

fn main() -> cosmic::iced::Result {
    cosmic::app::run::<SettingsApp>(
        cosmic::app::Settings::default()
            .size(cosmic::iced::Size::new(660.0, 640.0))
            // Breite fix (nur in der Höhe skalierbar) — breit genug für
            // Logo + Dropdown + Key + Pfeil-Cluster + Trash ohne Clipping
            // (Rückmeldung 2026-11-25: Pfeile waren abgeschnitten).
            .size_limits(
                cosmic::iced::Limits::NONE
                    .min_width(660.0)
                    .min_height(400.0)
                    .max_width(660.0)
                    .max_height(1200.0),
            ),
        (),
    )
}

/// Feste Breite des API-Key-Inputs (ziemlich breit — Keys sind lang).
const KEY_WIDTH: f32 = 220.0;
/// Feste Breite des Anbieter-Dropdowns (unabhängig vom Label — die
/// Key-Inputs der Zeilen fluchten dadurch).
const DROPDOWN_WIDTH: f32 = 150.0;
/// Logo-Breite in den Zeilen.
const LOGO_WIDTH: f32 = 20.0;

#[derive(Debug, Clone)]
enum Message {
    /// Dropdown-Auswahl in Zeile `row` → neuer Anbieter.
    ProviderChanged(usize, ProviderKind),
    KeyChanged(usize, String),
    MoveRowUp(usize),
    MoveRowDown(usize),
    BoundaryUp,
    BoundaryDown,
    AddRow,
    RemoveRow(usize),
    Save,
}

#[derive(Default)]
struct SettingsApp {
    core: Core,
    rows: Vec<ProviderRow>,
    /// Grenzindex: rows[..boundary] erscheinen im Panel. rows.len() = alle.
    boundary: usize,
    error: Option<String>,
}

/// Kanonische Anbieter-Reihenfolge (Spec 2026-11-25).
const KINDS: [ProviderKind; 2] = [ProviderKind::OpencodeGo, ProviderKind::ZaiCodingPlan];

impl SettingsApp {
    /// Dropdown-Optionen für Zeile `row_idx`: ALLE Anbieter in kanonischer
    /// Reihenfolge — Mehrfach-Einträge desselben Anbieters sind erlaubt
    /// (Rückmeldung 2026-11-25), der Index ist damit deterministisch.
    fn options_for(&self, _row_idx: usize) -> Vec<(ProviderKind, String)> {
        KINDS
            .into_iter()
            .map(|kind| (kind, kind.display().to_string()))
            .collect()
    }

    /// Index des eigenen Anbieters in der gefilterten Optionsliste.
    fn selected_index(&self, row_idx: usize) -> Option<usize> {
        let own = self.rows.get(row_idx)?.provider;
        self.options_for(row_idx)
            .iter()
            .position(|(kind, _)| *kind == own)
    }

    fn row_view(&self, i: usize, spacing: f32) -> Element<'_, Message> {
        let Some(row_cfg) = self.rows.get(i) else {
            return row![].into();
        };
        let options = self.options_for(i);
        let selected = self.selected_index(i);
        let labels: Vec<String> = options.iter().map(|(_, label)| label.clone()).collect();

        let mut handle = icon::from_svg_bytes(row_cfg.provider.logo_svg());
        handle.symbolic = true;
        let logo = icon::icon(handle).size(20);

        let dropdown = cosmic::widget::dropdown::dropdown(labels, selected, move |option_idx| {
            let kind = options
                .get(option_idx)
                .map(|(kind, _)| *kind)
                .unwrap_or(KINDS[0]);
            Message::ProviderChanged(i, kind)
        });
        // Feste Fußabdrücke: Dropdown und Logo haben sonst inhaltsabhängige
        // Breite — Key-Inputs und Pfeile würden pro Zeile wandern.
        let dropdown = container(dropdown)
            .width(Length::Fixed(DROPDOWN_WIDTH))
            .align_x(Alignment::Start);
        let logo = container(logo).width(Length::Fixed(LOGO_WIDTH));

        let trash_icon = cosmic::widget::icon::icon(cosmic::widget::icon::Handle::from(
            cosmic::widget::icon::from_name("window-close-symbolic"),
        ))
        .size(16)
        .class(cosmic::theme::Svg::custom(move |_| {
            cosmic::iced::widget::svg::Style {
                color: Some(cosmic::iced::Color::from_rgb(0.83, 0.11, 0.11)),
            }
        }));

        let arrow = |name: &str, msg: Message| {
            button::icon(cosmic::widget::icon::Handle::from(
                cosmic::widget::icon::from_name(name),
            ))
            .on_press(msg)
        };
        row![
            logo,
            dropdown,
            text_input::secure_input(fl!("api-key"), &row_cfg.api_key, None, true)
                .on_input(move |v| Message::KeyChanged(i, v))
                .width(Length::Fixed(KEY_WIDTH)),
            container(row![]).width(Length::Fill),
            arrow("go-up-symbolic", Message::MoveRowUp(i)),
            arrow("go-down-symbolic", Message::MoveRowDown(i)),
            button::custom(trash_icon)
                .padding(spacing)
                .class(cosmic::theme::Button::Icon)
                .on_press(Message::RemoveRow(i)),
        ]
        .spacing(spacing)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
    }
}

impl Application for SettingsApp {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "net.d11n.CosmicTokenQuotaSettings";

    fn init(core: Core, _flags: ()) -> (Self, Task<Message>) {
        let cfg: AppConfig = config::load();
        let mut app = Self {
            core,
            rows: cfg.providers.clone(),
            boundary: match cfg.panel_count {
                Some(n) => n.min(cfg.providers.len()),
                None => cfg.providers.len(),
            },
            error: config::config_path().and_then(|path| {
                std::fs::read_to_string(path).ok().and_then(|source| {
                    toml::from_str::<AppConfig>(&source)
                        .err()
                        .map(|error| error.to_string())
                })
            }),
        };
        app.core.set_header_title(fl!("settings-title"));
        let main_id = app.core.main_window_id().unwrap();
        let title_task =
            cosmic::ApplicationExt::set_window_title(&mut app, fl!("settings-title"), main_id);
        (app, title_task)
    }

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ProviderChanged(i, kind) => {
                if let Some(r) = self.rows.get_mut(i) {
                    r.provider = kind;
                }
            }
            Message::KeyChanged(i, v) => {
                if let Some(r) = self.rows.get_mut(i) {
                    r.api_key = v;
                }
            }
            Message::MoveRowUp(i) => {
                if i > 0 {
                    move_row_across(&mut self.rows, &mut self.boundary, i, i - 1);
                }
            }
            Message::MoveRowDown(i) => {
                if i + 1 < self.rows.len() {
                    move_row_across(&mut self.rows, &mut self.boundary, i, i + 1);
                }
            }
            Message::BoundaryUp => {
                if self.boundary > 0 {
                    self.boundary -= 1;
                }
            }
            Message::BoundaryDown => {
                if self.boundary < self.rows.len() {
                    self.boundary += 1;
                }
            }
            Message::AddRow => {
                // Voreinstellung: erster Anbieter, der noch nicht in der
                // Liste steht; gibt es keinen mehr, der erste (Mehrfach-
                // Einträge sind erlaubt).
                let used: Vec<_> = self.rows.iter().map(|r| r.provider).collect();
                let default_kind = KINDS
                    .into_iter()
                    .find(|k| !used.contains(k))
                    .unwrap_or(KINDS[0]);
                self.rows.push(ProviderRow {
                    provider: default_kind,
                    api_key: String::new(),
                });
            }
            Message::RemoveRow(i) => remove_row(&mut self.rows, &mut self.boundary, i),
            Message::Save => {
                let cfg = AppConfig {
                    providers: self
                        .rows
                        .iter()
                        .map(|r| ProviderRow {
                            provider: r.provider,
                            api_key: r.api_key.trim().to_string(),
                        })
                        .collect(),
                    panel_count: (self.boundary != self.rows.len()).then_some(self.boundary),
                };
                if let Err(error) = config::save(&cfg) {
                    self.error = Some(error.to_string());
                    return Task::none();
                }
                return self
                    .core
                    .main_window_id()
                    .map_or_else(Task::none, window::close);
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        self.view_window(self.core.main_window_id().unwrap())
    }

    fn view_window(&self, _id: window::Id) -> Element<'_, Message> {
        let binding = cosmic::theme::active();
        let Spacing {
            space_xxxs,
            space_xxs,
            space_s,
            space_m,
            ..
        } = binding.cosmic().spacing;

        let mut col = column![]
            .padding(space_m)
            .spacing(space_s)
            .width(Length::Fill);
        if let Some(error) = &self.error {
            col = col.push(text::body(fl!("error-line", e = error)));
        }

        // Kopfzeile: Spaltenbeschriftung Anbieter / API-Key.
        col = col.push(
            row![
                text::body(fl!("provider")),
                container(text::body(fl!("api-key"))).width(Length::Fixed(KEY_WIDTH)),
            ]
            .spacing(space_xxxs)
            .align_y(Alignment::Center),
        );

        let arrow = |name: &str, msg: Message| {
            button::icon(cosmic::widget::icon::Handle::from(
                cosmic::widget::icon::from_name(name),
            ))
            .on_press(msg)
        };

        // Zeile mit der Trennlinie: darüber steht, was im Panel erscheint.
        // Immer sichtbar — auch an erster/letzter Position, sonst lässt sie
        // sich nicht zurückholen. Pfeile + Platzhalter (≈ Trash-Breite)
        // spiegeln das Muster der Anbieter-Zeilen.
        // Gleiche Trash-Gesamtbreite wie in den Zeilen (siehe row_view):
        let trash_total = f32::from(space_xxs) * 2.0 + 16.0;
        let boundary_row = || {
            row![
                divider::horizontal::default().width(Length::Fill),
                arrow("go-up-symbolic", Message::BoundaryUp),
                arrow("go-down-symbolic", Message::BoundaryDown),
                container(row![]).width(Length::Fixed(trash_total)),
            ]
            .spacing(space_xxs)
            .align_y(Alignment::Center)
            .width(Length::Fill)
        };

        for i in 0..self.rows.len() {
            if i == self.boundary {
                col = col.push(boundary_row());
            }
            col = col.push(self.row_view(i, f32::from(space_xxs)));
        }
        if !self.rows.is_empty() && self.boundary == self.rows.len() {
            // Linie an letzter Position: hinter allen Zeilen ausgeben.
            col = col.push(boundary_row());
        }

        col = col.push(
            row![
                button::standard(fl!("add-provider")).on_press(Message::AddRow),
                // Speichern rechtsbündig
                container(row![]).width(Length::Fill),
                button::suggested(fl!("save")).on_press(Message::Save),
            ]
            .spacing(space_s)
            .width(Length::Fill),
        );

        container(scrollable(col)).width(Length::Fill).into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        None
    }
}
