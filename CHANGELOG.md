# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The **Unreleased** section accumulates changes as they land; its entries are
used verbatim as release notes when cutting a new release (the section is
then renamed to the new version and emptied).

## 1.0.0

### Added

- Multi-provider quota display: OpenCode Go and Z.ai Coding Plan side by
  side in the panel (logo + colored segments per provider).
- Settings window (own binary, spawned from the popup): provider
  dropdown, masked API-key input per row, sortable list with a divider
  line — rows above the line appear in the panel.
- API keys stored exclusively in the applet config (TOML, `0600`).
- Z.ai provider reads the (unofficial) monitor endpoint
  `api.z.ai/api/monitor/usage/quota/limit` — 5h/weekly token quotas plus
  the monthly MCP tool-call quota (popup only).
