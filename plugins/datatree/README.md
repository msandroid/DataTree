# DataTree agent plugin

Package MCP + skills so Cursor, Claude Code, Codex, and Antigravity can drive DataTree.

Requires `datatree-mcp` on PATH (`cargo build --release -p edirstat-mcp`).

## Install

From a DataTree checkout:

```powershell
.\target\release\datatree-mcp.exe install --all
```

Flags: `--cursor`, `--claude`, `--codex`, `--antigravity`, `--global`.

Or copy this folder:

| Client | Location |
|---|---|
| Cursor | workspace plugin / `.cursor/mcp.json` |
| Claude Code | plugin marketplace or `.mcp.json` |
| Codex | `~/.codex/config.toml` `[mcp_servers.datatree]` |
| Antigravity | `.agents/plugins/datatree/` (plus `mcp_config.json`) or `~/.gemini/config/mcp_config.json` |

## Layout

- `plugin.json` — Agent Plugins 1.0 + extra client fields
- `mcp.json` — stdio server `datatree-mcp mcp`
- `skills/` — `disk-usage`, `disk-cleanup`
- `.cursor-plugin/`, `.claude-plugin/`, `.codex-plugin/`, `.agy-plugin/` — native manifests
