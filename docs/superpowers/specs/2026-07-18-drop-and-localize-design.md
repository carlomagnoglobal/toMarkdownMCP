# Drag & Drop into Notes + Link Localization — Design

**Date:** 2026-07-18
**Component:** `gui/` (toMarkdown Viewer, Tauri 2)
**Goal:** (1) Drop images and files from the OS or other apps directly into an open note, choosing per drop whether to store the file or convert it to markdown. (2) For links already present in a note, let the user store the target in the vault or convert it to a markdown note ("localization").

## Language policy

Rust-first (project-wide rule): all file I/O, downloading, conversion, and note rewriting happen in Rust commands with unit tests. JavaScript in `gui/ui/index.html` is thin glue: drop routing, dialogs, and invoking commands. No third-party JS libraries, no npm.

## Constraints & platform reality

- Tauri intercepts OS *file* drags: the webview never sees file paths in DOM events. File drops arrive via the existing `tauri://drag-drop` listener, whose payload includes drop `position`. Per-element targeting is done by hit-testing that position against the note area.
- Non-file drags (image data or URLs dragged from apps like Safari) DO reach the DOM as `drop` events on the textareas/reader; a DOM handler covers those.
- No blocking dialogs; all UI uses the existing overlay/toast patterns.

## Feature 1 — Drop into note

Applies when a note is open and the drop lands on the note area (reader, split editor, or live editor). Drops elsewhere (sidebar, no note open) keep today's window-level behavior (open folder/file or import) unchanged.

### Input kinds and routing

| Dropped thing | Behavior |
|---|---|
| Image file (png/jpg/jpeg/gif/webp/svg/bmp) | No dialog. Copied to the vault attachment folder (same one `paste_image` uses, deduped name) and embedded at the insertion point. |
| Raw image data / browser file promise (webpage image dragged from Safari/Chrome; delivered as a DOM File item, often with a companion URL) | Two-action dialog (revised 2026-07-18 at user request): **Save into vault & embed** (bytes via `paste_image`) or **Link only** (`![](url)`, enabled only when a URL accompanies the data). Esc cancels. |
| Image URL (`text/uri-list` with image extension, no file data) | Same two-action dialog; Save downloads via the Rust command and embeds; failed downloads fall back to `![](url)` with a toast. |
| Any other file | Four-action dialog (below). |
| Non-image URL | Four-action dialog; its convert actions use the existing URL→markdown converter. |

### The four-action dialog

Non-blocking overlay in the existing dialog style; Esc or outside-click cancels (nothing inserted). Actions:

1. **Copy into vault & link** — file copied to the vault's configured attachment folder (same resolution as `paste_image`; created on demand, deduped filename); embed/link inserted.
2. **Link original location** — no copy; `[name](file:///…)` (or the URL itself for URL drops) inserted.
3. **Convert → new linked note** — existing `save_import` conversion flow into `Imports/`; a wikilink to the new note is inserted at the drop point (the note is NOT opened in a new tab, unlike the window-level import).
4. **Convert inline** — converted markdown inserted at the insertion point. If the converted text exceeds 200 KB, a confirmation step warns before inserting.

Convert actions (3, 4) are disabled with a tooltip when `is_convertible` says the file type is unsupported. Multiple files dropped at once: the dialog shows the file count and the chosen action applies to all; per-file failures toast and skip.

### Insertion point

- Split or live editing active: at the caret of the focused textarea (live mode: into the active block; if no block is being edited, a new block at the end).
- Reader mode: appended at the end of the note, with a toast naming where it went.
- Insertions dispatch the editors' `input` event so highlighting, autosave, and re-render react normally.

## Feature 2 — Link localization (existing notes)

External targets only: `http(s)://` links and images, and `file://` links. Links inside the vault are never touched.

### Per-link

Right-click on an external link or remote image in the reader (plus a hover affordance on remote images) opens a small context menu:

- **Store in vault** — target downloaded (URL) or copied (file path); saved to the attachment folder; the link in the note source is rewritten to the local copy.
- **Convert to markdown note** — target run through the converter (URL or file), saved as a note in `Imports/`, and the link rewritten to a wikilink.

### Whole-note

Palette command **"Localize external links…"**: Rust scans the current note and returns the list of external targets. A dialog lists them with a per-item action selector — defaulting to *Store* for images and *Skip* for the rest — plus Apply/Cancel. Processing is sequential with progress toasts; a failing item (dead URL, offline) is skipped with a warning toast and its link left untouched.

### Rewriting

`localize_link` runs in Rust: it performs the store/convert once, rewrites every identical occurrence of that exact link target in the note source (they reference the same resource), saves the note, and returns the updated source. The frontend refreshes from the returned source (and the file watcher covers external views). Matching is on the exact target string, so similar-but-different links are never touched.

## New Rust commands (all unit-tested)

- `store_attachment(root, src_path) -> String` — copy a local file into the vault attachment folder, deduped; returns vault-relative path.
- `store_url_attachment(root, url) -> String` — download (reusing the pipeline's HTTP machinery from `convert_url_to_markdown`) and store; returns vault-relative path. Errors are strings for toast display.
- `scan_external_links(root, note_path) -> Vec<{link, kind}>` — external targets in a note (`kind`: image_url, url, file).
- `localize_link(root, note_path, link, action) -> String` — store-or-convert one target, rewrite that link, save, return new source.

Existing commands reused: `paste_image`, `is_convertible`, `convert_file_to_markdown`, `convert_url_to_markdown`, `save_import`, `save_file`.

## Error handling

- All command failures surface as toasts (existing `friendlyError` mapping); no blocking dialogs.
- Cancelled dialogs insert/rewrite nothing.
- Localization never leaves a note half-rewritten: each link rewrite is a single Rust read-modify-save.

## Testing

- Rust unit tests per command: dedup naming, missing source file, non-vault root rejection, scan extraction (fixture note with mixed local/external links), localize rewrite precision — when the same external link appears multiple times, ALL identical occurrences are rewritten to the same local target in one operation (they point at the same resource, so one store serves them all), download-failure error paths (mock/local server or error-path-only).
- Manual checklist additions (live GUI required): drop file from Finder onto note vs sidebar, drag image from Safari (URL case), drag image data, multi-file drop, Esc cancel, per-link context menu, whole-note localize with a dead URL in the list.

## Out of scope

- Localizing links across the whole vault at once (per-note only for now).
- Re-uploading/exporting attachments back out of the vault.
- Any change to window-level drop behavior for folders/files when no note is targeted.
