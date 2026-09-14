---
name: disk-usage
description: Scan local disks with DataTree, query size and allocated bytes, search with *.iso / <100m / a>=1g, and export CSV. Use when the user asks what is using disk space, which files are largest, or to inspect a folder tree.
---

# DataTree disk usage

DataTree is a local disk usage analyzer. Prefer MCP tools from the `datatree` server (binary `datatree-mcp`). Do not dump an entire tree as JSON.

## Setup

- `datatree-mcp` must be on PATH (or next to `datatree.exe`).
- Scans stay on the machine. Tool results (paths and names) enter the chat context of the host AI.

## Tools

1. `list_volumes` — mounted disks.
2. `scan` — directory summary only (`engine`, `files`, `dirs`, `size`, `allocated`).
3. `scan_status` / `cancel_scan` — progress.
4. `children` — paginated (`limit` default 50, max 200). `sort`: `allocated` (default), `size`, `name`.
5. `search` — same filter language as the GUI.
6. `top` — largest N files and/or directories (never the full tree).
7. `path_info`, `summary`, `extensions`.
8. `load_snapshot` / `save_snapshot` — `.edst` / `.edst.zst`.
9. `export_csv`.
10. `open_gui` — open the human GUI on a path or the current snapshot.

CLI fallback (same JSON):

```text
datatree-mcp scan C:\Users --json
datatree-mcp search "a>=1g *.iso" --path C:\Users --json --limit 50
datatree-mcp top --path C:\Users --files --n 20 --json
```

## Filter syntax

| Token | Meaning |
|---|---|
| `report` | Name contains `report` (case-insensitive) |
| `*.iso` | Extension is `iso` |
| `<100m` | Logical size under 100 MiB |
| `>=1g` | Logical size at least 1 GiB |
| `a>=1g` | Allocated size at least 1 GiB |

Suffixes: `k`, `m`, `g`, `t` (1024-based). Combine tokens: `*.iso <100m backup`.

## Engine

- Windows NTFS as Administrator: `MFT` (direct `$MFT` read).
- Otherwise: `Walk` (parallel directory walk).
- Always report which engine was used. Do not promise MFT without elevation.

## Allocated vs size

- `size` is logical file size.
- `allocated` is on-disk cluster allocation. Prefer allocated when talking about "disk used".

## Rules

- Page results. Never enumerate every node.
- Paths may include `\\?\` prefixes; treat cleaned paths as equivalent.
- For visualization, call `open_gui` instead of drawing a treemap in chat.
- Deletions are a different skill (`disk-cleanup`). Do not delete from this skill.
