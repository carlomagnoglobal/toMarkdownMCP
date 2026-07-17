# GUI Polish Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining reading-polish gaps in the Tauri GUI: image lightbox, reading-progress indicator, keyboard shortcut cheat-sheet, keyboard font-size/line-height controls, and print-stylesheet polish.

**Architecture:** All changes are frontend-only, inside `gui/ui/index.html` (single-file vanilla-JS UI, one large classic `<script>` with shared top-level state; new code is appended to that script and new CSS to the main `<style>` block). No Rust changes, no new dependencies, no build step.

**Tech Stack:** Tauri 2 webview, vanilla JS/CSS. Verification is manual via `cargo tauri dev` from `gui/` (there is no JS test harness — do not add one).

## Global Constraints

- Frontend stays build-free vanilla JS: no npm, no bundler, no new toolchain (spec "Hard constraint").
- Third-party code only as vendored prebuilt files under `gui/ui/vendor/` (none needed in this plan).
- No blocking dialogs — user-visible failures use the existing `toast(msg, kind)` helper.
- Settings persist in `localStorage` (deliberate deviation from the spec's `get_settings`/`set_settings` commands: localStorage persistence already exists and works; do not add backend settings commands).
- MSRV 1.88; do not touch workspace Cargo.toml.
- Rust `cargo test` must stay green (`cargo test -p to_markdown_gui`), though no Rust files change here.

## Existing code you will reuse (already in `gui/ui/index.html`)

- `const contentBox = document.getElementById('content')` — the rendered reader element (line ~566).
- `<div id="scroll">` — the scrolling container wrapping `#content` (line ~443).
- `toast(msg, kind, ms)` — non-blocking notifications (line ~580).
- `trapFocus(overlay)` / `releaseFocus(overlay)` — overlay focus management (lines ~613–631).
- `postRender(container)` — runs after each markdown render (line ~539); the lightbox hook goes here.
- `COMMANDS` array for the Cmd+K palette (line ~1761) — every new user-facing action must also be appended there.
- Settings overlay: `openSettings()` / `applySettings()` (lines ~1728–1748); font size is `localStorage 'fontSize'`, applied to `contentBox` and `editPreview`.
- Global keydown handlers already exist; add new bindings to the main handler at line ~1365 (`const mod = e.metaKey || e.ctrlKey;`).

Line numbers drift as tasks land — locate anchors by searching for the quoted identifiers, not by line number.

---

### Task 1: Image lightbox

**Files:**
- Modify: `gui/ui/index.html` (CSS block, HTML overlays area near `#toast-stack`, JS near `postRender`)

**Interfaces:**
- Produces: `openLightbox(src)` (used by Task 3's cheat-sheet listing only as documentation; no code dependency).

- [ ] **Step 1: Add CSS** — inside the main `<style>` block, before `@media print`:

```css
  #lightbox { position: fixed; inset: 0; background: rgba(0,0,0,.85); display: none;
    align-items: center; justify-content: center; z-index: 300; cursor: zoom-out; }
  #lightbox.show { display: flex; }
  #lightbox img { max-width: 94vw; max-height: 94vh; border-radius: 4px;
    box-shadow: 0 8px 40px rgba(0,0,0,.6); }
  #content img, #edit-preview img { cursor: zoom-in; }
```

- [ ] **Step 2: Add overlay element** — next to the other overlays/`#toast-stack` markup:

```html
<div id="lightbox" role="dialog" aria-label="Image preview"><img alt=""></div>
```

- [ ] **Step 3: Add JS** — append near `postRender`:

```js
// ---- Image lightbox ----
const lightbox = document.getElementById('lightbox');
const lightboxImg = lightbox.querySelector('img');
function openLightbox(src) { lightboxImg.src = src; lightbox.classList.add('show'); }
function closeLightbox() { lightbox.classList.remove('show'); lightboxImg.src = ''; }
lightbox.addEventListener('click', closeLightbox);
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && lightbox.classList.contains('show')) { e.stopPropagation(); closeLightbox(); }
}, true);
```

- [ ] **Step 4: Hook renders** — inside `postRender(container)`, add as its last line:

```js
  container.querySelectorAll('img').forEach(img => {
    img.onclick = (e) => { e.preventDefault(); openLightbox(img.src); };
  });
```

- [ ] **Step 5: Verify manually** — `cd gui && cargo tauri dev`, open the fixture vault (`gui`'s tests reference it; any note with an image works, or paste an image with the existing paste-image feature). Click an image → fills screen; click anywhere or Esc → closes; Esc does not also close other overlays.

- [ ] **Step 6: Commit**

```bash
git add gui/ui/index.html
git commit -m "GUI: image lightbox (click to zoom, Esc/click to close)"
```

### Task 2: Reading-progress indicator

**Files:**
- Modify: `gui/ui/index.html` (CSS, HTML inside `#main` before `#scroll`, JS near the `#scroll` setup)

**Interfaces:**
- Consumes: `#scroll` container.
- Produces: nothing used by later tasks.

- [ ] **Step 1: Add CSS**:

```css
  #read-progress { position: absolute; top: 0; left: 0; height: 2px; width: 0;
    background: var(--accent, #4a7dd0); z-index: 50; transition: width .1s linear; }
  #main { position: relative; }
```

(If a `--accent` custom property already exists in the theme roots, keep it; the fallback color covers themes without one.)

- [ ] **Step 2: Add element** — first child of `<div id="main">`:

```html
<div id="read-progress"></div>
```

- [ ] **Step 3: Add JS**:

```js
// ---- Reading progress ----
const readProgress = document.getElementById('read-progress');
const scrollBox = document.getElementById('scroll');
scrollBox.addEventListener('scroll', () => {
  const max = scrollBox.scrollHeight - scrollBox.clientHeight;
  readProgress.style.width = max > 0 ? (scrollBox.scrollTop / max * 100) + '%' : '0';
}, { passive: true });
```

- [ ] **Step 4: Verify manually** — open a long note; bar grows left→right while scrolling, hits full width at bottom, invisible on short notes. Confirm it is hidden when printing (it sits in `#main`; add `#read-progress { display: none !important; }` to `@media print` if visible in print preview).

- [ ] **Step 5: Commit**

```bash
git add gui/ui/index.html
git commit -m "GUI: reading-progress indicator on reader scroll"
```

### Task 3: Shortcut cheat-sheet overlay (`?`)

**Files:**
- Modify: `gui/ui/index.html` (CSS, HTML overlay, JS; append entry to `COMMANDS`)

**Interfaces:**
- Consumes: `trapFocus` / `releaseFocus`, `COMMANDS`.
- Produces: `openCheatsheet()`.

- [ ] **Step 1: Add CSS** (reuse the app's existing overlay look — match the settings overlay's classes if one is generic; otherwise):

```css
  #cheatsheet-overlay { position: fixed; inset: 0; background: rgba(0,0,0,.45); display: none;
    align-items: center; justify-content: center; z-index: 260; }
  #cheatsheet-overlay.show { display: flex; }
  #cheatsheet { background: var(--bg, #fff); color: inherit; border-radius: 10px; padding: 20px 26px;
    max-height: 80vh; overflow-y: auto; min-width: 420px; box-shadow: 0 8px 40px rgba(0,0,0,.35); }
  #cheatsheet h3 { margin: 12px 0 6px; font-size: 13px; text-transform: uppercase; opacity: .6; }
  #cheatsheet div.row { display: flex; justify-content: space-between; gap: 24px; padding: 3px 0; font-size: 13.5px; }
  #cheatsheet kbd { background: rgba(128,128,128,.18); border-radius: 4px; padding: 1px 7px;
    font: 12px ui-monospace, monospace; }
```

- [ ] **Step 2: Add overlay markup**:

```html
<div id="cheatsheet-overlay" role="dialog" aria-label="Keyboard shortcuts"><div id="cheatsheet"></div></div>
```

- [ ] **Step 3: Add JS** — the table is data-driven so it stays honest; audit the actual bindings in the file while implementing and list exactly what exists (the set below reflects today's handlers — verify each before shipping):

```js
// ---- Shortcut cheat sheet ----
const SHORTCUTS = [
  ['General', [['Cmd/Ctrl+K', 'Command palette'], ['Cmd/Ctrl+O / Cmd+P', 'Quick switcher'],
    ['Cmd/Ctrl+,', 'Settings'], ['?', 'This cheat sheet'], ['Esc', 'Close overlay']]],
  ['View', [['Cmd/Ctrl+E', 'Live editing'], ['Cmd/Ctrl+Shift+E', 'Split source'],
    ['Cmd/Ctrl+=', 'Larger text'], ['Cmd/Ctrl+-', 'Smaller text'], ['Cmd/Ctrl+0', 'Reset text size']]],
  ['Notes', [['Cmd/Ctrl+D', 'Daily note'], ['Cmd/Ctrl+S', 'Save (editor)']]],
];
const cheatOverlay = document.getElementById('cheatsheet-overlay');
function openCheatsheet() {
  const box = document.getElementById('cheatsheet');
  box.innerHTML = SHORTCUTS.map(([title, rows]) =>
    `<h3>${title}</h3>` + rows.map(([k, d]) =>
      `<div class="row"><span>${escapeHtml(d)}</span><kbd>${escapeHtml(k)}</kbd></div>`).join('')
  ).join('');
  cheatOverlay.classList.add('show'); trapFocus(cheatOverlay);
}
function closeCheatsheet() { cheatOverlay.classList.remove('show'); releaseFocus(cheatOverlay); }
cheatOverlay.addEventListener('click', (e) => { if (e.target === cheatOverlay) closeCheatsheet(); });
document.addEventListener('keydown', (e) => {
  const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName) || document.activeElement?.isContentEditable;
  if (e.key === '?' && !typing && !e.metaKey && !e.ctrlKey) { e.preventDefault(); openCheatsheet(); }
  if (e.key === 'Escape' && cheatOverlay.classList.contains('show')) closeCheatsheet();
});
```

- [ ] **Step 4: Register in palette** — append to `COMMANDS`:

```js
  { label: 'Keyboard Shortcuts (?)', run: openCheatsheet },
```

- [ ] **Step 5: Verify manually** — `?` opens it only when not typing in an input/editor; Esc and outside-click close it; entry appears in Cmd+K; every listed shortcut actually works as described (fix the table, not the claim).

- [ ] **Step 6: Commit**

```bash
git add gui/ui/index.html
git commit -m "GUI: keyboard-shortcut cheat sheet on '?'"
```

### Task 4: Keyboard font-size controls + line-height setting

**Files:**
- Modify: `gui/ui/index.html` (settings overlay markup, `openSettings`/`applySettings`, startup restore block, main keydown handler, `COMMANDS`)

**Interfaces:**
- Consumes: `applySettings` pattern, `localStorage 'fontSize'`, `contentBox`, `editPreview`.
- Produces: `setFontSize(px)`, `localStorage 'lineHeight'`.

- [ ] **Step 1: Centralize font application** — replace the three duplicated `fontSize` snippets (in `applySettings`, and the startup restore block) with one helper defined near them, and add line-height:

```js
function setFontSize(px) {
  px = Math.max(11, Math.min(24, px));
  localStorage.setItem('fontSize', String(px));
  contentBox.style.fontSize = px + 'px';
  editPreview.style.fontSize = px + 'px';
}
function setLineHeight(lh) {
  lh = Math.max(1.3, Math.min(2.2, lh));
  localStorage.setItem('lineHeight', String(lh));
  contentBox.style.lineHeight = lh;
  editPreview.style.lineHeight = lh;
}
```

Call sites: `applySettings` uses `setFontSize(parseInt(setFont.value, 10) || 16)` and `setLineHeight(parseFloat(setLineHeight_el.value) || 1.6)`; the startup restore block becomes:

```js
if (localStorage.getItem('fontSize')) setFontSize(parseInt(localStorage.getItem('fontSize'), 10));
if (localStorage.getItem('lineHeight')) setLineHeight(parseFloat(localStorage.getItem('lineHeight')));
```

- [ ] **Step 2: Add line-height to the settings overlay** — next to the existing `#set-font` control:

```html
<label for="set-lineheight">Line height</label>
<select id="set-lineheight">
  <option value="1.4">Compact (1.4)</option>
  <option value="1.6" selected>Normal (1.6)</option>
  <option value="1.8">Relaxed (1.8)</option>
  <option value="2.0">Airy (2.0)</option>
</select>
```

with `const setLineHeight_el = document.getElementById('set-lineheight');` beside `setFont`, and `setLineHeight_el.value = localStorage.getItem('lineHeight') || '1.6';` in `openSettings()`.

- [ ] **Step 3: Keyboard bindings** — in the main keydown handler (the one with `const mod = e.metaKey || e.ctrlKey;`):

```js
  if (mod && (e.key === '=' || e.key === '+')) { e.preventDefault(); setFontSize((parseInt(localStorage.getItem('fontSize'), 10) || 16) + 1); }
  if (mod && e.key === '-') { e.preventDefault(); setFontSize((parseInt(localStorage.getItem('fontSize'), 10) || 16) - 1); }
  if (mod && e.key === '0') { e.preventDefault(); setFontSize(16); }
```

- [ ] **Step 4: Register in palette** — append to `COMMANDS`:

```js
  { label: 'Text: Larger (Cmd+=)', run: () => setFontSize((parseInt(localStorage.getItem('fontSize'), 10) || 16) + 1) },
  { label: 'Text: Smaller (Cmd+-)', run: () => setFontSize((parseInt(localStorage.getItem('fontSize'), 10) || 16) - 1) },
```

- [ ] **Step 5: Verify manually** — Cmd+=/− adjust size live in reader and live editor within 11–24px; Cmd+0 resets; line-height select applies and both survive an app restart; no clash with browser zoom (Tauri webview zoom is not bound by default).

- [ ] **Step 6: Commit**

```bash
git add gui/ui/index.html
git commit -m "GUI: Cmd+=/- font-size keys and line-height setting"
```

### Task 5: Print stylesheet polish + docs

**Files:**
- Modify: `gui/ui/index.html` (`@media print` block)
- Modify: `docs/gui/USER_GUIDE.md` (new features + shortcut list)
- Modify: `docs/gui/GUI.md` (manual test checklist section — create the section if absent)

**Interfaces:** none.

- [ ] **Step 1: Extend `@media print`** — replace the existing block with:

```css
  @media print {
    #sidebar, #statusbar, #error, #tabbar, #read-progress, #toast-stack { display: none !important; }
    #main, #scroll { overflow: visible !important; }
    body { height: auto; overflow: visible; }
    #content { max-width: none; padding: 0; font-size: 12pt; }
    #content pre, #content blockquote, #content table { break-inside: avoid; }
    #content h1, #content h2, #content h3 { break-after: avoid; }
    #content a { color: inherit; text-decoration: underline; }
  }
```

- [ ] **Step 2: Verify manually** — Cmd+K → "Print / Save as PDF" on a note with code, a table, and headings; PDF has no chrome, no mid-block page breaks in code/tables, full-width text.

- [ ] **Step 3: Update docs** — in `docs/gui/USER_GUIDE.md`, document: image lightbox, reading-progress bar, `?` cheat sheet, Cmd+=/−/0 text sizing, line-height setting, improved printing. In `docs/gui/GUI.md`, add (or extend) a "Manual test checklist" section with one line per verification step from Tasks 1–5 (copy the "Verify manually" bullets).

- [ ] **Step 4: Run Rust tests to confirm nothing regressed**

Run: `cargo test -p to_markdown_gui`
Expected: all tests pass (no Rust changed; this is a guard).

- [ ] **Step 5: Commit**

```bash
git add gui/ui/index.html docs/gui/USER_GUIDE.md docs/gui/GUI.md
git commit -m "GUI: print stylesheet polish; document Phase 1 polish features"
```

---

## Deferred to the Phase 2 plan

- CodeMirror 6 vendoring and the live-preview/source/split editor rework (spec Phase 2).
- ES-module extraction of `index.html` (blocked on classic-script global state; the editor rework is the natural moment).
- Backend `get_settings`/`set_settings` commands (superseded by localStorage — revisit only if settings must sync outside the webview).
