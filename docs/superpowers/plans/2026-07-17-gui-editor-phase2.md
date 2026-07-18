# GUI Phase 2: Rust-First Hybrid Editor Implementation Plan (rev 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Typora-style hybrid editing (live preview + highlighted source/split) with ALL editor intelligence in Rust — no third-party JS editor libraries — plus wikilink autocomplete and debounced autosave.

**Architecture:** New Rust commands in `gui/src/` do the heavy lifting: `highlight_markdown` computes markdown token spans with pulldown-cmark's offset iterator; `wikilink_complete` fuzzy-matches over the existing vault index; `render_blocks` gains a content-hash cache. The frontend (`gui/ui/index.html`, hand-written vanilla JS) only displays results: a backdrop `<pre>` behind the split-mode textarea shows Rust-computed highlighting; the existing Rust-backed block-based live mode is polished, not replaced. Reader mode untouched.

**Tech Stack:** Rust (pulldown-cmark, existing `vault` index, `once_cell`, std `DefaultHasher`), Tauri 2, hand-written vanilla JS glue only.

## Global Constraints

- **Rust-first (revised spec):** all editor logic in Rust. No third-party JS editor libraries, no npm, no bundler, no prebuilt JS bundles beyond already-vendored Mermaid/KaTeX. Only hand-written vanilla JS glue in `gui/ui/index.html`.
- New Rust commands live in `gui/src/main.rs` (or `gui/src/render.rs` for pure functions), are added to the existing `invoke_handler` list, and get unit tests in the same file's `#[cfg(test)]` module, like `daily_note` / `graph_data` do.
- No new crate dependencies unless a task explicitly says so (none do — pulldown-cmark 0.12, serde, once_cell are already in `gui/Cargo.toml`).
- No blocking dialogs; frontend failures use the existing `toast(msg, kind)` helper.
- `cargo test -p to_markdown_gui` green after every task. TDD for every Rust command: failing test first.
- MSRV 1.88 untouched; workspace lints apply (`cargo clippy -p to_markdown_gui` must stay clean).

## Existing code you will build on (find anchors by identifier, not line number)

- `gui/src/main.rs`: `render_blocks(md, vault_root, file_path) -> Vec<serde_json::Value>` (splits source into blocks via `render::split_blocks`, renders each to `{text, html}`); `quick_list(root)` returns `[{path, title, aliases}]` from `vault::get_index`; the `invoke_handler` list; existing `#[cfg(test)]` tests near the bottom.
- `gui/src/render.rs`: `split_blocks`, `RenderOpts`, the pulldown-cmark render pipeline.
- `gui/ui/index.html`: `<textarea id="editor">` in `#editor-wrap` (split-mode source editor); `#edit-preview` (split reader pane); `setViewMode(mode)` with modes `read | live | split`; the block-based live mode (`liveBlocks`, `render_blocks` round-trips, per-block textarea editing); `saveNow()`; `SHORTCUTS` and `COMMANDS` arrays; `applyTheme` / `effectiveDark`; `escapeHtml`.

---

### Task 1: Rust `highlight_markdown` command

**Files:**
- Modify: `gui/src/main.rs` (command + tests + invoke_handler registration)

**Interfaces:**
- Produces: Tauri command `highlight_markdown(source: String) -> Vec<Span>` where each span is `{ start: usize, end: usize, kind: String }` (byte offsets into `source`; kinds: `heading`, `emphasis`, `strong`, `code`, `codeblock`, `link`, `wikilink`, `blockquote`, `list_marker`, `frontmatter`). Consumed by Tasks 2 and 4.

- [ ] **Step 1: Write failing tests** — in the existing `#[cfg(test)]` module of `gui/src/main.rs`:

```rust
#[test]
fn highlight_markdown_spans_basic() {
    let spans = highlight_spans("# Title\n\nsome **bold** and `code`\n");
    let kind_at = |off: usize| spans.iter().find(|s| s.start <= off && off < s.end).map(|s| s.kind.as_str());
    assert_eq!(kind_at(0), Some("heading"));      // '#'
    assert_eq!(kind_at(14), Some("strong"));      // inside **bold**
    assert_eq!(kind_at(24), Some("code"));        // inside `code`
    assert!(spans.iter().all(|s| s.start < s.end));
}

#[test]
fn highlight_markdown_spans_wikilink_and_fence() {
    let src = "see [[Other Note]]\n\n```rust\nfn x() {}\n```\n";
    let spans = highlight_spans(src);
    assert!(spans.iter().any(|s| s.kind == "wikilink" && &src[s.start..s.end] == "[[Other Note]]"));
    assert!(spans.iter().any(|s| s.kind == "codeblock"));
}

#[test]
fn highlight_markdown_spans_frontmatter() {
    let src = "---\ntitle: X\n---\n# H\n";
    let spans = highlight_spans(src);
    assert!(spans.iter().any(|s| s.kind == "frontmatter" && s.start == 0));
}
```

- [ ] **Step 2: Run tests, verify they fail** — `cargo test -p to_markdown_gui highlight_markdown` → FAIL: `highlight_spans` not found.

- [ ] **Step 3: Implement** — in `gui/src/main.rs`:

```rust
#[derive(serde::Serialize, Debug)]
struct Span { start: usize, end: usize, kind: String }

fn highlight_spans(source: &str) -> Vec<Span> {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    let mut spans: Vec<Span> = Vec::new();
    let mut push = |range: std::ops::Range<usize>, kind: &str| {
        if range.start < range.end {
            spans.push(Span { start: range.start, end: range.end, kind: kind.into() });
        }
    };
    // Frontmatter is not markdown; detect it textually before parsing.
    let mut body_off = 0usize;
    if source.starts_with("---\n") {
        if let Some(end) = source[4..].find("\n---") {
            let after = 4 + end + 4;
            let fm_end = source[after..].find('\n').map(|n| after + n + 1).unwrap_or(source.len());
            push(0..fm_end, "frontmatter");
            body_off = fm_end;
        }
    }
    let body = &source[body_off..];
    // Wikilinks: pulldown-cmark doesn't know them; linear scan.
    let mut i = 0;
    while let Some(open) = body[i..].find("[[") {
        let s = i + open;
        match body[s..].find("]]") {
            Some(close) => { push(body_off + s..body_off + s + close + 2, "wikilink"); i = s + close + 2; }
            None => break,
        }
    }
    for (event, range) in Parser::new_ext(body, Options::all()).into_offset_iter() {
        let r = body_off + range.start..body_off + range.end;
        match event {
            Event::Start(Tag::Heading { .. }) => push(r, "heading"),
            Event::Start(Tag::Emphasis) => push(r, "emphasis"),
            Event::Start(Tag::Strong) => push(r, "strong"),
            Event::Start(Tag::CodeBlock(_)) => push(r, "codeblock"),
            Event::Start(Tag::Link { .. }) | Event::Start(Tag::Image { .. }) => push(r, "link"),
            Event::Start(Tag::BlockQuote(_)) => push(r, "blockquote"),
            Event::Code(_) => push(r, "code"),
            Event::Start(Tag::Item) => {
                let text = &body[range.start..range.end];
                let marker_len = text.find(|c: char| !"-*+0123456789. \t".contains(c)).unwrap_or(0);
                let end = (r.start + marker_len).min(r.end);
                push(r.start..end, "list_marker");
            }
            _ => {}
        }
    }
    spans.sort_by_key(|s| (s.start, s.end));
    spans
}

#[tauri::command]
fn highlight_markdown(source: String) -> Vec<Span> { highlight_spans(&source) }
```

Register `highlight_markdown` in the `invoke_handler` list.

- [ ] **Step 4: Run tests, verify pass** — `cargo test -p to_markdown_gui highlight_markdown` → 3 passed; then full `cargo test -p to_markdown_gui` and `cargo clippy -p to_markdown_gui` — green/clean. If an exact-offset assertion fails, `dbg!` the spans and adjust implementation or the test's offset to the real pulldown-cmark ranges — the behavioral claims (right kinds covering the right text) are the contract.

- [ ] **Step 5: Commit**

```bash
git add gui/src/main.rs
git commit -m "GUI: highlight_markdown command (Rust markdown token spans)"
```

### Task 2: Highlighted source editor (split mode, backdrop overlay)

**Files:**
- Modify: `gui/ui/index.html` (CSS, markup around `#editor`, JS glue)

**Interfaces:**
- Consumes: `highlight_markdown` (Task 1), existing `#editor` textarea, `setViewMode`, `escapeHtml`.
- Produces: `refreshHighlight()`, `spansToHtml(text, spans)`, `byteToCharIndex(text)` — reused by Task 4.

- [ ] **Step 1: Backdrop markup + CSS** — wrap the existing `<textarea id="editor">` (keep its id and all handlers):

```html
<div id="editor-shell"><pre id="editor-backdrop" aria-hidden="true"></pre><textarea id="editor" spellcheck="false"></textarea></div>
```

```css
  #editor-shell { position: relative; flex: 1; min-width: 0; display: flex; }
  #editor-backdrop, #editor-shell #editor {
    margin: 0; border: 0; overflow-y: auto; white-space: pre-wrap; overflow-wrap: anywhere;
    font: 13.5px/1.55 ui-monospace, SFMono-Regular, Menlo, monospace;
    padding: 16px 20px; box-sizing: border-box;
  }
  #editor-backdrop { position: absolute; inset: 0; pointer-events: none; color: var(--fg, inherit); }
  #editor-shell #editor { position: relative; flex: 1; min-width: 0; background: transparent; color: transparent; caret-color: var(--fg, currentColor); resize: none; }
  .hl-heading { font-weight: 700; color: var(--hl-heading, #2a6bcc); }
  .hl-strong { font-weight: 700; }
  .hl-emphasis { font-style: italic; }
  .hl-code, .hl-codeblock { color: var(--hl-code, #8a5a00); }
  .hl-link, .hl-wikilink { color: var(--hl-link, #2a6bcc); text-decoration: underline; }
  .hl-blockquote { color: var(--hl-quote, #5a6472); font-style: italic; }
  .hl-list_marker { color: var(--hl-heading, #2a6bcc); }
  .hl-frontmatter { opacity: .55; }
  :root[data-theme="dark"] { --hl-heading: #7aa2f7; --hl-code: #e0af68; --hl-link: #7aa2f7; --hl-quote: #9aa5ce; }
```

CRITICAL: bold spans change glyph widths, which would desync backdrop and textarea text. Counter it: `.hl-heading, .hl-strong { font-weight: 700; }` must therefore be DROPPED from the backdrop if drift is observed with a wrapped bold line — in that case express heading/strong via color only. Start with weight enabled; the verify step decides.

- [ ] **Step 2: Highlight glue JS** — near the `const editor = …` declaration:

```js
// ---- Rust-computed source highlighting (backdrop overlay) ----
const editorBackdrop = document.getElementById('editor-backdrop');
let hlTimer = null, hlSeq = 0;
function byteToCharIndex(text) {
  const enc = new TextEncoder();
  const map = new Map(); let b = 0;
  for (let c = 0; c < text.length; c++) { map.set(b, c); b += enc.encode(text[c]).length; }
  map.set(b, text.length);
  return (off) => { while (off > 0 && !map.has(off)) off--; return map.get(off) ?? text.length; };
}
function spansToHtml(text, spans) {
  let out = '', pos = 0;
  for (const s of spans) {
    if (s.start < pos) continue;               // skip overlaps; first span wins
    out += escapeHtml(text.slice(pos, s.start));
    out += '<span class="hl-' + s.kind + '">' + escapeHtml(text.slice(s.start, s.end)) + '</span>';
    pos = s.end;
  }
  return out + escapeHtml(text.slice(pos));
}
async function refreshHighlight() {
  const text = editor.value, seq = ++hlSeq;
  try {
    const raw = await invoke('highlight_markdown', { source: text });
    if (seq !== hlSeq) return;                 // stale response
    const toChar = byteToCharIndex(text);
    const spans = raw.map(s => ({ start: toChar(s.start), end: toChar(s.end), kind: s.kind }));
    editorBackdrop.innerHTML = spansToHtml(text, spans) + '\n';
  } catch { editorBackdrop.textContent = text; }
}
editor.addEventListener('input', () => { clearTimeout(hlTimer); hlTimer = setTimeout(refreshHighlight, 120); });
editor.addEventListener('scroll', () => { editorBackdrop.scrollTop = editor.scrollTop; }, { passive: true });
```

- [ ] **Step 3: Hook mode entry** — wherever `setViewMode('split')` loads source into `editor.value`, call `refreshHighlight()` after.

- [ ] **Step 4: Verify** — `cargo tauri dev`: split mode shows colored headings/bold/code/wikilinks exactly aligned under the caret text in all four themes; typing updates highlights ~120 ms later; scroll stays aligned; an emoji-laden note doesn't drift; a long wrapped bold line doesn't drift (else apply the Step 1 weight-drop rule). Headless fallback: static re-read + `cargo test -p to_markdown_gui`; flag skipped visual checks.

- [ ] **Step 5: Commit**

```bash
git add gui/ui/index.html
git commit -m "GUI: Rust-highlighted source editor via backdrop overlay"
```

### Task 3: Rust `wikilink_complete` command

**Files:**
- Modify: `gui/src/main.rs`

**Interfaces:**
- Consumes: `vault::get_index` (as `quick_list` does).
- Produces: command `wikilink_complete(root: String, prefix: String) -> Vec<{label, path}>` — top 20 matches; rank 0 title-prefix, rank 1 title-substring, rank 2 alias-substring; case-insensitive; ties alphabetical. Consumed by Task 5.

- [ ] **Step 1: Failing tests** (a `fixture_vault()` helper already exists in the test module; inspect it and pick a prefix that matches at least one real fixture title — record the choice in your report):

```rust
#[test]
fn wikilink_complete_ranks_prefix_first() {
    let root = fixture_vault();
    let hits = wikilink_matches(root.path(), "pro").unwrap();
    assert!(!hits.is_empty());
    let labels: Vec<String> = hits.iter().map(|h| h.label.to_lowercase()).collect();
    if let Some(fs) = labels.iter().position(|l| !l.starts_with("pro")) {
        assert!(labels[..fs].iter().all(|l| l.starts_with("pro")));
        assert!(labels[fs..].iter().all(|l| !l.starts_with("pro")));
    }
}

#[test]
fn wikilink_complete_empty_prefix_lists_up_to_20() {
    let root = fixture_vault();
    let hits = wikilink_matches(root.path(), "").unwrap();
    assert!(!hits.is_empty());
    assert!(hits.len() <= 20);
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p to_markdown_gui wikilink_complete` → FAIL: not found.

- [ ] **Step 3: Implement:**

```rust
#[derive(serde::Serialize, Debug)]
struct WikiMatch { label: String, path: String }

fn wikilink_matches(root: &Path, prefix: &str) -> Result<Vec<WikiMatch>, String> {
    let idx = vault::get_index(root).map_err(|e| e.to_string())?;
    let q = prefix.to_lowercase();
    let mut ranked: Vec<(u8, String, String)> = Vec::new();
    for n in idx.notes.values() {
        let t = n.title.to_lowercase();
        let rank = if q.is_empty() || t.starts_with(&q) { 0 }
            else if t.contains(&q) { 1 }
            else if n.aliases.iter().any(|a| a.to_lowercase().contains(&q)) { 2 }
            else { continue };
        ranked.push((rank, n.title.clone(), n.path.clone()));
    }
    ranked.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    Ok(ranked.into_iter().take(20).map(|(_, label, path)| WikiMatch { label, path }).collect())
}

#[tauri::command]
fn wikilink_complete(root: String, prefix: String) -> Result<Vec<WikiMatch>, String> {
    wikilink_matches(Path::new(&root), &prefix)
}
```

Register in `invoke_handler`.

- [ ] **Step 4: Run tests** — targeted, then full suite + clippy → green/clean.

- [ ] **Step 5: Commit**

```bash
git add gui/src/main.rs
git commit -m "GUI: wikilink_complete command (ranked vault matches)"
```

### Task 4: Live-block mode polish + Rust render cache

**Files:**
- Modify: `gui/src/main.rs` and/or `gui/src/render.rs` (cache + tests)
- Modify: `gui/ui/index.html` (active-block highlighting glue)

**Interfaces:**
- Consumes: `render_blocks`, `highlight_markdown`, Task 2's `spansToHtml`/`byteToCharIndex`, the existing block-edit flow (search for where a per-block `<textarea>` is created and `ta.value = liveBlocks[i].text`).
- Produces: cached `render_blocks` (signature unchanged — frontend untouched by the cache); highlighted active-block editing.

- [ ] **Step 1: Failing cache test:**

```rust
#[test]
fn render_block_cache_hits() {
    render_cache_clear();
    let opts = RenderOpts { file_dir: None, vault_root: None };
    let html1 = render_block_cached("**hi**", &opts);
    let (hits0, _) = render_cache_stats();
    let html2 = render_block_cached("**hi**", &opts);
    let (hits1, _) = render_cache_stats();
    assert_eq!(html1, html2);
    assert_eq!(hits1, hits0 + 1);
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p to_markdown_gui render_block_cache` → FAIL.

- [ ] **Step 3: Implement cache** (place beside `render_blocks`; adapt the inner render call to whatever `render_blocks` actually invokes per block today — inspect first):

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

static RENDER_CACHE: once_cell::sync::Lazy<Mutex<(std::collections::HashMap<u64, String>, u64)>> =
    once_cell::sync::Lazy::new(|| Mutex::new((std::collections::HashMap::new(), 0)));

fn render_cache_clear() { let mut c = RENDER_CACHE.lock().unwrap(); c.0.clear(); c.1 = 0; }
fn render_cache_stats() -> (u64, usize) { let c = RENDER_CACHE.lock().unwrap(); (c.1, c.0.len()) }

fn render_block_cached(text: &str, opts: &RenderOpts) -> String {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    opts.vault_root.map(|p| p.to_path_buf()).hash(&mut h);
    opts.file_dir.map(|p| p.to_path_buf()).hash(&mut h);
    let key = h.finish();
    {
        let mut c = RENDER_CACHE.lock().unwrap();
        if let Some(html) = c.0.get(&key) { c.1 += 1; return html.clone(); }
    }
    let html = /* the existing per-block render expression from render_blocks, applied to `text` with `opts` */;
    let mut c = RENDER_CACHE.lock().unwrap();
    if c.0.len() > 4096 { c.0.clear(); } // crude bound; blocks are small
    c.0.insert(key, html.clone());
    html
}
```

Reroute `render_blocks`'s per-block rendering through `render_block_cached`.

- [ ] **Step 4: Run tests** — targeted, full, clippy → green/clean.

- [ ] **Step 5: Active-block highlighting glue** — where the live mode creates the per-block editing `<textarea>`: wrap it in a mini shell reusing Task 2's technique — a `<pre class="block-backdrop">` sibling sharing the block textarea's computed font/padding/wrapping; on input (120 ms debounce) call `highlight_markdown` on the block text and paint via the shared `spansToHtml`/`byteToCharIndex` helpers (do NOT duplicate them); mirror scrollTop. Add:

```css
  .block-shell { position: relative; }
  .block-shell .block-backdrop { position: absolute; inset: 0; pointer-events: none; margin: 0; }
  .block-shell textarea { position: relative; background: transparent; color: transparent; caret-color: var(--fg, currentColor); }
```

- [ ] **Step 6: Verify** — live mode: click a paragraph → source appears with highlighting aligned; blur → re-renders; unchanged sibling blocks re-render via cache (subjectively instant on a long note); mermaid/katex blocks still render. Full suite + clippy green.

- [ ] **Step 7: Commit**

```bash
git add gui/src/main.rs gui/src/render.rs gui/ui/index.html
git commit -m "GUI: render_blocks content-hash cache; highlighted active-block editing"
```

### Task 5: Wikilink autocomplete UI + debounced autosave

**Files:**
- Modify: `gui/ui/index.html`

**Interfaces:**
- Consumes: `wikilink_complete` (Task 3), `#editor`, live-block textareas, `saveNow()`, `escapeHtml`, `currentFolder`, `viewMode`.
- Produces: final editing UX; `COMMANDS` updated.

- [ ] **Step 1: Autocomplete popup** — one shared dropdown for both editors:

```html
<div id="wiki-ac" role="listbox"></div>
```

```css
  #wiki-ac { position: fixed; z-index: 280; display: none; min-width: 220px; max-height: 40vh; overflow-y: auto;
    background: var(--bg, #fff); border: 1px solid rgba(128,128,128,.35); border-radius: 8px;
    box-shadow: 0 6px 24px rgba(0,0,0,.25); font-size: 13px; }
  #wiki-ac div { padding: 5px 10px; cursor: pointer; }
  #wiki-ac div.sel { background: rgba(100,140,220,.25); }
```

```js
// ---- Wikilink autocomplete ([[ trigger) ----
const wikiAc = document.getElementById('wiki-ac');
let acItems = [], acSel = 0, acTarget = null, acStart = -1;
function acHide() { wikiAc.style.display = 'none'; acTarget = null; }
function acApply() {
  const it = acItems[acSel]; if (!it || !acTarget) return;
  const v = acTarget.value, caret = acTarget.selectionStart;
  acTarget.value = v.slice(0, acStart) + it.label + ']]' + v.slice(caret);
  const pos = acStart + it.label.length + 2;
  acTarget.setSelectionRange(pos, pos);
  acTarget.dispatchEvent(new Event('input'));
  acHide();
}
async function acMaybeShow(ta) {
  const upto = ta.value.slice(0, ta.selectionStart);
  const m = upto.match(/\[\[([^\]\n]*)$/);
  if (!m || !currentFolder) { acHide(); return; }
  acTarget = ta; acStart = ta.selectionStart - m[1].length;
  try {
    const hits = await invoke('wikilink_complete', { root: currentFolder, prefix: m[1] });
    acItems = hits; acSel = 0;
    if (!hits.length) { acHide(); return; }
    wikiAc.innerHTML = hits.map((h, i) => `<div class="${i === 0 ? 'sel' : ''}">${escapeHtml(h.label)}</div>`).join('');
    const r = ta.getBoundingClientRect();
    wikiAc.style.left = r.left + 16 + 'px';
    wikiAc.style.top = Math.min(r.top + 60, innerHeight - 200) + 'px';
    wikiAc.style.display = 'block';
    Array.from(wikiAc.children).forEach((el, i) => { el.onmousedown = (e) => { e.preventDefault(); acSel = i; acApply(); }; });
  } catch { acHide(); }
}
function acKeydown(e) {
  if (wikiAc.style.display !== 'block') return false;
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    acSel = (acSel + (e.key === 'ArrowDown' ? 1 : acItems.length - 1)) % acItems.length;
    Array.from(wikiAc.children).forEach((el, i) => el.classList.toggle('sel', i === acSel));
    e.preventDefault(); return true;
  }
  if (e.key === 'Enter' || e.key === 'Tab') { acApply(); e.preventDefault(); return true; }
  if (e.key === 'Escape') { acHide(); e.stopPropagation(); return true; }
  return false;
}
```

Wire: `editor.addEventListener('input', () => acMaybeShow(editor));` and at the TOP of the existing `editor.addEventListener('keydown', …)` handler insert `if (acKeydown(e)) return;`. Do the same pair on the live-block textarea where it is created. `acHide()` on blur of either.

- [ ] **Step 2: Debounced silent autosave** — change `saveNow` to `async function saveNow(silent = false)` and gate its success toast/flash on `!silent` (audit the current body; error toasts stay unconditional). Then:

```js
let autosaveTimer = null;
editor.addEventListener('input', () => {
  clearTimeout(autosaveTimer);
  autosaveTimer = setTimeout(() => { if (viewMode === 'split') saveNow(true); }, 1200);
});
```

Live mode already saves on block commit; leave it.

- [ ] **Step 3: COMMANDS + Escape-order audit** — add `{ label: 'Insert Wikilink ([[)', run: () => { if (viewMode === 'split') { const p = editor.selectionStart; editor.setRangeText('[[', p, p, 'end'); editor.focus(); acMaybeShow(editor); } } }`. Verify the popup's Escape wins over cheat-sheet/overlay closers (its `stopPropagation` handles bubble-phase; the lightbox capture-phase handler only acts when the lightbox is open — no conflict).

- [ ] **Step 4: Verify** — `[[` + letters pops ranked matches in split AND inside a live block; arrows/Enter insert `[[Title]]` and re-trigger highlighting; autosave writes to disk ~1.2 s after typing stops with no toast; Cmd+S still gives feedback; Escape closes only the popup. Full suite + clippy green.

- [ ] **Step 5: Commit**

```bash
git add gui/ui/index.html
git commit -m "GUI: wikilink autocomplete + silent debounced autosave"
```

### Task 6: Split scroll sync + documentation

**Files:**
- Modify: `gui/ui/index.html` (scroll sync)
- Modify: `docs/gui/USER_GUIDE.md`, `docs/gui/GUI.md`, `CHANGELOG.md`

- [ ] **Step 1: Proportional scroll sync** editor → preview. First `grep -n 'edit-preview' gui/ui/index.html` for an existing sync; if one exists, verify and skip. Otherwise:

```js
editor.addEventListener('scroll', () => {
  if (viewMode !== 'split') return;
  const dst = document.getElementById('edit-preview');
  const max = editor.scrollHeight - editor.clientHeight;
  if (max > 0) dst.scrollTop = (editor.scrollTop / max) * (dst.scrollHeight - dst.clientHeight);
}, { passive: true });
```

(Coexists with Task 2's backdrop scroll-mirror — two listeners on one event is fine.)

- [ ] **Step 2: Docs** — USER_GUIDE: editing section covering highlighted source mode, live-block editing with highlighting, `[[` autocomplete (arrows/Enter/Tab/Escape), silent autosave, scroll sync. GUI.md: extend the manual checklist with one line per Task 2/4/5/6 verify item. CHANGELOG: Unreleased entry "Rust-powered hybrid editing: source highlighting, wikilink autocomplete, autosave, render cache".

- [ ] **Step 3: Full gate** — `cargo test -p to_markdown_gui` and `cargo clippy -p to_markdown_gui` → green/clean.

- [ ] **Step 4: Commit**

```bash
git add gui/ui/index.html docs/gui/USER_GUIDE.md docs/gui/GUI.md CHANGELOG.md
git commit -m "GUI: split scroll sync; document Rust-first hybrid editing"
```

---

## Notes for the executor

- Tasks 1 and 3 are pure-Rust TDD and independent of each other; Tasks 2 and 4 depend on Task 1; Task 5 depends on Task 3. Execute in numeric order.
- Exact pulldown-cmark ranges (Task 1) and fixture-vault titles (Task 3) may force small test adjustments — expected tuning; keep the behavioral contracts.
- Backdrop alignment (Tasks 2/4) is the main visual risk: any metric mismatch shows as color drifting off the text; the bold-weight-drop rule in Task 2 Step 1 is the sanctioned fix.
- The legacy block-based live mode is KEPT and improved in this revision (it is already Rust-rendered) — do not delete `liveBlocks` machinery.
- If the executor cannot run `cargo tauri dev`, flag every skipped visual verification in reports so the final review lists them for a human pass.
