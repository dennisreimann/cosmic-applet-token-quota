# AGENTS.md

Guidelines and context for AI agents working on this repository.

## Project

**cosmic-applet-token-quota** — A native COSMIC panel applet
that displays AI coding-plan quota usage for MULTIPLE PROVIDERS in the
panel (provider logo + colored percentage segments). Launch providers:
OpenCode Go and Z.ai Coding Plan. Configuration lives in the applet's own
settings window (separate binary, spawned from the popup).

## Multi-provider architecture

- **Registry:** `src/providers/mod.rs` — `ProviderKind` enum
  (`opencode-go`, `zai-coding-plan`; these strings are the Config- and
  cache-keys), `display()`, `logo_svg()`, `fetch(client, kind, key)`
  dispatch. Adding a provider = enum variant + module + registry arm.
- **Normalized model:** `src/quota.rs` — `ProviderSnapshot { windows:
  Vec<ProviderWindow>, fetched_at_unix, level }`,
  `ProviderWindow { kind: WindowKind, percent_used, reset_unix }`.
  `WindowKind`: FiveHours, Weekly, Monthly, McpMonthly. `percent_used` is
  the CONSUMED share (0–100); color logic (Normal/Warning ≥ 80/Critical
  100) on the ROUNDED value. Panel shows only `in_panel()` kinds (MCP
  excluded); letters: Weekly `W`, Monthly `M`, FiveHours unlabeled.
- **Providers:** `src/providers/opencode.rs` (parser ported from the
  original applet) and `src/providers/zai.rs`. Each offers
  `fetch_quota`/`fetch_quota_at` (the `_at` variant takes a URL for
  httpmock tests).
- **Views:** `src/applet.rs` — pure helpers `provider_panel_segments`,
  `provider_popup_rows`, `configured_fetch_targets`, `merge_fetched`,
  `fmt_reset_in(unix)`, `format_duration`. Panel groups = panel rows WITH
  key only; keyless providers are skipped in the panel and show
  `hint-no-key` in the popup. Refresh: self-rescheduling chain —
  `init` → `Task::batch([fetch_all, sleep(60s)→Tick])` → `Tick` → … (ONE
  sleep per cycle; `Fetched` only updates state). Config changes are
  picked up by a 1 s `stat()` mtime watcher (`wait_for_config_change`,
  HA pattern) — no restart needed. `OpenSettings` spawns the sibling
  binary (`settings_binary_path()`).
- **Settings window:** `src/bin/cosmic-applet-token-quota-settings.rs`
  (ported from cosmic-applet-homeassistant). Each row: provider logo +
  `widget::dropdown` (NOTE: this libcosmic revision has NO `pick_list`!)
  + masked key input; sortable with up/down arrows; a divider line marks
  the panel boundary (rows above it appear in the panel) —
  `move_row_across`/`remove_row` in `src/config.rs` have the exact
  HA boundary semantics (crossing the line switches exactly that row's
  membership). Dropdown options per row = own kind + kinds unused by
  OTHER rows → duplicates impossible, index deterministic. Save writes
  the config; the watcher reloads it within ~1 s and refetches.

## Keys (IMPORTANT — decision 2026-11-25)

API keys live **exclusively** in
`~/.config/cosmic-applet-token-quota/config.toml` (written `0600`).
There is **NO** auth.json fallback and **NO** env fallback — the old
`resolve_api_key` machinery from the fork parent is gone. Keyless
providers render as inactive (panel: skipped; popup: hint).

## Config & cache

- Config: TOML `~/.config/cosmic-applet-token-quota/config.toml` —
  `[[providers]] provider = "opencode-go"|"zai-coding-plan"`, `api_key`,
  plus `panel_count` (absent = all in panel). Default when the file is
  missing: one OpenCode Go row with an empty key (old behavior).
- Cache: `~/.cache/cosmic-applet-token-quota/cache.json` — map
  provider-id → `ProviderSnapshot`; entries validated on load
  (percent 0–100 finite, reset_unix ≥ 0); a broken entry is skipped,
  the rest survive; a broken FILE yields an empty map. Fetch failures
  keep the last snapshot (and freeze the "Updated" clock); `Err` in the
  panel only when that provider never fetched successfully.

## Data sources

- **OpenCode Go:** `GET https://opencode.ai/zen/go/v1/usage` —
  `Authorization: Bearer <key>`, `Accept: application/json`. Root object
  with `usage.rolling|weekly|monthly`, each
  `{ "status": "ok"|"rate-limited", "percent": <consumed 0-100>,
  "resetsAt": RFC3339 }`. **API-`percent` is the CONSUMED share** and maps
  directly to `percent_used`. A missing/malformed window fails the whole
  response (contract error). All three windows are mandatory.
- **Z.ai Coding Plan (GLM) — INOFFICIAL monitor endpoint** (not in the
  docs' openapi.json; verified 2026-11-25 against the live API and
  community projects like `guyinwonder168/opencode-glm-quota`,
  `jukanntenn/glm-plan-usage`):
  `GET https://api.z.ai/api/monitor/usage/quota/limit` with
  `Authorization: <key>` **WITHOUT Bearer prefix** (CN platform
  equivalent: `open.bigmodel.cn`; both variants are one line apart in
  `src/api.rs::AuthStyle` — if the endpoint ever rejects plain auth,
  switch to `AuthStyle::Bearer`). Response:
  `{ code, success, msg, data: { level, limits: [...] } }` with
  - `TOKENS_LIMIT, unit=3, number=5` → 5h window (`percentage` = used,
    `nextResetTime` Unix **milliseconds**),
  - `TOKENS_LIMIT, unit=6` → weekly (has `usage`/`currentValue` too),
  - `TIME_LIMIT, unit=5, number=1` → MCP calls per calendar month
    (`usageDetails[]` per tool; NOT shown in the panel),
  - `data.level` = plan tier ("lite"/"pro"/"max").
  Missing windows are OK (legacy plans lack the weekly window) — the
  parser skips unknown limit types and only fails on `success:false` or a
  recognized window without `percentage`. There is NO monthly token
  window and NO official balance API (billing lives in the web console —
  deliberately not scraped, project principle).
- **HTTP layer:** `src/api.rs` — 10 s timeout, 1 retry on
  408/425/429/5xx, `AuthStyle::{Bearer, Plain}`.

## libcosmic applet patterns (important)

- Dependency: libcosmic from Git pinned to
  `c3ced76094376bcbe331da0d4518fcc6b27006bb` with
  `default-features = false` and explicit features (`"applet"`,
  `"applet-token"`, `"dbus-config"`, `"multi-window"`, `"tokio"`,
  `"wayland"`, `"winit"`). Do not let `cargo update` move the pin.
- `cosmic::app::Task<M>` is `iced::Task<cosmic::Action<M>>` — Application
  methods return `Task<Message>` (NOT `Task<Action<Message>>`).
  `cosmic::Task` (root) is the bare `iced::Task`.
- Selection widget at this revision: `cosmic::widget::dropdown` — NO
  `pick_list`/`combobox`. Plain `dropdown` works in normal windows;
  `popup_dropdown` (surface actions) is the layer-surface fallback.
- Settings windows must be a separate binary (panel applets only get
  layer surfaces); `size_limits` need finite max values on both axes.
- Text colors: use `.class(color)` on text widgets, not iced's
  `color_maybe`.
- Logos are symbolic SVGs (`currentColor`, no hardcoded fills) rendered
  with `icon::from_svg_bytes` + `handle.symbolic = true`.
- **SVG logo canvas MUST be taller than wide** (aspect ≤ 1): the pinned
  iced/tiny_skia vector pipeline scales by the DOMINANT box dimension and
  rasterizes into a pixmap of the box size — a wider-than-tall SVG in the
  square panel slot overflows the pixmap and gets clipped on the right
  (`iced/tiny_skia/src/vector.rs`, `scale_to_height` branch). Keep
  horizontal/vertical padding inside the viewBox so the artwork never
  touches its edges (subpixel clipping).

## Module structure

- `src/providers/mod.rs` — ProviderKind registry + fetch dispatch.
- `src/providers/opencode.rs` — OpenCode Go fetch + parser.
- `src/providers/zai.rs` — Z.ai Coding Plan fetch + parser.
- `src/quota.rs` — normalized snapshot model + validation (custom
  `Deserialize`) + color/letter helpers.
- `src/api.rs` — shared HTTP helper (`AuthStyle`, retry, timeout).
- `src/config.rs` — TOML config (`AppConfig`, `ProviderRow`), boundary
  helpers, `save_to` with `0600`.
- `src/cache.rs` — snapshot map cache (per-entry validation).
- `src/applet.rs` — panel/popup views, fetch chain, watcher, settings
  spawn; pure formatting helpers for tests.
- `src/bin/cosmic-applet-token-quota-settings.rs` — settings window.
- `src/i18n.rs` — Fluent loader (`fl!` macro, catalogs in
  `i18n/<lang>/cosmic_applet_token_quota.ftl`, language from desktop
  locale, `I18N_LANGUAGE` override for tests).

## Testing & building

- Test suite: `cargo test` — integration tests in `tests/` (api, applet,
  cache, cache_validation, config, i18n_de, opencode, zai).
- Translations: add new strings via `fl!("message-id")` in both catalogs
  (`i18n/en`, `i18n/de`) — the `fl!` macro validates against the EN
  catalog at compile time.
- Lint: `cargo clippy --all-targets -- -D warnings`.
- **Before EVERY release (mandatory):** run `cargo update` first (needs
  registry write access!), check for available major bumps of the direct
  dependencies in `Cargo.toml` and apply sensible ones, then run the full
  test suite and clippy again. Never tag a release from a stale
  Cargo.lock.
- **Vendored dbus-settings-bindings crates:** upstream renamed
  `cosmic-settings-daemon` → `cosmic-settings-daemon-zbus` and
  `switcheroo-control` → `switcheroo-control-zbus` on `main`; the pinned
  libcosmic rev still needs the old names. Both crates are vendored in
  `vendor/` and wired up via
  `[patch.'https://github.com/pop-os/dbus-settings-bindings']` (path
  patches). Details in `vendor/*/README.md`.
- Release: `cargo build --release` (10–15 min, iced/libcosmic are heavy).
- Install (user-local): `./install.sh` — binaries → `~/.local/bin`
  (applet + `-settings`), desktop entry →
  `~/.local/share/applications/net.d11n.CosmicTokenQuota.desktop`, icon
  (o.svg as `$APP_ID.svg`) → `~/.local/share/icons/hicolor/scalable/apps/`.
- Panel: add the `Token Quota` applet in Settings → Panel → Applets
  (right wing). Panel hot-reloads applets on config changes.

## Reliability patterns

- Keep-last-state semantics per provider: `Message::Fetched(Ok)` replaces
  that provider's snapshot and clears its error; `Fetched(Err)` records
  the error only — the last snapshot stays, cache writes still happen.
  The panel shows `Err` only when there has never been a successful
  fetch. Do not "fix" this by clearing state on errors.
- Cache gives an instant panel at startup; the first fetch replaces it.
- The 5h popup row hides its reset countdown while the displayed value
  is 0 % (the countdown runs anyway and would just confuse).
