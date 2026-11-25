# cosmic-applet-token-quota

A native **COSMIC panel applet** that shows AI coding-plan quota usage right
in the panel — for multiple providers side by side:

```
[o] 16% · 85% W · 38% M   [z] 5% · 24% W
```

Each provider contributes its logo plus colored percentage segments
(consumed share, counting up; normal < 80 %, warning ≥ 80 %, critical at
100 %). The popup lists every configured provider with relative reset
times; a settings window (own binary, opened from the popup) manages the
provider list and API keys.

Supported providers at launch:

- **OpenCode Go** — `16% · 85% W · 38% M` (5h / week / month)
- **Z.ai Coding Plan (GLM)** — `5% · 24% W` (5h / week; the monthly MCP
  tool-call quota shows in the popup)

## Providers & API keys

Keys are stored **exclusively** in the applet's own config
(`~/.config/cosmic-applet-token-quota/config.toml`, owner-only `0600`).
Open the applet popup → **Settings**, add a provider from the dropdown and
paste the key:

| Provider | Where to get the key |
| --- | --- |
| OpenCode Go | OpenCode console (`opencode.ai`) → API key |
| Z.ai Coding Plan | z.ai → **API Keys → Personal Coding Plan** (keys for the standard API are billed separately!) |

Providers without a key are skipped in the panel and show a hint in the
popup. The list is sortable; providers **above the divider line** appear in
the panel, the rest only in the popup.

## Install (user-local)

Download the release tarball (GitHub Releases) and:

```bash
tar -xzf cosmic-applet-token-quota-linux-x86_64.tar.gz
./install.sh
```

Then add the *Token Quota* applet to a panel wing in
**Settings → Panel → Applets** and enter your keys in the applet's
Settings window.

## Build (from source)

Requires Rust plus system packages (`libwayland-dev`, `libxkbcommon-dev`, …).

```bash
cargo build --release   # 10–15 min, iced/libcosmic are heavy
./install.sh            # installs from target/release/
```

## Tests

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```
