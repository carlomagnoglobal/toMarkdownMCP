# Obsidian Vault Tools

Eleven `obsidian_*` tools give the server first-class understanding of Obsidian
vaults: real wikilink parsing (`[[target|alias]]`, `[[note#heading]]`,
`[[note#^block]]`, `![[embeds]]`), YAML frontmatter with `aliases`/`tags`,
Obsidian's shortest-path link resolution, backlinks, tasks, callouts,
`.canvas` files, Dataview inline fields, `.obsidian` config, templates, and
link-preserving renames. All are read-only except `obsidian_rename_note`
(dry-run by default) and `obsidian_create_note_from_template`.

The vault index is cached per vault and automatically invalidated whenever any
file's path, mtime, or size changes — safe to use while Obsidian is open.

## Tools

| Tool | Purpose |
|------|---------|
| `obsidian_vault_index` | Vault summary: note/attachment counts, tag frequencies, aliases, broken and ambiguous links, orphans (`include_orphans: true`) |
| `obsidian_get_note` | One note by path/stem/alias: parsed frontmatter, tags (frontmatter + inline), headings, outgoing links, backlink count, content. `inline_embeds: true` transcludes `![[embeds]]` (depth `max_embed_depth`, default 3) |
| `obsidian_resolve_link` | Resolve any link string using shortest-path rules; reports `Resolved`/`Ambiguous`/`Broken`, heading existence, block line. `from_note` enables same-folder disambiguation |
| `obsidian_get_backlinks` | All inbound links — including alias/heading/block/embed forms — with source line context |
| `obsidian_search` | `mode`: `tag` (nested prefix: `status` matches `status/active`), `alias`, `field` (`key` or `key=value` in frontmatter), `text` |
| `obsidian_list_tasks` | Checkbox tasks in vault or note; states `[ ]`/`[x]`/`[/]`/`[-]`/`[>]` etc., nesting, #tags, Tasks-plugin dates (📅 ✅ ⏳ 🛫 🔁). Filter with `status` |
| `obsidian_get_vault_config` | `.obsidian/` settings: attachment folder, new-link format, daily-notes folder/format/template, templates folder, core plugins |
| `obsidian_create_note_from_template` | New note from a template with `{{title}}`, `{{date}}`, `{{time}}`, `{{date:FORMAT}}`; `daily: true` uses daily-notes settings. Never overwrites |
| `obsidian_rename_note` | Rename/move a note and rewrite every inbound wikilink preserving `|alias`, `#heading`, `#^block`. **`dry_run` defaults to true** |
| `obsidian_convert_canvas` | `.canvas` (JsonCanvas) → structured Markdown: groups with members, nodes in reading order, edge list |
| `obsidian_extract_dataview_fields` | Dataview inline fields (`key:: value`, `[key:: value]`, `(key:: value)`) plus frontmatter properties, optionally filtered by `field` |

Every tool takes `vault_path` (vault root); notes are addressable by
vault-relative path (`folder/Note.md`), filename stem (`Note`), or frontmatter
alias.

## Example

```json
{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{
  "name": "obsidian_rename_note",
  "arguments": {"vault_path": "/path/to/vault", "note": "Old Name", "new_name": "New Name", "dry_run": true}
}}
```

Returns the planned `link_rewrites` per file; run again with `"dry_run": false` to apply.

## TUI viewer

The same binary ships an interactive terminal viewer:

```bash
to_markdown_mcp tui /path/to/vault    # or a single .md file
```

Press `?` inside the app for the full in-app key reference (a popup covering
every binding below). Summary:

- **Navigation**: `↑/↓` or `j/k` move · `h`/`l` back / open·follow-link ·
  `Space` or `Ctrl+f` page down · `Ctrl+b` page up · `Ctrl+d`/`Ctrl+u`
  half-page · `g`/`G` or `Home`/`End` jump to top/bottom · `Tab`/`Shift+Tab`
  switch pane
- **Search**: `/` search — filters the file list in the tree pane, or
  searches the open note's text in the content pane · `n`/`N` jump to the
  next/previous match · `Enter` confirm · `Esc` cancel (fully reverts)
- **Notes**: `Enter` open file / follow the `[[wikilink]]` on the cursor
  line · `Backspace` or `Esc` go back · `r` toggle raw source vs. formatted
  view (persists as you move between notes)
- **View**: `z` zen mode (hide the file tree, distraction-free) · `T` cycle
  dark/light theme · **mouse**: wheel scrolls either pane, click selects and
  opens a file, click a content line to move the cursor there, click it
  again to follow its wikilink
- **General**: `?` toggle help · `q` or `Ctrl+c` quit

Rendering is fully typographic: headings lose their `#` and gain colored
underlines (ATX and setext forms), frontmatter shows as a boxed dimmed
"Properties" panel, pipe tables are re-aligned into real columns with box
borders, fenced code blocks get labeled rules, a gutter, and **real syntax
highlighting** (syntect — per-token colors for rust/js/python/…), inline
code drops its backticks onto a shaded chip, `[text](url)` links show just
the styled text, bullets become `•` with hanging indents on wrapped lines,
`~~strike~~`/`__bold__`/`_italic_` all render, blockquotes get a `▌` bar,
callouts (`> [!warning] ...`) render as colored icon-labeled boxes, and
checkboxes show `☐`/`☑` glyphs colored by state. Search matches are
highlighted in-line (all occurrences). Long panes get a scrollbar; wide
terminals cap text at 100 columns and center it; the title bar shows a
breadcrumb of the nearest heading plus tags and backlink count; the status
bar shows position, word count, and reading time. Image references —
`![[photo.png]]` and `![alt](cat.jpg)` — show as a `🖼` placeholder (no
image rendering is attempted). If the open file changes on disk (e.g.
edited in Obsidian while the viewer is open), it's reloaded automatically on
the next tick.

## Implementation

`src/obsidian/`: `wikilink.rs` (grammar + code-fence awareness), `vault.rs`
(walker, `VaultIndex`, shortest-path resolution, mtime-fingerprint cache),
`frontmatter.rs` (serde_yaml), `tasks.rs`, `callout.rs`, `canvas.rs`,
`dataview.rs`, `config.rs`, `template.rs`. TUI in `src/tui/`. Fixture vault
for tests: `tests/fixtures/mini_vault/`.
