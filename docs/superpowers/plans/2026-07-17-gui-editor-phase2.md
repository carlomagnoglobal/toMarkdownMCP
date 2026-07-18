# GUI Phase 2: CodeMirror 6 Hybrid Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the GUI's textarea-based source editor and block-based live mode with a vendored CodeMirror 6 editor providing Typora/Obsidian-style live preview, source/split editing, wikilink autocomplete, and debounced autosave.

**Architecture:** A one-time prebuilt CodeMirror 6 IIFE bundle (built outside the repo, committed to `gui/ui/vendor/`) exposes the needed API on `window.CM`. New editor code lives in `gui/ui/index.html` alongside existing code. The three view modes keep their names (`read` / `live` / `split`) and entry points (`setViewMode`, Cmd+E cycle); `read` mode's server-side rendering pipeline is untouched. The old `liveBlocks` block-editing machinery is deleted once the CM6 live mode replaces it.

**Tech Stack:** Tauri 2 webview, vanilla JS, CodeMirror 6 (@codemirror/state, view, language, commands, lang-markdown, autocomplete), esbuild (used ONCE, outside the repo, to produce the vendored bundle).

## Global Constraints

- Frontend stays build-free vanilla JS: no npm, no bundler, no toolchain **in the repo or its dev/CI workflow** (spec "Hard constraint"). The CM6 bundle is built once in a scratch directory outside the repo and committed as a prebuilt minified file — exactly like `vendor/mermaid.min.js` and `vendor/katex/`.
- Third-party code only as vendored prebuilt files under `gui/ui/vendor/`, each with license text committed alongside.
- No blocking dialogs — failures use the existing `toast(msg, kind)` helper.
- Settings persist in `localStorage` (established Phase 1 deviation; no backend settings commands).
- No Rust changes except none are expected; `cargo test -p to_markdown_gui` must stay green after every task.
- Existing backend commands to reuse (do not add new ones): `save_file`, `read_source`, `render_blocks`, `quick_list`, `resolve_wikilink`.
- MSRV 1.88 untouched.

## Existing code you will build on (in `gui/ui/index.html`; find anchors by identifier, not line number)

- `let viewMode = 'read'` and `async function setViewMode(mode)` — mode switching; Cmd+E / Cmd+Shift+E bindings near `function enterEditMode`.
- `<textarea id="editor">` inside `#editor-wrap` — the current split-mode source editor; `saveNow()` calls `invoke('save_file', { path: currentFile, content: editor.value, vaultRoot: currentFolder })`.
- `#live-wrap` / `#live-doc`, `let liveBlocks = []`, `liveDoc`, `commitActiveBlock`, `saveLive` — the OLD block-based live mode, deleted in Task 5.
- `invoke('render_blocks', { source, path, vaultRoot })` → `[{ text, html }]` per top-level markdown block — reused by Task 4's widgets.
- `invoke('quick_list', { root })` → note names for the switcher — reused for wikilink autocomplete.
- `postRender(container)` — Mermaid/KaTeX/lightbox post-processing; call it on any HTML you inject.
- `applyTheme(name)` / `effectiveDark()` — theme switching; the editor must restyle when these run.
- `SHORTCUTS` cheat-sheet table and `COMMANDS` palette array — update when bindings change.

---

### Task 1: Build and vendor the CodeMirror 6 bundle

**Files:**
- Create: `gui/ui/vendor/codemirror.min.js`
- Create: `gui/ui/vendor/codemirror.LICENSE.txt`
- Modify: `gui/ui/index.html` (one `<script>` tag)

**Interfaces:**
- Produces: global `window.CM` with keys: `EditorState`, `EditorView`, `keymap`, `Decoration`, `WidgetType`, `ViewPlugin`, `StateField`, `StateEffect`, `Compartment`, `RangeSetBuilder`, `defaultKeymap`, `historyKeymap`, `history`, `indentWithTab`, `markdown`, `markdownLanguage`, `syntaxTree`, `syntaxHighlighting`, `HighlightStyle`, `tags`, `autocompletion`, `drawSelection`, `highlightActiveLine`.

- [ ] **Step 1: Build the bundle in a scratch directory (NOT in the repo)**

```bash
mkdir -p /tmp/cm6-bundle && cd /tmp/cm6-bundle
npm init -y
npm install esbuild @codemirror/state @codemirror/view @codemirror/language \
  @codemirror/commands @codemirror/lang-markdown @codemirror/autocomplete @lezer/highlight
cat > entry.js <<'EOF'
import { EditorState, StateField, StateEffect, Compartment, RangeSetBuilder } from "@codemirror/state";
import { EditorView, keymap, Decoration, WidgetType, ViewPlugin, drawSelection, highlightActiveLine } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { syntaxTree, syntaxHighlighting, HighlightStyle } from "@codemirror/language";
import { tags } from "@lezer/highlight";
import { autocompletion } from "@codemirror/autocomplete";
window.CM = { EditorState, StateField, StateEffect, Compartment, RangeSetBuilder,
  EditorView, keymap, Decoration, WidgetType, ViewPlugin, drawSelection, highlightActiveLine,
  defaultKeymap, history, historyKeymap, indentWithTab,
  markdown, markdownLanguage, syntaxTree, syntaxHighlighting, HighlightStyle, tags, autocompletion };
EOF
npx esbuild entry.js --bundle --minify --format=iife --outfile=codemirror.min.js
```

- [ ] **Step 2: Vendor it**

```bash
cp /tmp/cm6-bundle/codemirror.min.js /Users/elisjmendez/Documents/toMarkdownMCP/gui/ui/vendor/codemirror.min.js
cp /tmp/cm6-bundle/node_modules/@codemirror/view/LICENSE /Users/elisjmendez/Documents/toMarkdownMCP/gui/ui/vendor/codemirror.LICENSE.txt
```

Append to `codemirror.LICENSE.txt` a header line: `CodeMirror 6 (MIT) — bundled from @codemirror/* and @lezer/* packages; versions recorded below.` followed by the output of `npm ls --depth=0` from the scratch dir.

- [ ] **Step 3: Load it** — in `gui/ui/index.html` next to the mermaid script tag:

```html
<script src="vendor/codemirror.min.js"></script>
```

- [ ] **Step 4: Smoke-verify** — temporarily add at the end of the main script: `console.log('CM keys', Object.keys(window.CM).length);` run `cd gui && cargo tauri dev`, confirm ≥ 20 keys logged and no console errors, then REMOVE the temporary line. If no interactive run is possible, verify instead that `grep -c 'window.CM' gui/ui/vendor/codemirror.min.js` ≥ 1 and the file is non-trivial (`wc -c` > 300000).

- [ ] **Step 5: Commit**

```bash
git add gui/ui/vendor/codemirror.min.js gui/ui/vendor/codemirror.LICENSE.txt gui/ui/index.html
git commit -m "GUI: vendor prebuilt CodeMirror 6 bundle (window.CM)"
```

### Task 2: CM6 source editor replaces the textarea (split mode)

**Files:**
- Modify: `gui/ui/index.html`

**Interfaces:**
- Consumes: `window.CM`, `#editor-wrap`, `saveNow()`, `setViewMode`, `applyTheme`.
- Produces: `cmView` (the EditorView instance), `cmGetDoc()`, `cmSetDoc(text)`, `cmThemeCompartment`, `cmRefreshTheme()` — used by Tasks 3–5.

- [ ] **Step 1: Add editor container + CSS** — inside `#editor-wrap`, after the `<textarea id="editor">` element:

```html
<div id="cm-editor"></div>
```

```css
  #cm-editor { flex: 1; min-width: 0; overflow: hidden; display: flex; }
  #cm-editor .cm-editor { flex: 1; font: 13.5px ui-monospace, SFMono-Regular, Menlo, monospace; }
  #cm-editor .cm-editor.cm-focused { outline: none; }
  #editor { display: none; }  /* superseded by CM6; element kept until Task 5 cleanup */
```

- [ ] **Step 2: Create the editor** — new JS section after the `const editor = document.getElementById('editor');` block:

```js
// ---- CodeMirror 6 source editor ----
const cmThemeCompartment = new CM.Compartment();
function cmHighlightStyle() {
  const t = CM.tags;
  return CM.HighlightStyle.define([
    { tag: t.heading, fontWeight: '700' },
    { tag: t.strong, fontWeight: '700' },
    { tag: t.emphasis, fontStyle: 'italic' },
    { tag: t.strikethrough, textDecoration: 'line-through' },
    { tag: t.link, color: effectiveDark() ? '#7aa2f7' : '#2a6bcc' },
    { tag: t.monospace, color: effectiveDark() ? '#e0af68' : '#8a5a00' },
    { tag: t.quote, color: effectiveDark() ? '#9aa5ce' : '#5a6472', fontStyle: 'italic' },
  ]);
}
function cmThemeExt() {
  return [
    CM.EditorView.theme({}, { dark: effectiveDark() }),
    CM.syntaxHighlighting(cmHighlightStyle()),
  ];
}
let cmView = null;
function cmEnsure() {
  if (cmView) return cmView;
  cmView = new CM.EditorView({
    parent: document.getElementById('cm-editor'),
    state: CM.EditorState.create({
      doc: '',
      extensions: [
        CM.history(),
        CM.drawSelection(),
        CM.highlightActiveLine(),
        CM.keymap.of([...CM.defaultKeymap, ...CM.historyKeymap, CM.indentWithTab]),
        CM.markdown({ base: CM.markdownLanguage }),
        cmThemeCompartment.of(cmThemeExt()),
        CM.EditorView.lineWrapping,
        CM.EditorView.updateListener.of((u) => { if (u.docChanged) cmDirty(); }),
      ],
    }),
  });
  return cmView;
}
function cmGetDoc() { return cmView ? cmView.state.doc.toString() : ''; }
function cmSetDoc(text) {
  cmEnsure();
  cmView.dispatch({ changes: { from: 0, to: cmView.state.doc.length, insert: text } });
}
function cmRefreshTheme() {
  if (cmView) cmView.dispatch({ effects: cmThemeCompartment.reconfigure(cmThemeExt()) });
}
```

- [ ] **Step 3: Wire into split mode** — in `setViewMode`, where split mode currently loads `read_source` into `editor.value`, additionally/instead do `cmSetDoc(sourceText)` and focus with `cmView.focus()`. Change `saveNow()` to read `cmGetDoc()` instead of `editor.value`. `cmDirty()` is a small debounce marker used fully in Task 5; for now define:

```js
let cmDirtyFlag = false;
function cmDirty() { cmDirtyFlag = true; }
```

and call `cmDirtyFlag = false;` inside `saveNow()` after a successful save. Keep the existing Cmd+S binding calling `saveNow()`.

- [ ] **Step 4: Theme hook** — at the end of `applyTheme(name)`, add `cmRefreshTheme();`.

- [ ] **Step 5: Verify** — `cargo tauri dev`: open a note, Cmd+Shift+E → split shows CM6 editor with markdown highlighting; type, Cmd+S saves (check file on disk); switch themes → editor colors update; undo/redo (Cmd+Z / Cmd+Shift+Z) work. Headless fallback: re-read inserted code in context; `cargo test -p to_markdown_gui` green.

- [ ] **Step 6: Commit**

```bash
git add gui/ui/index.html
git commit -m "GUI: CodeMirror 6 source editor replaces textarea in split mode"
```

### Task 3: Live-preview decorations (Typora-style)

**Files:**
- Modify: `gui/ui/index.html`

**Interfaces:**
- Consumes: `cmView`, `cmEnsure`, `CM.ViewPlugin`, `CM.Decoration`, `CM.syntaxTree`, `CM.Compartment`.
- Produces: `cmLiveCompartment`, `cmSetLive(on)` — Task 5 wires these into `setViewMode('live')`.

- [ ] **Step 1: Add the live-preview plugin** — after the Task 2 section:

```js
// ---- Live preview: hide formatting marks except on selection lines ----
const HIDE_NODES = new Set(['HeaderMark', 'EmphasisMark', 'StrongEmphasis', 'CodeMark', 'QuoteMark', 'LinkMark', 'URL']);
const cmLiveCompartment = new CM.Compartment();
const livePreviewPlugin = CM.ViewPlugin.fromClass(class {
  constructor(view) { this.decorations = this.build(view); }
  update(u) { if (u.docChanged || u.selectionSet || u.viewportChanged) this.decorations = this.build(u.view); }
  build(view) {
    const b = new CM.RangeSetBuilder();
    const sel = view.state.selection.main;
    const activeFrom = view.state.doc.lineAt(sel.from).from;
    const activeTo = view.state.doc.lineAt(sel.to).to;
    for (const { from, to } of view.visibleRanges) {
      CM.syntaxTree(view.state).iterate({ from, to, enter: (node) => {
        if (!HIDE_NODES.has(node.name)) return;
        if (node.name === 'StrongEmphasis' || node.name === 'URL') return; // containers/link URLs handled via their marks
        if (node.to >= activeFrom && node.from <= activeTo) return; // reveal on active line(s)
        b.add(node.from, node.to, CM.Decoration.replace({}));
      } });
    }
    return b.finish();
  }
}, { decorations: (v) => v.decorations });
function cmSetLive(on) {
  cmEnsure();
  cmView.dispatch({ effects: cmLiveCompartment.reconfigure(on ? [livePreviewPlugin, liveTypography] : []) });
}
```

- [ ] **Step 2: Live typography theme** — larger headings and prose font when in live mode (source mode stays monospace):

```js
const liveTypography = CM.EditorView.theme({
  '&': { fontFamily: 'inherit' },
  '.cm-content': { fontFamily: 'inherit', maxWidth: '68ch', margin: '0 auto', padding: '32px 8px 40vh' },
});
```

- [ ] **Step 3: Register the compartment** — add `cmLiveCompartment.of([])` to the extensions array in `cmEnsure()` (Task 2's create call).

- [ ] **Step 4: Temporary test hook** — append `{ label: 'DEV: Toggle Live Decorations', run: () => { window._lp = !window._lp; cmSetLive(window._lp); } }` to `COMMANDS` so the mode is testable before Task 5 rewires `setViewMode`. (Removed in Task 5.)

- [ ] **Step 5: Verify** — in split mode, run the DEV toggle: `#` marks on headings vanish except on the cursor's line; bold/italic/code marks hide likewise; moving the cursor onto a line reveals its syntax; editing stays responsive on a 1,000-line note. Headless fallback: static re-read; `cargo test -p to_markdown_gui`.

- [ ] **Step 6: Commit**

```bash
git add gui/ui/index.html
git commit -m "GUI: live-preview decorations (syntax hidden except active line)"
```

### Task 4: Inline block widgets (Mermaid, KaTeX math blocks, images)

**Files:**
- Modify: `gui/ui/index.html`

**Interfaces:**
- Consumes: `livePreviewPlugin` area, `CM.WidgetType`, `postRender`, `invoke('render_blocks', …)`.
- Produces: widget rendering active whenever live mode is on (no new API; extends `cmSetLive`'s extension list).

- [ ] **Step 1: Widget class** — after the Task 3 section:

```js
// ---- Live preview block widgets (mermaid / math / images) ----
class HtmlBlockWidget extends CM.WidgetType {
  constructor(html, key) { super(); this.html = html; this.key = key; }
  eq(other) { return other.key === this.key; }
  toDOM() {
    const div = document.createElement('div');
    div.className = 'cm-block-widget';
    div.innerHTML = this.html;
    postRender(div);
    return div;
  }
  ignoreEvent() { return false; }
}
```

```css
  .cm-block-widget { padding: 4px 0; }
  .cm-block-widget img { max-width: 100%; }
```

- [ ] **Step 2: Widget state field** — rendered HTML comes from the backend per block; cache by block text:

```js
const widgetCache = new Map(); // block text -> html
async function refreshWidgetCache() {
  if (!currentFile || !cmView) return;
  try {
    const blocks = await invoke('render_blocks', { source: cmGetDoc(), path: currentFile, vaultRoot: currentFolder });
    widgetCache.clear();
    for (const b of blocks) widgetCache.set(b.text.trim(), b.html);
    cmView.dispatch({ effects: widgetRefreshEffect.of(null) });
  } catch (e) { /* rendering is best-effort; source stays editable */ }
}
const widgetRefreshEffect = CM.StateEffect.define();
```

- [ ] **Step 3: Decorate widget blocks** — extend `livePreviewPlugin.build` with a second pass over the syntax tree: for nodes named `FencedCode` whose info string is `mermaid`, `Image` lines, and `$$…$$` math blocks (node `BlockMath` if present in the tree; otherwise detect fenced lines starting/ending with `$$` textually), when the block does NOT intersect the active selection lines and `widgetCache` has its exact trimmed text, add:

```js
b.add(node.from, node.to, CM.Decoration.replace({ widget: new HtmlBlockWidget(widgetCache.get(text), text), block: true }));
```

(where `text = view.state.doc.sliceString(node.from, node.to).trim()`). Also make the plugin's `update` respond to `widgetRefreshEffect` (`u.transactions.some(tr => tr.effects.some(e => e.is(widgetRefreshEffect)))`).

- [ ] **Step 4: Refresh triggers** — call `refreshWidgetCache()` when live mode turns on (`cmSetLive(true)`) and debounced 700 ms after doc changes while live (extend `cmDirty()`):

```js
let widgetTimer = null;
function cmDirty() {
  cmDirtyFlag = true;
  if (viewMode === 'live') { clearTimeout(widgetTimer); widgetTimer = setTimeout(refreshWidgetCache, 700); }
}
```

- [ ] **Step 5: Verify** — in live mode a ```mermaid fence renders as a diagram; a `$$…$$` block renders via KaTeX; an image line shows the image; clicking a widget (or moving the cursor into its range with arrow keys) reveals the source; editing the source re-renders ~0.7 s after typing stops. Headless fallback: static re-read; cargo tests green.

- [ ] **Step 6: Commit**

```bash
git add gui/ui/index.html
git commit -m "GUI: live-preview inline widgets for mermaid, math, and images"
```

### Task 5: Mode rewire, autosave, wikilink autocomplete, legacy cleanup

**Files:**
- Modify: `gui/ui/index.html`

**Interfaces:**
- Consumes: everything above plus `setViewMode`, `quick_list`, `resolve_wikilink`, `SHORTCUTS`, `COMMANDS`.
- Produces: final mode behavior — `read` (unchanged), `live` (CM6 + decorations + widgets), `split` (CM6 source + reader pane).

- [ ] **Step 1: Rewire `setViewMode('live')`** — replace the old block-based live mode body: show `#editor-wrap`'s CM container full-width (hide the split reader pane), `cmSetDoc(await invoke('read_source', {path}))` if not already loaded for this file, `cmSetLive(true)`, focus. `'split'`: `cmSetLive(false)`, show reader pane beside the editor. `'read'`: if `cmDirtyFlag`, `await saveNow()` first, then re-render via the existing pipeline. Remove the DEV toggle from `COMMANDS`.

- [ ] **Step 2: Debounced autosave** — extend `cmDirty()`:

```js
let autosaveTimer = null;
function cmDirty() {
  cmDirtyFlag = true;
  clearTimeout(autosaveTimer); autosaveTimer = setTimeout(() => { if (cmDirtyFlag) saveNow(); }, 1200);
  if (viewMode === 'live') { clearTimeout(widgetTimer); widgetTimer = setTimeout(refreshWidgetCache, 700); }
}
```

`saveNow()` shows no toast on autosave success (only on failure) to avoid noise; keep the explicit-Cmd+S success flash if one exists today.

- [ ] **Step 3: Wikilink autocomplete** — add to `cmEnsure()` extensions:

```js
CM.autocompletion({ override: [async (ctx) => {
  const m = ctx.matchBefore(/\[\[[^\]]*/);
  if (!m) return null;
  const q = m.text.slice(2).toLowerCase();
  try {
    const notes = await invoke('quick_list', { root: currentFolder });
    return { from: m.from + 2,
      options: notes.filter(n => n.toLowerCase().includes(q)).slice(0, 20)
        .map(n => ({ label: n, apply: n + ']]' })) };
  } catch { return null; }
}] }),
```

- [ ] **Step 4: Delete legacy block-live code** — remove `liveBlocks`, `liveDoc`, `commitActiveBlock`, `saveLive`, the `#live-wrap`/`#live-doc` markup and their CSS, the block `<textarea>` editing handlers, and the now-dead `<textarea id="editor">` element plus its `#editor { display:none }` rule. Search for every reference (`grep -n 'liveBlocks\|liveDoc\|live-doc\|live-wrap\|commitActiveBlock\|saveLive' gui/ui/index.html`) and either delete or reroute to the CM6 equivalents; typewriter mode's scroll-centering now targets the CM editor (`cmView.scrollDOM`).

- [ ] **Step 4b: Split-mode scroll sync** — preserve (or add, if the old textarea sync dies with it) proportional scroll sync from editor to the split reader pane:

```js
cmEnsure();
cmView.scrollDOM.addEventListener('scroll', () => {
  if (viewMode !== 'split') return;
  const src = cmView.scrollDOM, dst = document.getElementById('edit-preview');
  const max = src.scrollHeight - src.clientHeight;
  if (max > 0) dst.scrollTop = (src.scrollTop / max) * (dst.scrollHeight - dst.clientHeight);
}, { passive: true });
```

(If the existing split preview element has a different id than `edit-preview`, use the actual one — check the current split-mode markup.)

- [ ] **Step 5: Update SHORTCUTS + COMMANDS + settings** — cheat-sheet and palette entries still say Live/Split with the same keys (verify wording); the settings' "open in live editing by default" checkbox (`liveDefaultOn`) now routes to the CM6 live mode — verify the auto-live path in `openFile` still works.

- [ ] **Step 6: Verify (full pass)** — Cmd+E cycles read → live → split → read; live mode: type prose, syntax hides off-cursor, mermaid/math/image widgets render, `[[` pops autocomplete that inserts `name]]`; autosave writes ~1.2 s after typing stops (check disk); split: monospace source + reader pane; read: rendered view reflects saved edits; no console errors; `grep -c liveBlocks gui/ui/index.html` = 0. Run `cargo test -p to_markdown_gui` — green.

- [ ] **Step 7: Commit**

```bash
git add gui/ui/index.html
git commit -m "GUI: CM6 live mode replaces block editor; autosave + wikilink autocomplete"
```

### Task 6: Documentation

**Files:**
- Modify: `docs/gui/USER_GUIDE.md`, `docs/gui/GUI.md`, `CHANGELOG.md`

- [ ] **Step 1: USER_GUIDE.md** — rewrite the editing section: three modes and what each is for, live-preview behavior (syntax reveals at cursor; widgets for diagrams/math/images), autosave, `[[` autocomplete, Cmd+E / Cmd+Shift+E / Cmd+S.
- [ ] **Step 2: GUI.md** — extend the manual test checklist with the Task 5 Step 6 verification list, one line each. Note the vendored CM6 bundle and how to rebuild it (point at this plan's Task 1).
- [ ] **Step 3: CHANGELOG.md** — add an Unreleased entry summarizing Phase 2.
- [ ] **Step 4: Run `cargo test -p to_markdown_gui`** — expected green (guard).
- [ ] **Step 5: Commit**

```bash
git add docs/gui/USER_GUIDE.md docs/gui/GUI.md CHANGELOG.md
git commit -m "GUI: document CM6 hybrid editing modes; extend manual checklist"
```

---

## Notes for the executor

- Tasks are strictly sequential (each builds on the previous editor state).
- Tasks 3 and 4 contain the real engineering risk. The syntax-tree node names (`HeaderMark`, `EmphasisMark`, `CodeMark`, `QuoteMark`, `LinkMark`) come from @lezer/markdown; verify actual names at runtime with a temporary `console.log(node.name)` during development if decorations don't appear — adjust the `HIDE_NODES` set to the observed names, that is expected tuning, not a spec deviation.
- If the vendored bundle's minified size exceeds ~1.5 MB, that is acceptable (mermaid.min.js is comparable); do not attempt tree-shaking heroics.
- Live-preview interactive verification matters more here than in Phase 1: if the executor environment cannot run `cargo tauri dev`, flag every skipped verify step in the report so the final review lists them for a human pass.
