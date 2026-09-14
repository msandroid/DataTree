---
name: disk-cleanup
description: Prepare and execute DataTree file deletions with a mandatory two-step confirm. Use when the user wants to trash or permanently delete files found by a disk scan.
---

# DataTree cleanup

Destructive MCP tools must never run in one shot. Show the plan to the human, wait, then confirm.

## Protocol

1. `scan` (or reuse the current snapshot).
2. `search` / `top` / `children` to pick candidates.
3. `plan_delete` with `paths` and `mode`:
   - `trash` (default) — Recycle Bin / OS trash.
   - `permanent` — only if `DATATREE_ALLOW_PERMANENT_DELETE=1` is set in the MCP server environment.
4. Show the returned item list, totals, `confirm_phrase`, and `expires_in_secs`.
5. After the user agrees, call `confirm_delete` with the exact `plan_id`, `confirm_token`, and:
   - `TRASH` for trash
   - `DELETE PERMANENTLY` for permanent
6. Report deleted vs failed (permission denied is called out).

Plans expire after 10 minutes. A wrong token does nothing.

## Hard refusals

- Volume roots (`C:\`, `/`).
- The scan root itself.
- More than 1000 paths in one plan.
- Permanent mode when the env var is unset.
- System directories (`C:\Windows`, `/usr`, `/System`) unless the user names that exact path and confirms in chat.

## Do not

- Call `confirm_delete` in the same turn as `plan_delete` unless the user already pasted the token and phrase.
- Expand a glob into a delete plan without listing the matches first.
- Read file contents. Metadata only.
