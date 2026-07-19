# Word Graph View with Co-Occurrence Indexing — Design

**Date:** 2026-07-18  
**Component:** `gui/` (toMarkdown Viewer, Tauri 2)  
**Goal:** Add a third graph-view mode (alongside Global and Current Note) that displays word co-occurrence relationships to help users discover super-links based on shared vocabulary patterns.

## Overview

The Word graph visualizes relationships between words in the vault based on co-occurrence (words appearing together in notes). Word nodes are sized by frequency and colored by associated notes; edges represent co-occurrence strength. A SQLite database maintains a persistent index with incremental updates on app startup and when the user opens the Word view (if files have changed). All graph views (Global, Current Note, Word) support independent zoom via mouse wheel, touch pinch, keyboard, and ±/Reset buttons.

## Architecture

### Components

- **Database layer** (Rust): SQLite schema for words, co-occurrence pairs, and indexing state
- **Indexing engine** (Rust): Tokenize vault text, compute word frequency and co-occurrence pairs, perform incremental delta updates
- **Graph renderer** (Canvas 2D): Reuse existing force-directed physics simulation, adapted for word nodes
- **UI controls** (HTML/CSS): Third toggle button for Word view, zoom controls, "Last updated" timestamp
- **Interaction layer** (JavaScript): Word node click → highlight related notes; per-view zoom state

### Data Flow

1. **App startup**: Check SQLite index freshness; if stale/missing, perform full vault indexing in background thread
2. **User opens Word tab**: Check file watcher for changed files; if found, delta-index changed files into SQLite
3. **Graph render**: Query SQLite for top-N words and co-occurrence pairs; build node/link arrays; render with Canvas physics
4. **User interaction**: Click word → query SQLite for co-occurrence partners and notes; highlight in UI
5. **Zoom**: Per-view zoom state maintained locally; Canvas scale factor updated on wheel/pinch/key events

## SQLite Schema and Indexing

### Tables

```sql
CREATE TABLE words (
  id INTEGER PRIMARY KEY,
  word TEXT UNIQUE NOT NULL,
  frequency INTEGER NOT NULL,  -- count across entire vault
  last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE co_occurrence (
  word1_id INTEGER NOT NULL,
  word2_id INTEGER NOT NULL,
  count INTEGER NOT NULL,      -- co-occurrence count (notes containing both)
  notes_list_json TEXT,        -- JSON: ["note1.md", "note2.md", ...]
  UNIQUE(word1_id, word2_id),
  FOREIGN KEY(word1_id) REFERENCES words(id),
  FOREIGN KEY(word2_id) REFERENCES words(id)
);

CREATE TABLE index_state (
  vault_path TEXT PRIMARY KEY,
  last_full_index TIMESTAMP,
  changed_files_since TEXT      -- JSON: list of changed file paths
);
```

### Indexes

- Primary key on `words.id`
- Unique constraint on `co_occurrence(word1_id, word2_id)` to prevent duplicates
- Index on `words.frequency` for top-N queries
- Index on `index_state.vault_path` for quick state lookup

### Indexing Strategy

**On app startup**:
- Check if SQLite index exists for current vault path
- If missing or older than 24 hours: perform full vault index
  - Tokenize all markdown notes (extract words, apply stopword filter, lowercase)
  - Compute word frequencies
  - For each note: identify all word pairs that co-occur (both present in note)
  - Store in SQLite; update `index_state.last_full_index`
- If fresh: load from SQLite immediately (no startup delay)

**On Word tab open** (with 500ms debounce):
- Check vault file watcher for changed files since last index
- If changes exist: delta index
  - Tokenize only changed files
  - Update `words` table with new frequencies
  - Update `co_occurrence` table with new pairs from changed notes
  - Update `index_state.changed_files_since`
- If no changes: use cached SQLite data (instant)

**Word scope** (adaptive):
- Top N words by frequency, where N = `min(100, vault_size / 10)` capped at 200
- Exclude common stopwords (the, and, is, a, in, to, of, for, etc.)
- Minimum word length: 3 characters (avoid noise)
- Co-occurrence threshold: only show edges if two words appear together in 2+ notes (configurable, default=2)

## Word Graph Rendering

### Graph Display

The Word graph is a force-directed graph using the same Canvas 2D physics engine as existing Global and Current Note views (300-tick animation, node repulsion, gravity, spring forces).

**Nodes** (words):
- Each word is a node
- **Size**: proportional to word frequency (logarithmic scale: larger word = higher frequency)
- **Color**: heat-map by frequency (cool colors = low frequency, warm colors = high frequency); alternatively, grouped by dominant note clusters
- **Label**: word text, rendered only for nodes above size threshold to avoid clutter

**Links** (co-occurrence):
- Edge between two words if they co-occur in N+ notes (default N=2)
- **Edge weight/thickness**: proportional to co-occurrence count
- **Edge color**: lighter/thinner for weak co-occurrence, stronger for tight clusters

**Physics settings** (reuse from existing implementation):
- Node repulsion: push words apart
- Gravity: pull toward center
- Spring forces: attract co-occurring words
- Damping: smooth animation, stable convergence

### Zoom (Per-View, Independent)

Each graph view (Global, Current Note, Word) maintains its own zoom level and pan offset independently.

**Zoom levels**:
- Min: 0.2× (fit entire graph in view)
- Max: 5× (close inspection of word clusters)
- Default: fit-all (all nodes visible)

**Controls** (all active simultaneously):

| Method | Action |
|--------|--------|
| **Mouse wheel** | Scroll up = zoom in; scroll down = zoom out; centered on cursor position |
| **Touch pinch** | Two-finger pinch on trackpad (macOS) or touch device (mobile); pinch center is zoom center |
| **Keyboard** | `+` = zoom in 1.2×; `−` = zoom out; `0` = reset to fit-all |
| **Buttons** | `+` and `−` buttons in graph header (10% zoom per click); `Reset` button to fit-all |

**Implementation**:
- Zoom state stored per-view: `{scale, panX, panY, centerX, centerY}`
- Canvas transform applied before drawing: `ctx.scale(zoom.scale, zoom.scale); ctx.translate(-zoom.panX, -zoom.panY)`
- All event handlers (click, hover, interaction) use inverse transform to get world-space coordinates
- Switching views restores that view's zoom state

## Interaction Model

### Word Node Interaction

**Click**:
- Highlight clicked word node (brighten color, enlarge 1.5×)
- Query SQLite for all notes containing this word
- Highlight those notes in sidebar (brighten/accent color)
- Fade non-related notes (reduce opacity to 0.3)
- Show tooltip: `"Appears in N notes, co-occurs with: word1, word2, word3"` (top 3 by count)

**Double-click**:
- Open first/most relevant note containing this word
- Switch to Current Note view, center on that note

**Hover**:
- Show tooltip with word stats: frequency (count), co-occurrence partners (top 5), example notes (up to 3)

**Right-click** (context menu):
- "Show in notes" — highlight all notes containing this word
- "Show co-occurrence cluster" — filter graph to show only this word and its immediate neighbors
- "Open top note" — open most relevant note containing this word

### Multi-Selection (Future)

- Shift+click: Add/remove word from selection (highlight multiple words)
- Ctrl+click: Quick filter to selected words and their co-occurrence links

## UI Integration

### Toggle Button

Replace existing graph toggle from `"Global | Current"` to `"Global | Current | Word"`:

```html
<div id="graph-toggle">
  <button id="graph-global">Global</button>
  <button id="graph-current">Current</button>
  <button id="graph-word">Word</button>
</div>
```

### Word View Header

When Word view is active, display:

```
Word Relationships              [+] [-] [Reset]
Last updated: 2 hours ago
[Search words...]
```

- **Title**: "Word Relationships"
- **Timestamp**: "Last updated: HH:MM ago" (or "Indexing..." if delta update in progress)
- **Search/filter input** (optional, v2): filter word nodes by name
- **Zoom buttons**: `+`, `−`, `Reset` (same as Global/Current views)

### Sidebar Integration

No changes required to sidebar. Word view uses the same note list as other views. Clicking a word highlights related notes in the existing sidebar (same highlighting mechanism as Global/Current views).

## Performance & Scalability

### Constraints

- **Vault size**: SQLite index handles up to 100K notes efficiently
- **Word count**: Top 200 words manageable; Graph rendering smooth for ≤500 nodes
- **Startup**: Full index ~5–10 seconds for 1K notes; incremental delta <100ms
- **Memory**: Co-occurrence table ~1–2 MB for typical 1K-note vault; Canvas animation 60 FPS
- **Real-time**: File watcher debounces rapid changes (500ms); indexing runs in background thread (Tauri `async_runtime::spawn_blocking`)

### Optimization Strategies

1. **Lazy loading**: Don't index until user opens Word view first time
2. **Incremental indexing**: Only recompute changed files, merge into existing index
3. **Throttled rendering**: Animate graph only when tab is visible (pause on tab switch, resume on return)
4. **Word threshold**: Top-N filter removes long-tail words that add visual noise
5. **Background thread**: Full indexing and delta updates run off-main-thread to prevent UI freeze

## Testing

### Unit Tests (Rust)

- **Tokenization**: verify word extraction, lowercase, stopword filtering, minimum length
- **Frequency**: verify frequency counts across multiple notes
- **Co-occurrence**: verify pair detection when words appear in same note, not counted across different notes
- **Delta updates**: verify incremental index correctly merges new/changed files without full rebuild
- **SQLite schema**: verify table creation, indexes, constraints, data integrity

### Integration Tests

- Index fixture vault (20–50 notes) with known word pairs
- Verify co-occurrence counts match expected output
- Test delta update: modify one note, verify only that note's pairs are recalculated
- Verify top-N word filtering produces expected result set
- Performance test: index 1K-note vault, measure startup time and memory usage

### Manual/Visual Tests

- Verify zoom per-view independent (zoom Word view, switch to Global, verify Global zoom unchanged)
- Verify interaction highlighting (click word, confirm correct notes highlighted in sidebar)
- Verify zoom controls all functional (mouse wheel, pinch, buttons, keyboard)
- Performance on large vault (1K+ notes): no UI freeze, smooth animation
- File watch: modify note, open Word tab, verify index updates within 1 second
- Stopword filtering: verify common words (the, and, etc.) excluded from graph

## Error Handling

- **SQLite errors**: Log to debug log; fall back to in-memory index if database unavailable
- **Tokenization errors**: Skip malformed files; log error; continue with remaining vault
- **File watch errors**: Graceful degradation; retry on next Word tab open
- **Zoom out of bounds**: Clamp to min/max zoom levels
- **Empty vault**: Show "No words found" message; disable Word tab until files added

## Out of Scope (Future Phases)

- Multi-selection of words (Shift+click)
- Custom stopword lists
- Word filtering UI (search words in graph)
- Exporting word relationship data
- Semantic similarity (requires embeddings)
- Cross-vault word analysis
- Word graph statistics dashboard

## Related Features

- Existing Global/Current Note graph views (reuse Canvas physics, zoom implementation)
- File watcher already implemented (notification on file changes)
- Sidebar note highlighting already implemented (hover/click notes)
