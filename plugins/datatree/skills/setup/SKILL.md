---
name: setup
description: Install datatree-mcp on PATH and verify DataTree MCP tools. Use when scan/search tools are missing, the server fails to start, or the user just installed the plugin.
---

# DataTree MCP setup

The plugin talks to a local stdio server. File contents are never read. Paths and names in tool results go to the host AI context.

## Install the binary

```powershell
cargo build --release -p edirstat-mcp
```

Put `target/release/datatree-mcp` (`.exe` on Windows) on PATH, or keep it next to `datatree.exe`. The GUI command `datatree mcp` launches the sibling helper.

Windows installer also copies `datatree-mcp.exe` next to the GUI.

## Connect the client

```powershell
datatree-mcp install --all
```

Or register stdio MCP: command `datatree-mcp`, args `["mcp"]`.

## Check

Call `list_volumes`. If that fails, the binary is not on PATH. Do not dump the filesystem tree as JSON; use `scan` then paged `children` / `search` / `top`.
