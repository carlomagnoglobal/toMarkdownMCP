# Drag & Drop into Notes + Link Localization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drop images/files/URLs from the OS or other apps directly into an open note (store or convert, chosen per drop), and localize external links already present in notes (store target in vault or convert to a linked note).

**Architecture:** Four new Rust commands (`store_attachment`, `store_url_attachment`, `scan_external_links`, `localize_link`) in `gui/src/main.rs`, TDD'd like the Phase 2 commands; one small `pub` byte-fetch helper added to the core crate's `sources` module (reuses its existing HTTP dependency — no new deps). Frontend glue in `gui/ui/index.html`: hit-testing the existing `tauri://drag-drop` event position, a DOM `drop` handler for non-file drags, a four-action overlay dialog, and context-menu/palette entries reusing the existing `ctxMenu` and `COMMANDS` machinery.

**Tech Stack:** Rust (std fs, existing to_markdown_mcp converters/sources, chrono_stamp/dedup patterns), Tauri 2, hand-written vanilla JS glue.

## Global Constraints

- **Rust-first:** all file I/O, downloading, conversion, and note rewriting in Rust. JS is routing/dialog glue only. No third-party JS, no npm.
- **No new crate dependencies.** The URL download reuses whatever HTTP client `to_markdown_mcp::sources::fetch_from_source` already uses (inspect it; add a `pub` bytes variant beside it).
- New commands registered in the `invoke_handler` list; unit tests in the same file's `#[cfg(test)]` module; TDD (failing test first).
- Gates after every task: `cargo test -p to_markdown_gui` green, `cargo clippy -p to_markdown_gui --all-targets` clean. Tasks touching the core crate also run `cargo test --lib` at the workspace root.
- No blocking dialogs; failures toast via existing `friendlyError`/`toast`.
- Window-level drop behavior for folders (and files when no note is open / drop is outside the note area) is UNCHANGED.
- MSRV 1.88 untouched.

## Existing code you will build on (anchors by identifier, not line number)

- `gui/src/main.rs`: `paste_image` (attachment-folder resolution via `obsidian::config::read_config(...).attachment_folder`, `chrono_stamp()`, `vault::invalidate`); `save_import` (Imports/ dedup loop); `convert_file_to_markdown` / `convert_url_to_markdown` → `Converted { markdown, suggested_name, source }`; `is_convertible`; `fixture_vault()` test helper; `invoke_handler` list.
- `src/sources.rs` (core crate): `fetch_from_source`, `SourceType` — the HTTP machinery to extend with a bytes fetch.
- `gui/ui/index.html`: `tauri://drag-drop` listener (payload has `paths` and `position {x, y}`); `ctxMenu` + `showCtx(x, y, items)` context-menu helpers (search `const ctxMenu`); `COMMANDS` palette array; `toast`, `friendlyError`, `escapeHtml`, `trapFocus`/`releaseFocus`; `currentFile`, `currentFolder`, `viewMode`; editors: `#editor` textarea (split), live-block textarea `textarea.block-edit`, reader `#content` inside `#scroll`; `saveNow(silent)`; every programmatic editor mutation dispatches `new Event('input')`.

---

### Task 1: Rust `store_attachment` + shared attachment-dir helper

**Files:**
- Modify: `gui/src/main.rs`

**Interfaces:**
- Produces: `fn attachment_dir(root: &Path) -> Result<PathBuf, String>` (extracted from `paste_image`, reused by Task 2); command `store_attachment(root: String, src_path: String) -> Result<StoredAttachment, String>` where `StoredAttachment { name: String, embed: String }` (`embed` is `![[name]]` for images, `[[name]]` for other files).

- [ ] **Step 1: Failing tests** (in the existing `#[cfg(test)]` module; use `tempfile`-free std patterns like the existing tests — check how `fixture_vault()` builds temp dirs and mirror it):

```rust
#[test]
fn store_attachment_copies_and_dedupes() {
    let root = fixture_vault();
    let src = root.path().join("src img.png");
    std::fs::write(&src, b"fakepng").unwrap();
    let a = store_attachment_impl(root.path(), &src).unwrap();
    assert!(a.embed.starts_with("![["), "png should embed: {}", a.embed);
    let b = store_attachment_impl(root.path(), &src).unwrap();
    assert_ne!(a.name, b.name, "second copy must dedupe filename");
    // both files exist in the attachment dir
    let dir = attachment_dir(root.path()).unwrap();
    assert!(dir.join(&a.name).exists() && dir.join(&b.name).exists());
}

#[test]
fn store_attachment_non_image_links_instead_of_embeds() {
    let root = fixture_vault();
    let src = root.path().join("doc.pdf");
    std::fs::write(&src, b"%PDF").unwrap();
    let a = store_attachment_impl(root.path(), &src).unwrap();
    assert!(a.embed.starts_with("[[") && !a.embed.starts_with("![["));
}

#[test]
fn store_attachment_missing_source_errors() {
    let root = fixture_vault();
    assert!(store_attachment_impl(root.path(), Path::new("/nope/x.png")).is_err());
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p to_markdown_gui store_attachment` → FAIL: not found.

- [ ] **Step 3: Implement** — extract the dir logic from `paste_image` into `attachment_dir(root)` and reroute `paste_image` through it; then:

```rust
#[derive(serde::Serialize, Debug)]
struct StoredAttachment { name: String, embed: String }

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];

fn store_attachment_impl(root: &Path, src: &Path) -> Result<StoredAttachment, String> {
    if !src.is_file() { return Err(format!("File not found: {}", src.display())); }
    let dir = attachment_dir(root)?;
    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
    let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mut name = if ext.is_empty() { stem.clone() } else { format!("{}.{}", stem, ext) };
    let mut n = 2;
    while dir.join(&name).exists() {
        name = if ext.is_empty() { format!("{} {}", stem, n) } else { format!("{} {}.{}", stem, n, ext) };
        n += 1;
    }
    std::fs::copy(src, dir.join(&name)).map_err(|e| e.to_string())?;
    vault::invalidate(root);
    let embed = if IMAGE_EXTS.contains(&ext.as_str()) { format!("![[{}]]", name) } else { format!("[[{}]]", name) };
    Ok(StoredAttachment { name, embed })
}

#[tauri::command]
fn store_attachment(root: String, src_path: String) -> Result<StoredAttachment, String> {
    store_attachment_impl(Path::new(&root), Path::new(&src_path))
}
```

Register in `invoke_handler`. `attachment_dir` is `paste_image`'s existing folder-resolution block verbatim, returning the created dir.

- [ ] **Step 4: Run tests** — targeted, then full `cargo test -p to_markdown_gui` + clippy → green/clean (confirm `paste_image`'s behavior unchanged by the extraction — its tests, if any, plus manual read).

- [ ] **Step 5: Commit**

```bash
git add gui/src/main.rs
git commit -m "GUI: store_attachment command (copy file into vault, deduped)"
```

### Task 2: Byte fetch in core crate + Rust `store_url_attachment`

**Files:**
- Modify: `src/sources.rs` (core crate — pub bytes fetch helper + test)
- Modify: `gui/src/main.rs` (command + tests)

**Interfaces:**
- Produces: core `pub async fn fetch_url_bytes(url: &str) -> Result<(Vec<u8>, Option<String>), Error>` (bytes + content-type header, same error type as `fetch_from_source`); command `store_url_attachment(root: String, url: String) -> Result<StoredAttachment, String>`.

- [ ] **Step 1: Inspect** `fetch_from_source` in `src/sources.rs` — identify the HTTP client and error type it uses. `fetch_url_bytes` is that same request path minus the UTF-8/HTML decoding, returning raw bytes and the `content-type` header value.

- [ ] **Step 2: Failing tests** — core crate (in `src/sources.rs` tests): invalid-URL error path only (no network in tests):

```rust
#[tokio::test]
async fn fetch_url_bytes_rejects_invalid_url() {
    assert!(fetch_url_bytes("not-a-url").await.is_err());
}
```

(Match the existing async-test attribute style in this file — if existing tests use a different runtime macro, use that.) In `gui/src/main.rs`, test the pure naming logic:

```rust
#[test]
fn url_attachment_name_derivation() {
    assert_eq!(url_attachment_name("https://x.com/a/photo.png?w=2", None), "photo.png");
    assert_eq!(url_attachment_name("https://x.com/img", Some("image/jpeg")), "img.jpg");
    let n = url_attachment_name("https://x.com/", None);
    assert!(!n.is_empty() && !n.contains('/'));
}
```

- [ ] **Step 3: Run, verify fail** — both targeted test names FAIL (functions not found).

- [ ] **Step 4: Implement** — core helper per Step 1's findings; in `gui/src/main.rs`:

```rust
fn url_attachment_name(url: &str, content_type: Option<&str>) -> String {
    let path_part = url.split('?').next().unwrap_or(url).split('#').next().unwrap_or(url);
    let last = path_part.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    let mut name = if last.is_empty() || last.contains("://") { format!("Downloaded {}", chrono_stamp()) } else { last.to_string() };
    if !name.contains('.') {
        let ext = match content_type.map(|c| c.split(';').next().unwrap_or(c).trim()) {
            Some("image/jpeg") => "jpg", Some("image/png") => "png", Some("image/gif") => "gif",
            Some("image/webp") => "webp", Some("image/svg+xml") => "svg", Some("application/pdf") => "pdf",
            _ => "bin",
        };
        name = format!("{}.{}", name, ext);
    }
    name
}

#[tauri::command]
async fn store_url_attachment(root: String, url: String) -> Result<StoredAttachment, String> {
    let (bytes, ctype) = to_markdown_mcp::sources::fetch_url_bytes(&url).await.map_err(|e| e.to_string())?;
    let name0 = url_attachment_name(&url, ctype.as_deref());
    let rootp = Path::new(&root);
    let dir = attachment_dir(rootp)?;
    let (stem, ext) = match name0.rsplit_once('.') { Some((s, e)) => (s.to_string(), e.to_string()), None => (name0.clone(), String::new()) };
    let mut name = name0;
    let mut n = 2;
    while dir.join(&name).exists() {
        name = if ext.is_empty() { format!("{} {}", stem, n) } else { format!("{} {}.{}", stem, n, ext) };
        n += 1;
    }
    std::fs::write(dir.join(&name), bytes).map_err(|e| e.to_string())?;
    vault::invalidate(rootp);
    let e = name.rsplit('.').next().unwrap_or("").to_lowercase();
    let embed = if IMAGE_EXTS.contains(&e.as_str()) { format!("![[{}]]", name) } else { format!("[[{}]]", name) };
    Ok(StoredAttachment { name, embed })
}
```

Register in `invoke_handler`. (The dedup loop duplicates Task 1's — extract a `fn dedupe_in(dir: &Path, stem: &str, ext: &str) -> String` helper and use it in both.)

- [ ] **Step 5: Run tests** — `cargo test --lib` at workspace root (core) + `cargo test -p to_markdown_gui` + clippy → green/clean.

- [ ] **Step 6: Commit**

```bash
git add src/sources.rs gui/src/main.rs
git commit -m "GUI: store_url_attachment command; core fetch_url_bytes helper"
```

### Task 3: Rust `scan_external_links` + `localize_link`

**Files:**
- Modify: `gui/src/main.rs`

**Interfaces:**
- Consumes: `store_attachment_impl`, `store_url_attachment` internals, `convert_file_to_markdown` / `convert_url_to_markdown`, `save_import`'s Imports dedup (extract `fn save_markdown_to_imports(root: &Path, suggested_name: &str, markdown: &str) -> Result<PathBuf, String>` from `save_import` and reroute `save_import` through it).
- Produces: command `scan_external_links(root, note_path) -> Vec<ExtLink>` with `ExtLink { link: String, kind: String }` (`kind`: `image_url` | `url` | `file`); command `localize_link(root, note_path, link, action) -> Result<String, String>` (`action`: `store` | `convert`; returns the full updated note source).

- [ ] **Step 1: Failing tests:**

```rust
#[test]
fn scan_external_links_finds_targets() {
    let root = fixture_vault();
    let note = root.path().join("Ext.md");
    std::fs::write(&note, "![a](https://x.com/p.png)\n[b](https://x.com/page)\n[c](file:///tmp/d.pdf)\n[[Note A]]\n![local](local.png)\n[dup](https://x.com/page)\n").unwrap();
    let links = scan_external_links_impl(root.path(), &note).unwrap();
    let kinds: Vec<(&str, &str)> = links.iter().map(|l| (l.link.as_str(), l.kind.as_str())).collect();
    assert!(kinds.contains(&("https://x.com/p.png", "image_url")));
    assert!(kinds.contains(&("https://x.com/page", "url")));
    assert!(kinds.contains(&("file:///tmp/d.pdf", "file")));
    assert_eq!(links.iter().filter(|l| l.link == "https://x.com/page").count(), 1, "dedup identical targets");
    assert!(!kinds.iter().any(|(l, _)| l.contains("Note A") || l.contains("local.png")), "vault-internal links excluded");
}

#[test]
fn localize_link_store_rewrites_all_occurrences_of_file_url() {
    let root = fixture_vault();
    let src = root.path().join("att.pdf");
    std::fs::write(&src, b"%PDF").unwrap();
    let file_url = format!("file://{}", src.display());
    let note = root.path().join("Loc.md");
    std::fs::write(&note, format!("[x]({0})\ntext\n[y]({0})\n", file_url)).unwrap();
    let updated = localize_link_impl(root.path(), &note, &file_url, "store").unwrap();
    assert!(!updated.contains(&file_url), "no occurrence of the old target remains");
    assert_eq!(updated.matches("[[att").count(), 2, "both occurrences rewritten to the same local target");
    assert_eq!(std::fs::read_to_string(&note).unwrap(), updated, "note saved");
}
```

(URL-store and convert actions hit the network; their logic shares the same rewrite path exercised here — do not write network tests.)

- [ ] **Step 2: Run, verify fail.**

- [ ] **Step 3: Implement:**

```rust
#[derive(serde::Serialize, Debug)]
struct ExtLink { link: String, kind: String }

fn scan_external_links_impl(_root: &Path, note: &Path) -> Result<Vec<ExtLink>, String> {
    let src = std::fs::read_to_string(note).map_err(|e| e.to_string())?;
    let mut out: Vec<ExtLink> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // Markdown links/images: ![alt](target) and [text](target)
    let mut i = 0;
    while let Some(open) = src[i..].find("](") {
        let start = i + open + 2;
        if let Some(close) = src[start..].find(')') {
            let target = src[start..start + close].split_whitespace().next().unwrap_or("").to_string();
            i = start + close + 1;
            let is_image = { let before = &src[..start - 2]; before.rfind('[').map(|b| before[..b].ends_with('!')).unwrap_or(false) };
            let kind = if target.starts_with("http://") || target.starts_with("https://") {
                let clean = target.split('?').next().unwrap_or(&target);
                let ext = clean.rsplit('.').next().unwrap_or("").to_lowercase();
                if is_image || IMAGE_EXTS.contains(&ext.as_str()) { "image_url" } else { "url" }
            } else if target.starts_with("file://") { "file" } else { continue };
            if seen.insert(target.clone()) { out.push(ExtLink { link: target, kind: kind.into() }); }
        } else { break; }
    }
    Ok(out)
}

async fn localize_target(root: &Path, link: &str, action: &str) -> Result<String, String> {
    // returns the replacement embed/wikilink text for the link target
    match (action, link) {
        ("store", l) if l.starts_with("file://") => {
            let p = l.trim_start_matches("file://");
            Ok(store_attachment_impl(root, Path::new(p))?.embed)
        }
        ("store", l) => Ok(store_url_attachment(root.display().to_string(), l.to_string()).await?.embed),
        ("convert", l) => {
            let conv = if l.starts_with("file://") {
                convert_file_to_markdown(l.trim_start_matches("file://").to_string()).await?
            } else { convert_url_to_markdown(l.to_string()).await? };
            let path = save_markdown_to_imports(root, &conv.suggested_name, &conv.markdown)?;
            let title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            Ok(format!("[[{}]]", title))
        }
        _ => Err(format!("Unknown action: {}", action)),
    }
}

async fn localize_link_impl2(root: &Path, note: &Path, link: &str, action: &str) -> Result<String, String> {
    let src = std::fs::read_to_string(note).map_err(|e| e.to_string())?;
    let replacement = localize_target(root, link, action).await?;
    // Replace whole markdown-link constructs targeting `link` with the replacement embed;
    // the wikilink/embed supersedes the [text](target) form entirely.
    let mut updated = src.clone();
    for pat in [format!("]({})", link)] {
        while let Some(pos) = updated.find(&pat) {
            let before = &updated[..pos];
            let open = before.rfind('[').ok_or("malformed link")?;
            let bang = open > 0 && updated.as_bytes()[open - 1] == b'!';
            let start = if bang { open - 1 } else { open };
            let end = pos + pat.len();
            updated.replace_range(start..end, &replacement);
        }
    }
    std::fs::write(note, &updated).map_err(|e| e.to_string())?;
    vault::invalidate(root);
    Ok(updated)
}

#[tauri::command]
fn scan_external_links(root: String, note_path: String) -> Result<Vec<ExtLink>, String> {
    scan_external_links_impl(Path::new(&root), Path::new(&note_path))
}

#[tauri::command]
async fn localize_link(root: String, note_path: String, link: String, action: String) -> Result<String, String> {
    localize_link_impl2(Path::new(&root), Path::new(&note_path), &link, &action).await
}
```

For the sync test in Step 1, add a thin sync wrapper `fn localize_link_impl(root, note, link, action)` that drives `localize_link_impl2` via `tauri::async_runtime::block_on` (mirror how other async logic is tested in this file — inspect and match). Register both commands. Note `convert_file_to_markdown`/`convert_url_to_markdown` are `async fn` commands callable as plain functions.

- [ ] **Step 4: Run tests** — targeted, full gui suite, clippy → green/clean.

- [ ] **Step 5: Commit**

```bash
git add gui/src/main.rs
git commit -m "GUI: scan_external_links and localize_link commands"
```

### Task 4: Drop routing + four-action dialog (JS glue)

**Files:**
- Modify: `gui/ui/index.html`

**Interfaces:**
- Consumes: `tauri://drag-drop` listener, commands from Tasks 1–2 plus `is_convertible`, `convert_file_to_markdown`, `convert_url_to_markdown`, `save_import` (with `pick: false`), `paste_image`; editors + `viewMode`; `toast`.
- Produces: `insertIntoNote(text)` (used by Task 5), the `#drop-dialog` overlay, `handleNoteDrop(paths)`.

- [ ] **Step 1: Insertion helper:**

```js
// ---- Insert text into the open note at the natural point ----
async function insertIntoNote(text) {
  if (!currentFile) return;
  if (viewMode === 'split') {
    const p = editor.selectionStart;
    editor.setRangeText(text, p, p, 'end');
    editor.dispatchEvent(new Event('input'));
    editor.focus();
  } else if (viewMode === 'live') {
    const ta = document.querySelector('#live-doc textarea.block-edit');
    if (ta) {
      const p = ta.selectionStart;
      ta.setRangeText(text, p, p, 'end');
      ta.dispatchEvent(new Event('input'));
      ta.focus();
    } else {
      const src = await invoke('read_source', { path: currentFile });
      const nl = src.endsWith('\n') ? '' : '\n';
      await invoke('save_file', { path: currentFile, content: src + nl + text + '\n', vaultRoot: currentFolder });
      toast('Added at end of note', 'success');
    }
  } else { // reader
    const src = await invoke('read_source', { path: currentFile });
    const nl = src.endsWith('\n') ? '' : '\n';
    await invoke('save_file', { path: currentFile, content: src + nl + text + '\n', vaultRoot: currentFolder });
    toast('Added at end of note', 'success');
  }
}
```

(The reader/live-fallback branches share the append logic — factor a small `appendToNote(text)` inner helper.) The watcher refreshes the rendered view after `save_file`.

- [ ] **Step 2: Dialog markup + CSS** (existing overlay styling conventions; reuse `.overlay`-style classes if a generic one exists, else mirror the cheatsheet overlay):

```html
<div id="drop-overlay" role="dialog" aria-label="Dropped file">
  <div id="drop-dialog">
    <h3 id="drop-title"></h3>
    <button data-act="copy">Copy into vault &amp; link</button>
    <button data-act="linkorig">Link original location</button>
    <button data-act="convnote">Convert to new linked note</button>
    <button data-act="convinline">Convert inline</button>
    <button data-act="cancel">Cancel (Esc)</button>
  </div>
</div>
```

```css
  #drop-overlay { position: fixed; inset: 0; background: rgba(0,0,0,.45); display: none; align-items: center; justify-content: center; z-index: 270; }
  #drop-overlay.show { display: flex; }
  #drop-dialog { background: var(--bg); border: 1px solid var(--border); border-radius: 10px; padding: 18px 22px; min-width: 320px; display: flex; flex-direction: column; gap: 8px; }
  #drop-dialog h3 { margin: 0 0 6px; font-size: 14px; }
  #drop-dialog button { text-align: left; padding: 8px 12px; border-radius: 6px; border: 1px solid var(--border); background: var(--bg); color: var(--fg); cursor: pointer; }
  #drop-dialog button:hover:not(:disabled) { background: var(--hover); }
  #drop-dialog button:disabled { opacity: .45; cursor: default; }
```

- [ ] **Step 3: Dialog logic + drop routing:**

```js
// ---- Drop-into-note ----
const dropOverlay = document.getElementById('drop-overlay');
function askDropAction(title, convertible) {
  return new Promise((resolve) => {
    document.getElementById('drop-title').textContent = title;
    dropOverlay.querySelectorAll('button').forEach(b => {
      b.disabled = !convertible && (b.dataset.act === 'convnote' || b.dataset.act === 'convinline');
      b.title = b.disabled ? 'This file type cannot be converted' : '';
      b.onclick = () => { dropOverlay.classList.remove('show'); releaseFocus(dropOverlay); resolve(b.dataset.act); };
    });
    dropOverlay.classList.add('show'); trapFocus(dropOverlay);
    const esc = (e) => { if (e.key === 'Escape') { e.stopPropagation(); dropOverlay.classList.remove('show'); releaseFocus(dropOverlay); document.removeEventListener('keydown', esc, true); resolve('cancel'); } };
    document.addEventListener('keydown', esc, true);
  });
}
const IMG_RE = /\.(png|jpe?g|gif|webp|svg|bmp)$/i;
async function applyDropAction(act, p) {
  const name = p.split('/').pop();
  if (act === 'copy') {
    const a = await invoke('store_attachment', { root: currentFolder, srcPath: p });
    await insertIntoNote(a.embed);
  } else if (act === 'linkorig') {
    await insertIntoNote(`[${name}](file://${encodeURI(p)})`);
  } else if (act === 'convnote') {
    const conv = await invoke('convert_file_to_markdown', { path: p });
    const saved = await invoke('save_import', { root: currentFolder, suggestedName: conv.suggested_name, markdown: conv.markdown, pick: false });
    if (saved) await insertIntoNote(`[[${saved.split('/').pop().replace(/\.md$/, '')}]]`);
  } else if (act === 'convinline') {
    const conv = await invoke('convert_file_to_markdown', { path: p });
    if (conv.markdown.length > 200 * 1024 && !confirmInsert(conv.markdown.length)) return;
    await insertIntoNote(conv.markdown);
  }
}
function confirmInsert(len) {
  // non-blocking confirm via the same dialog pattern is overkill; a toast-guard suffices:
  toast(`Converted text is ${(len / 1024).toFixed(0)} KB — drop again within 10s to confirm inline insert`, 'info', 10000);
  if (window._bigInsertOk && Date.now() - window._bigInsertOk < 10000) { window._bigInsertOk = 0; return true; }
  window._bigInsertOk = Date.now();
  return false;
}
async function handleNoteDrop(paths) {
  const imgs = paths.filter(p => IMG_RE.test(p));
  const others = paths.filter(p => !IMG_RE.test(p));
  for (const p of imgs) {
    try { const a = await invoke('store_attachment', { root: currentFolder, srcPath: p }); await insertIntoNote(a.embed); }
    catch (e) { toast(friendlyError(e), 'error'); }
  }
  if (!others.length) return;
  const convertible = await invoke('is_convertible', { path: others[0] }).catch(() => false);
  const title = others.length === 1 ? others[0].split('/').pop() : `${others.length} files`;
  const act = await askDropAction(title, convertible);
  if (act === 'cancel') return;
  for (const p of others) {
    try { await applyDropAction(act, p); } catch (e) { toast(friendlyError(e), 'error'); }
  }
}
```

Modify the existing `tauri://drag-drop` listener: before its current logic, hit-test — if `currentFolder && currentFile` and `event.payload.position` falls inside the bounding rect of the visible note area (`#scroll` in read mode, `#editor-wrap` in split/live — use `getBoundingClientRect()` on whichever is visible), call `handleNoteDrop(paths)` and `return`. IMPORTANT: `position` is in physical pixels on some platforms — divide by `window.devicePixelRatio` before comparing to DOM rects, and verify against runtime behavior (log once during manual testing).

- [ ] **Step 4: DOM drop handler for non-file drags** (image data / URLs from apps; these are NOT intercepted by Tauri because there is no file path):

```js
for (const el of [document.getElementById('scroll'), document.getElementById('editor-wrap')]) {
  el.addEventListener('dragover', (e) => { if (!e.dataTransfer?.types?.includes('Files')) e.preventDefault(); });
  el.addEventListener('drop', async (e) => {
    if (e.dataTransfer?.types?.includes('Files')) return; // handled by tauri://drag-drop
    if (!currentFile) return;
    e.preventDefault();
    const uri = e.dataTransfer.getData('text/uri-list').split('\n').find(l => l && !l.startsWith('#'));
    const item = [...(e.dataTransfer.items || [])].find(i => i.kind === 'file' && i.type.startsWith('image/'));
    try {
      if (item) { // raw image data
        const file = item.getAsFile();
        const buf = new Uint8Array(await file.arrayBuffer());
        let bin = ''; buf.forEach(b => bin += String.fromCharCode(b));
        const ext = (item.type.split('/')[1] || 'png').replace('jpeg', 'jpg');
        const embed = await invoke('paste_image', { root: currentFolder, base64Data: btoa(bin), extension: ext });
        await insertIntoNote(embed);
      } else if (uri && /^https?:\/\//.test(uri)) {
        if (IMG_RE.test(uri.split('?')[0])) {
          try { const a = await invoke('store_url_attachment', { root: currentFolder, url: uri }); await insertIntoNote(a.embed); }
          catch { await insertIntoNote(`![](${uri})`); toast('Could not download — inserted a link instead', 'info'); }
        } else {
          const act = await askDropAction(uri, true);
          if (act === 'cancel') return;
          if (act === 'copy') { const a = await invoke('store_url_attachment', { root: currentFolder, url: uri }); await insertIntoNote(a.embed); }
          else if (act === 'linkorig') await insertIntoNote(`<${uri}>`);
          else {
            const conv = await invoke('convert_url_to_markdown', { url: uri });
            if (act === 'convnote') {
              const saved = await invoke('save_import', { root: currentFolder, suggestedName: conv.suggested_name, markdown: conv.markdown, pick: false });
              if (saved) await insertIntoNote(`[[${saved.split('/').pop().replace(/\.md$/, '')}]]`);
            } else if (conv.markdown.length <= 200 * 1024 || confirmInsert(conv.markdown.length)) {
              await insertIntoNote(conv.markdown);
            }
          }
        }
      }
    } catch (err) { toast(friendlyError(err), 'error'); }
  });
}
```

- [ ] **Step 5: Verify** — `cargo test -p to_markdown_gui` + clippy (guard); live checks (flag if headless): Finder file onto note → dialog; Finder file onto sidebar → old behavior; image file onto note → instant embed; Safari image drag → downloads & embeds (offline → link fallback); Esc cancels; multi-file applies one action to all.

- [ ] **Step 6: Commit**

```bash
git add gui/ui/index.html
git commit -m "GUI: drop-into-note routing, four-action dialog, DOM drop for app drags"
```

### Task 5: Link localization UI (per-link menu + whole-note dialog)

**Files:**
- Modify: `gui/ui/index.html`

**Interfaces:**
- Consumes: `scan_external_links`, `localize_link` (Task 3), `ctxMenu`/`showCtx` helpers, `COMMANDS`, `insertIntoNote`'s refresh conventions (`openFile(currentFile, false, true)` to re-render preserving scroll — check how other code refreshes after source changes and match).

- [ ] **Step 1: Per-link context menu** — in the reader (`#content`), a `contextmenu` handler on `a[href^="http"], a[href^="file:"], img[src^="http"]`:

```js
document.getElementById('content').addEventListener('contextmenu', (e) => {
  const a = e.target.closest('a[href], img[src]');
  if (!a || !currentFile) return;
  const target = a.tagName === 'IMG' ? a.getAttribute('src') : a.getAttribute('href');
  if (!/^(https?|file):/.test(target || '')) return;
  e.preventDefault();
  showCtx(e.clientX, e.clientY, [
    { label: 'Store in vault', run: () => runLocalize(target, 'store') },
    { label: 'Convert to markdown note', run: () => runLocalize(target, 'convert') },
  ]);
});
async function runLocalize(link, action) {
  try {
    toast('Localizing…', 'info', 1500);
    await invoke('localize_link', { root: currentFolder, notePath: currentFile, link, action });
    await openFile(currentFile, false, true);
    toast('Link localized', 'success');
  } catch (e) { toast(friendlyError(e), 'error', 6000); }
}
```

(Adapt `showCtx`'s actual item format — read its definition first; the existing one may take `[label, fn]` pairs.) Note: rendered `img[src]` may be a rewritten/asset URL for local images — only http(s)/file targets pass the regex, so vault-local images are naturally excluded; verify with a fixture note.

- [ ] **Step 2: Whole-note dialog** — palette command `{ label: 'Localize External Links…', run: localizeAllDialog }`:

```js
async function localizeAllDialog() {
  if (!currentFile) return;
  const links = await invoke('scan_external_links', { root: currentFolder, notePath: currentFile });
  if (!links.length) { toast('No external links in this note', 'info'); return; }
  // Reuse the drop dialog shell: replace buttons with a list of selects.
  const dlg = document.getElementById('drop-dialog');
  dlg.innerHTML = '<h3>Localize external links</h3>' + links.map((l, i) =>
    `<div class="loc-row"><span title="${escapeHtml(l.link)}">${escapeHtml(l.link.length > 48 ? l.link.slice(0, 45) + '…' : l.link)}</span>
     <select data-i="${i}">
       <option value="skip"${l.kind === 'image_url' ? '' : ' selected'}>Skip</option>
       <option value="store"${l.kind === 'image_url' ? ' selected' : ''}>Store</option>
       <option value="convert">Convert</option>
     </select></div>`).join('') +
    '<div><button id="loc-apply">Apply</button> <button id="loc-cancel">Cancel</button></div>';
  dropOverlay.classList.add('show'); trapFocus(dropOverlay);
  const close = () => { dropOverlay.classList.remove('show'); releaseFocus(dropOverlay); restoreDropDialog(); };
  document.getElementById('loc-cancel').onclick = close;
  document.getElementById('loc-apply').onclick = async () => {
    const jobs = [...dlg.querySelectorAll('select')].map(s => ({ link: links[+s.dataset.i].link, action: s.value })).filter(j => j.action !== 'skip');
    close();
    let ok = 0;
    for (const j of jobs) {
      try { await invoke('localize_link', { root: currentFolder, notePath: currentFile, link: j.link, action: j.action }); ok++; }
      catch (e) { toast(`Skipped ${j.link.slice(0, 40)}: ${friendlyError(e)}`, 'error', 5000); }
    }
    if (ok) { await openFile(currentFile, false, true); toast(`Localized ${ok} link${ok === 1 ? '' : 's'}`, 'success'); }
  };
}
```

`restoreDropDialog()` re-creates the four-action button markup from Task 4 (factor Task 4's dialog body into a template string both paths set, so the shell is shared without breakage). Add CSS: `.loc-row { display: flex; justify-content: space-between; gap: 12px; align-items: center; font-size: 13px; }`.

- [ ] **Step 3: Verify** — guard tests + clippy; live (flag if headless): right-click a remote image → Store rewrites the link and the image now loads from the vault; convert on a URL creates an Imports/ note and a wikilink; palette command lists links with images defaulting to Store; a dead URL is skipped with a toast and the note is untouched for that link.

- [ ] **Step 4: Commit**

```bash
git add gui/ui/index.html
git commit -m "GUI: per-link and whole-note external-link localization UI"
```

### Task 6: Documentation

**Files:**
- Modify: `docs/gui/USER_GUIDE.md`, `docs/gui/GUI.md`, `CHANGELOG.md`

- [ ] **Step 1: USER_GUIDE.md** — new "Dropping files into notes" section (image auto-embed incl. from browsers, the four actions, multi-file) and "Localizing external links" (right-click menu, palette command, defaults). Match existing style.
- [ ] **Step 2: GUI.md** — extend the manual checklist with Task 4/5's verify items, one line each.
- [ ] **Step 3: CHANGELOG.md** — new Unreleased section with both features.
- [ ] **Step 4: Gate** — `cargo test -p to_markdown_gui` + clippy → green/clean.
- [ ] **Step 5: Commit**

```bash
git add docs/gui/USER_GUIDE.md docs/gui/GUI.md CHANGELOG.md
git commit -m "GUI: document drop-into-note and link localization"
```

---

## Notes for the executor

- Task order: 1 → 2 → 3 are Rust TDD (3 depends on 1–2); 4 depends on 1–2; 5 depends on 3 and 4's dialog shell; 6 last.
- The `tauri://drag-drop` position units (physical vs logical pixels) are the main platform risk in Task 4 — the plan mandates a devicePixelRatio check; verify live and note the finding.
- The link-scanner in Task 3 is a pragmatic hand-rolled parser consistent with the codebase's existing wikilink scanning; do not pull in a markdown-AST pass for it.
- `confirmInsert`'s drop-twice-to-confirm guard is deliberately simple; if it proves confusing in live testing, note it for follow-up rather than redesigning mid-task.
- Live visual checks that can't run headless must be flagged in reports and appended to GUI.md's checklist (Task 6 consolidates).
