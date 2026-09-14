# DataTree VS Code / Cursor extension

Lightweight IDE bridge. It does not reimplement the treemap GUI.

## What it does

- Registers the `datatree-mcp` stdio MCP server when the host supports MCP providers
- Commands: **Scan Folder**, **Open in GUI**, **Show Last Summary**
- Status bar shows engine and allocated size after a scan

## Requirements

`datatree-mcp` and `datatree` on PATH, or set `datatree.mcpPath` / `datatree.guiPath`.

```powershell
cargo build --release -p edirstat -p edirstat-mcp
```

## Install (local)

Copy this folder into the editor extensions directory and reload:

```powershell
# VS Code
Copy-Item -Recurse extensions\datatree "$env:USERPROFILE\.vscode\extensions\msandroid.datatree-2.2.0"

# Cursor
Copy-Item -Recurse extensions\datatree "$env:USERPROFILE\.cursor\extensions\msandroid.datatree-2.2.0"
```
