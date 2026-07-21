# Vault Viewer Enhancements Design
**Date:** 2026-07-20  
**Status:** Approved  
**Scope:** Multi-tab file viewer with indexing, search, and minimal code editing

---

## 1. Overview

This design enhances the toMarkdownMCP GUI from a markdown-only viewer into a comprehensive **vault explorer** supporting multiple file types, intelligent indexing, and quick navigation.

**Core goals:**
- View markdown, code, images, and binary files in appropriate formats
- Index vault contents for fast search and relationship detection
- Enable file operations (duplicate, create notes) via context menu
- Provide minimal code editing with undo/redo and find/replace
- Persist open tabs and show recent files for quick navigation

---

## 2. Architecture

### 2.1 Polymorphic Tab Viewer System

The enhancement uses a **trait-based architecture** where each file type has a dedicated viewer:

```rust
trait FileViewer {
    fn render(&self) -> String;  // HTML or raw content
    fn handle_input(&mut self, event: InputEvent);
    fn get_state(&self) -> ViewerState;
}

impl FileViewer for MarkdownViewer { ... }
impl FileViewer for CodeViewer { ... }
impl FileViewer for ImageViewer { ... }
impl FileViewer for HexViewer { ... }
```

**File type detection** (priority order):
1. Check file extension
2. Read magic bytes for ambiguous files
3. Default to Hex viewer

**Tab states:**
```rust
enum TabType {
    Markdown { path, render_opts },
    Code { path, language, content, dirty, cursor_pos },
    Image { path, scale, fit_mode },
    Hex { path, offset, bytes_per_line },
}
```

### 2.2 Unified SQLite Vault Database

Each vault has a single `.tomarkdown/vault.db` containing:
- File metadata (name, extension, type, language, size, modified time)
- File relationships (imports, wikilinks, includes)
- Word co-occurrence graph (consolidated from v0.5.0)
- Index state and metadata

---

## 3. File Viewers

### 3.1 Markdown Viewer
- **Extensions:** `.md`, `.markdown`
- **Behavior:** Existing implementation, no changes
- **Features:** Rendered HTML, word/char/read-time stats

### 3.2 Code Viewer (NEW)

**Supported languages:**

| Category | Extensions |
|----------|-----------|
| Web | `.js`, `.ts`, `.jsx`, `.tsx`, `.html`, `.css`, `.scss`, `.less`, `.vue`, `.svelte` |
| Python | `.py`, `.pyx`, `.pyi`, `.ipynb` |
| Rust | `.rs` |
| Go | `.go` |
| C/C++ | `.c`, `.cpp`, `.cc`, `.cxx`, `.h`, `.hpp`, `.h++` |
| Java/JVM | `.java`, `.kt`, `.scala` |
| C#/.NET | `.cs`, `.csx`, `.fsx`, `.fs` |
| PHP | `.php`, `.phtml` |
| Ruby | `.rb`, `.erb` |
| Shell | `.sh`, `.bash`, `.zsh`, `.fish` |
| Config/Data | `.json`, `.yaml`, `.yml`, `.toml`, `.xml`, `.ini`, `.env`, `.properties`, `.sql`, `.sqlite` |
| Text | `.txt` |

**Features:**
- Syntax highlighting via `syntect` (auto-detect language from extension)
- Read-only by default, toggle edit mode via button
- Ultra-minimal editor:
  - Type/delete text
  - Undo/redo (with history limit)
  - Find/replace (Ctrl+F, Ctrl+H)
  - Auto-save (configurable, 2-second delay after inactivity)
- Line numbers on left margin
- Preserve indentation on newlines
- Inline syntax error highlighting (red squiggle, error dot in margin)
- Save indicator in tab (• for unsaved)
- Large file handling (>10MB): warn, disable editing

**Error detection:**
- Python: `pylint`/`flake8` if available
- JavaScript/TypeScript: `eslint` if available
- Rust: lightweight `rustc` check
- JSON/YAML/TOML: parse validation
- Graceful fallback if linter not installed

**Auto-save:**
- Toggle on/off per tab
- Default: OFF (require manual save)
- When enabled: save after 2 seconds of inactivity
- Show "Saved ✓" toast on successful auto-save

### 3.3 Image Viewer (NEW)

**Supported formats:** PNG, JPG, JPEG, GIF, SVG, WebP, AVIF, TIFF

**Features:**
- Display at natural size or fit to window
- Zoom in/out (mouse wheel, +/- buttons)
- Pan/drag to navigate zoomed images
- Show file info: dimensions, file size, format
- Lazy-load high-res images (show thumbnail first)
- Large image handling (>100MB): warn, load reduced resolution

### 3.4 Hex Viewer (NEW)

**File types:** Any unrecognized binary or text file

**Features:**
- Hexadecimal format: 16 bytes per line (standard hex dump style)
- ASCII representation on right side
- Scrollable for large files
- Read-only (no editing)
- Large file handling (>10MB): lazy-load in chunks

---

## 4. Unified SQLite Vault Database

### 4.1 Schema

```sql
-- File metadata
CREATE TABLE files (
  id INTEGER PRIMARY KEY,
  path TEXT UNIQUE NOT NULL,
  name TEXT NOT NULL,
  extension TEXT,
  file_type TEXT, -- 'markdown', 'code', 'image', 'hex'
  language TEXT, -- 'python', 'rust', 'javascript', etc (for code)
  size_bytes INTEGER,
  modified_at INTEGER, -- Unix timestamp
  last_indexed_at INTEGER,
  is_indexed BOOLEAN DEFAULT 0
);

-- File relationships (imports, links, wikilinks)
CREATE TABLE file_links (
  id INTEGER PRIMARY KEY,
  source_id INTEGER NOT NULL,
  target_id INTEGER,
  link_type TEXT, -- 'import', 'wikilink', 'reference', 'include'
  FOREIGN KEY (source_id) REFERENCES files(id),
  FOREIGN KEY (target_id) REFERENCES files(id)
);

-- Word co-occurrence graph (unified from v0.5.0)
CREATE TABLE word_graph (
  id INTEGER PRIMARY KEY,
  word1 TEXT NOT NULL,
  word2 TEXT NOT NULL,
  co_occurrence_count INTEGER DEFAULT 1,
  last_updated INTEGER
);

-- Index state
CREATE TABLE index_state (
  id INTEGER PRIMARY KEY,
  last_full_index INTEGER,
  last_incremental_index INTEGER,
  total_files_indexed INTEGER
);

-- Recently opened files (for quick access)
CREATE TABLE recent_files (
  id INTEGER PRIMARY KEY,
  file_id INTEGER NOT NULL,
  opened_at INTEGER NOT NULL,
  FOREIGN KEY (file_id) REFERENCES files(id),
  UNIQUE(file_id)
);

-- Full-text search index (for file content search)
CREATE VIRTUAL TABLE files_fts USING fts5(path, name, content, language);

-- Indexes for performance
CREATE INDEX idx_files_extension ON files(extension);
CREATE INDEX idx_files_language ON files(language);
CREATE INDEX idx_file_links_source ON file_links(source_id);
CREATE INDEX idx_file_links_target ON file_links(target_id);
CREATE INDEX idx_recent_files_opened ON recent_files(opened_at DESC);
```

### 4.2 Indexing Strategy

**Lazy indexing:** Index files on-demand as they're opened, not all at once (avoids startup lag)

**Incremental indexing:** Only re-index files modified since last index

**Relationship detection:**
- Python imports: `import X`, `from X import Y`
- Rust uses: `use X::Y`
- JavaScript imports: `import`, `require`
- Wikilinks: `[[path/to/file]]`
- Generic references: `[link text](path)`

**On file open:** Auto-update `files` table entry and `recent_files` table

**Performance:** Large vault (>10k files) → prompt user to narrow scope or enable selective indexing

---

## 5. File Operations & Context Menu

### 5.1 File Duplication

**Trigger:** Right-click on file in tree or viewer pane → "Duplicate"

**Behavior:**
- Create copy in same folder with name `filename copy.ext`
- If `filename copy.ext` exists, use `filename copy (2).ext`, etc.
- Works for any file type (not just markdown)
- Refresh tree view to show new file
- Add new file to SQLite `files` table
- Log operation

### 5.2 Create Markdown Note

**Trigger:** Right-click on any file or folder in tree → "Create MD Note"

**Behavior:**
- Open filename input dialog (default: "New Note.md")
- Create file in same folder or selected folder
- Add to SQLite `files` table
- Open in new markdown tab automatically
- Show markdown editor ready for typing
- Add to `recent_files` table

---

## 6. Tab System

### 6.1 Tab Display

Each tab shows:
- **Icon:** 📄 (markdown), <> (code), 🖼️ (image), 🔢 (hex)
- **Label:** Filename
- **Dirty indicator:** • (dot) if unsaved changes (code editor only)
- **Close button:** ✕

### 6.2 Tab Persistence

**On app close:** Save list of open tabs to `.tomarkdown/tabs.json` (limit to last 5 tabs)

```json
{
  "vault_root": "/Users/elisjmendez/vault",
  "active_tab_index": 0,
  "tabs": [
    { "path": "/path/to/file1.md", "type": "markdown" },
    { "path": "/path/to/file2.rs", "type": "code" },
    { "path": "/path/to/image.png", "type": "image" }
  ]
}
```

**On app start:** Restore tabs if vault is the same

**Error handling:** If file no longer exists, skip it (don't error)

---

## 7. Search & Filter

### 7.1 Vault-Wide Search

**UI:** Input field above file tree with toggles:
```
[Search files...] 📄 (name) | 🔍 (content)
```

**Behavior:**
- **Search by name:** Instant filtering (filters tree as user types)
- **Search by content:** Uses FTS, shows results as list, click to open file + highlight match
- Real-time search
- Clear button to reset to full tree view

**Implementation:** SQLite FTS5 for performance on large vaults

---

## 8. File Preview on Hover

**Behavior:** Hover over tree item → show preview after 500ms delay

**Preview content by type:**
- **Markdown:** First 200 chars of rendered content
- **Images:** Thumbnail (max 200x200px)
- **Code:** First 10 lines with syntax highlighting
- **Other:** File info (size, type, modified date, language)

**Implementation:** Lazy-load preview (don't read file until hover occurs)

---

## 9. Quick Navigation Panel

**New sidebar section:** "Recent Files" (below file tree)
- Shows last **10 recently opened files** (sorted by open time)
- Click to open file
- Icons show file type
- Toggle visibility via button
- Stored in SQLite `recent_files` table

**On file open:** Update `opened_at` timestamp (move to top of list)

---

## 10. Error Handling & Recovery

**File operations:**
- Duplicate fails → show error toast, log error
- Create note fails → show error dialog with reason
- Open file fails → show error, close tab
- Save fails → show error, keep dirty flag, offer retry

**SQLite errors:**
- Database locked → retry with exponential backoff (up to 3 times)
- Corruption detected → log error, fallback to re-index
- Index out of sync → automatic re-index on vault load

**Large file handling:**
- Code >10MB: warn user, load read-only (no editing)
- Images >100MB: load reduced resolution, show warning
- Hex viewer: lazy-load in chunks (show first 1MB)

**Concurrent access:**
- File modified externally while open → prompt to reload or keep current
- File deleted by another app → show error, mark tab as deleted

---

## 11. Performance Considerations

**Optimization strategies:**
- Lazy indexing: index on-demand, not upfront
- Incremental indexing: only re-index changed files
- SQLite indexes: on `extension`, `language`, `file_type`
- Syntax highlighting: cache tokens per file
- Image rendering: lazy-load full images, show thumbnail first
- Large vaults (>10k files): prompt user to narrow scope or enable selective indexing

---

## 12. Logging & Debugging

**Output:** Console (stdout) + rotating file logs

**File location:** `.tomarkdown/logs/vault.log`

**Retention:** Keep last 5 days of logs (older logs deleted automatically)

**Rotation:** New log file per day (e.g., `vault-2026-07-20.log`)

**Log level:** INFO (normal ops), DEBUG (detailed), ERROR (failures)

**Important functions to log:**
- `open_file(path)` → file type, viewer created
- `duplicate_file(path)` → source, destination, success/error
- `create_markdown_note(name, location)` → file created, tab opened
- `index_vault()` → start, files scanned, relationships found, duration
- `detect_language(path)` → detected language
- `scan_file_links(path)` → found imports/wikilinks
- `update_word_graph(content)` → words added
- `read_file(path)` → size, encoding
- `save_file(path, content)` → bytes written, dirty cleared
- `undo/redo operations` → state changes
- `tab_opened(path)` → tab created, type
- `right_click_menu(action)` → action triggered
- `find_replace(query, replacement)` → matches found

**Log format:**
```
[2026-07-20 14:23:45] INFO: Vault indexing started for /Users/elisjmendez/vault
[2026-07-20 14:23:46] DEBUG: Detected language 'rust' for main.rs
[2026-07-20 14:23:47] DEBUG: Found 42 wikilinks in notes/index.md
[2026-07-20 14:23:50] INFO: Indexing complete: 127 files scanned, 856 links found in 4.2s
[2026-07-20 14:24:12] ERROR: Failed to duplicate file: Permission denied (os error 13)
```

---

## 13. Testing Strategy

### 13.1 Unit Tests (Rust Backend)

- File type detection: each format (extension, magic bytes, fallback to hex)
- File duplication: naming pattern, error handling
- MD note creation: file creation, naming validation
- File operations: permissions, disk errors
- SQLite operations: CRUD, FTS queries, index maintenance
- Language detection: all supported file types
- Link detection: imports, wikilinks, references
- Auto-save logic: timer, debouncing, dirty flag

### 13.2 Integration Tests (Tauri + Frontend)

- Open each file type → renders correctly
- Syntax highlighting → displays properly
- Code edit/save → content persists
- Undo/redo → works in code editor
- Find/replace → finds and replaces correctly
- Image zoom/pan → works smoothly
- Hex viewer → displays binary data
- Right-click duplicate → new file in tree
- Right-click create note → new tab opens
- Tab switching → state preserved
- Tab persistence → tabs restore on app start
- Search by name → instant filtering works
- Search by content → FTS finds matches
- Hover preview → shows content
- Recent files list → updates on file open
- Auto-save → saves after inactivity
- Large files → load without crashing
- Error cases: permissions, missing files, corrupted data

### 13.3 Manual Testing Checklist

- [ ] Each file type opens in correct tab
- [ ] Icons display correctly per type
- [ ] Dirty indicators show/hide properly
- [ ] Save and undo work in code editor
- [ ] Large files don't crash (hex, images)
- [ ] Right-click menu works in tree and viewer
- [ ] Tab persistence works (close/reopen app)
- [ ] Search finds files by name and content
- [ ] Preview on hover shows content
- [ ] Recent files list updates
- [ ] Auto-save enabled/disabled toggle works
- [ ] Syntax errors highlighted in code editor
- [ ] Logging captures important events

---

## 14. Edge Cases & Constraints

**File system:**
- Symlinks: follow symlinks, log warning if cycles detected
- Hidden files: exclude from tree (existing behavior)
- Special characters: handle URL encoding
- Long paths (>260 Windows, >4096 Linux): warn user

**Encoding:**
- Non-UTF8 code files: detect encoding, display warning
- Binary files as text: detect and show hex view
- Mixed line endings (CRLF/LF): preserve on save

**Concurrent access:**
- File modified externally: prompt to reload or keep current
- File deleted: show error, mark tab as deleted
- Database locked: retry with backoff

---

## 15. Implementation Priority

**Phase 1 (Core):**
- Tab system enhancements (markdown, code, image, hex viewers)
- File type detection
- Basic SQLite schema and lazy indexing
- Duplicate file and create note operations
- Right-click context menu

**Phase 2 (Polish):**
- Code editor (minimal editing, syntax highlighting, auto-save)
- Syntax error highlighting
- Tab persistence
- Logging and error handling

**Phase 3 (Advanced):**
- Search/filter (FTS)
- File preview on hover
- Quick navigation (recent files sidebar)
- Performance optimization for large vaults

---

## 16. Success Criteria

✅ Users can view markdown, code, images, and binary files in appropriate formats  
✅ File duplication works for any file type with correct naming  
✅ Markdown notes can be created via right-click in new tabs  
✅ Code files support minimal editing with undo/redo/find/replace  
✅ Syntax errors highlighted inline in code editor  
✅ Auto-save works when enabled  
✅ Tab state persists across sessions (last 5 tabs)  
✅ SQLite indexes vault for fast search and relationship detection  
✅ Logging captures important operations for debugging  
✅ Large vaults (>10k files) handle gracefully without lag  
✅ All edge cases (permissions, encoding, concurrent access) handled gracefully
