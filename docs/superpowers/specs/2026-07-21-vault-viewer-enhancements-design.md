# Vault Viewer Enhancements - Design Specification

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to create the implementation plan for this design.

**Goal:** Add comprehensive tab management, file recovery, content export, and image zoom capabilities to the toMarkdown Vault Viewer GUI.

**Architecture:** Four interconnected feature systems (Tab Management, Recycle Bin, Clipboard/Copy, Image Zoom) unified through a central state manager and Settings system. All features share common data structures, persistent configuration, and consistent UI patterns.

**Tech Stack:** Rust backend (Tauri), TypeScript/HTML frontend, SQLite for vault state, JSON for configuration.

## Global Constraints

- All features must work with existing multi-tab architecture
- Auto-save enabled on all tab close operations (no unsaved changes warnings)
- Keyboard shortcuts must be customizable in Settings
- All user preferences must persist across sessions per vault
- Settings stored in vault config file (.tomarkdown/config.json)
- Recycle bin is a special system folder in the file tree
- All copy operations show toast notifications
- Image zoom supports pan/drag when zoomed in
- Must maintain backward compatibility with existing vault data

---

## 1. TAB MANAGEMENT SYSTEM

### Core Behavior

- **Default mode:** Multi-tab (like browser tabs)
- **Toggle capability:** Users can switch to single-tab mode via Settings → Interface
- **Preference persistence:** Choice saved to vault config and restored on app restart
- **Auto-save:** All unsaved changes auto-saved before tab close (no confirmation dialogs)

### Tab Navigation Methods

**Browser Shortcuts (Customizable):**
- Cmd+Tab / Cmd+Shift+Tab (forward/backward cycle through tabs)
- Cmd+[ (back button - returns to previously viewed tab)

**Numbered Shortcuts (Customizable):**
- Cmd+1, Cmd+2, Cmd+3... (jump directly to specific tab by position)

**UI Interaction:**
- Click any tab to switch
- Middle-click to close tab (standard browser behavior)

### Tab Controls

**Right-click Context Menu on Tabs:**
- Close this tab
- Close all except this (also has keyboard shortcut: Cmd+Shift+Alt+W)
- Duplicate tab
- Pin/unpin tab (optional future enhancement)

**Tab Bar Elements:**
- Close button (X) on each tab
- Tab title shows filename
- Active tab highlighted
- Unsaved indicator (*) on title if changes exist (but auto-saved)

### Preview Tab Feature

**Dedicated Preview Tab:**
- Special tab labeled "📋 Preview" 
- Clicking files in the tree updates preview immediately
- Preview auto-refreshes as you click different files
- Preview shows first 200 lines or content summary
- Click "Open" button in preview to open file as full editor tab

**Preview State Management:**
- Preview tab state saved/restored with other tabs
- Preview updates in real-time as tree selections change
- Preview can be closed and reopened from tab bar

### Back Button Navigation

**Browser-style History:**
- Back button (← arrow) in toolbar
- Returns to previously active tab
- Maintains history stack of visited tabs
- Keyboard shortcut: Cmd+[ (standard back)
- Forward button (→) also available: Cmd+]
- Disabled when no history available

### Tab Persistence

**Session Restoration:**
- Currently open tabs saved to vault config
- Tab order preserved
- Active tab remembered
- Restored on next app launch (if setting enabled)
- Separate config per vault

---

## 2. RECYCLE BIN SYSTEM

### Deletion Workflow

**Safe Deletion:**
- Right-click file in tree → Delete
- File moved to vault recycle bin (not permanently deleted)
- Original location stored
- Deletion timestamp recorded
- Toast confirmation: "File moved to recycle bin"

### Recycle Bin as Special Folder

**File Tree Integration:**
- Appears as "🗑️ Recycle Bin" special folder in tree
- Expandable like normal folders
- Shows count of deleted files as badge (e.g., "🗑️ Recycle Bin (5)")
- Files display with:
  - Original filename
  - Deletion date (tooltip)
  - Original location (small text below filename)

**Metadata Storage:**
```
DeletedFile {
  id: UUID,
  original_path: string,
  vault_path: string,
  deleted_at: DateTime,
  file_size: u64,
  file_hash: string
}
```

### Recovery/Restoration

**Right-click on Deleted File:**
- "Restore" option → Restores to original location
- If original folder no longer exists → Creates folder structure
- If file exists at original path → Prompt user:
  - Keep both (adds suffix: "file copy.md")
  - Overwrite
  - Cancel

**Restoration Process:**
1. Read original location from metadata
2. Create parent folders if needed
3. Move file from recycle bin to original location
4. Update file tree view
5. Show toast: "Restored filename to original location"

### Permanent Deletion

**Right-click on Deleted File:**
- "Delete Permanently" option
- Shows confirmation: "Permanently delete? This cannot be undone."
- Removes file from recycle bin entirely
- Does not show in trash after permanent deletion

**Bulk Operations:**
- Right-click Recycle Bin folder → "Empty Recycle Bin"
- Deletes all files permanently
- Asks for confirmation

### Retention Policy

**Configuration (Settings → File Management):**
- Default: 6 months
- Options: 1 Month / 3 Months / 6 Months / 1 Year / Permanent
- User-customizable dropdown

**Auto-cleanup:**
- Runs on app startup
- Deletes files older than retention period
- Logs cleanup action (X files auto-deleted)
- Permanent mode: files never auto-delete

**Manual Cleanup:**
- Empty Recycle Bin button in recycle bin context menu
- Immediately deletes all files regardless of age

---

## 3. CLIPBOARD/COPY OPTIONS SYSTEM

### Access Methods

**Primary: Right-click Context Menu**
- Right-click any file in tree or tab
- Shows "Copy" submenu with grouped options
- Same menu structure everywhere (consistent UX)

**Secondary: Keyboard Shortcuts**
- Customizable in Settings → Copy/Clipboard
- Default shortcuts:
  - Cmd+C → Copy as Markdown
  - Cmd+Shift+C → Copy as Base64
  - Cmd+Ctrl+H → Copy SHA256 Hash
- Users can customize all shortcuts

**Feedback:**
- Toast notification (top-right): "✓ Copied as Base64"
- Notification visible for 2-3 seconds
- Shows format name to confirm what was copied

### Copy Options Organization

**Grouped Submenu Structure:**

**Copy Content:**
- Copy as Base64 (encode entire file)
- Copy as Markdown (render/wrap as markdown)
- Copy as Hex-Encoded (hex string format)

**Copy Hash:**
- Copy SHA256 Hash
- Copy MD5 Hash
- Copy CRC Hash

### Behavior by File Type

**Markdown Files (.md, .markdown):**
- All formats available
- Base64: file bytes encoded
- Markdown: raw markdown text
- Hex-Encoded: file bytes as hex string
- Hashes: computed from file bytes

**Code Files (.js, .py, .rs, .go, etc.):**
- All formats available
- Base64: file bytes encoded
- Markdown: wrapped in markdown code block with language syntax highlighting
- Hex-Encoded: file bytes as hex string
- Hashes: computed from file bytes

**Image Files (.png, .jpg, .gif, .svg, etc.):**
- All formats available
- Base64: image data (can be used as data URL)
- Markdown: `![filename](data:image/png;base64,...)` markdown syntax
- Hex-Encoded: image bytes as hex string
- Hashes: computed from image bytes

**Binary/Unknown Files:**
- All formats available
- Base64: file bytes encoded
- Markdown: hex dump wrapped in code block
- Hex-Encoded: file bytes as hex string
- Hashes: computed from file bytes

**Text Files (.txt, .log, .json, etc.):**
- All formats available
- Base64: file text encoded
- Markdown: wrapped in markdown code block
- Hex-Encoded: file bytes as hex string
- Hashes: computed from file bytes

### Keyboard Shortcut Defaults

Users can customize in Settings → Copy/Clipboard:
```
Cmd+C           → Copy as Markdown
Cmd+Shift+C     → Copy as Base64
Cmd+Ctrl+H      → Copy SHA256 Hash
Cmd+Ctrl+M      → Copy MD5 Hash
Cmd+Ctrl+R      → Copy CRC Hash
Cmd+Ctrl+X      → Copy as Hex-Encoded
```

All shortcuts are user-customizable via Settings table.

---

## 4. IMAGE ZOOM SYSTEM

### Zoom Range

- **Minimum:** 1x (original size)
- **Maximum:** 15x (highly magnified)
- **Step increments:** 0.1x (smooth progression)
- **Current level displayed:** "3.2x" shown in toolbar

### Zoom Controls (All Available Together)

**A) Zoom Buttons:**
- "−" button (decrease zoom, −1 step)
- "+" button (increase zoom, +1 step)
- Located in image toolbar above image
- Shows current zoom level: "3.2x"

**B) Zoom Slider:**
- Horizontal slider (1x to 15x range)
- Drag to adjust smoothly
- Shows numeric value while dragging
- Located in toolbar next to buttons

**C) Keyboard Shortcuts:**
- `+` key (or `=`) → Zoom in (+1 step)
- `−` key → Zoom out (−1 step)
- `0` key → Reset to 1x
- `Shift+0` → Fit to window

**D) Mouse Wheel:**
- Scroll up over image → Zoom in
- Scroll down over image → Zoom out
- Only works when cursor is on image

### Fit-to-Window Button

- Auto-fits image to current viewport
- Calculates best zoom level for full image visibility
- Button labeled "Fit" in toolbar
- Shortcut: Shift+0
- Useful for returning from zoomed state

### Pan/Drag Capability

**When Image is Zoomed In:**
- Image larger than viewport boundaries
- Click and drag to pan around image
- Arrow keys also pan in four directions
- Cursor changes to "grab" hand when hoverable
- Scroll bars appear at edges

**Pan Controls:**
- Drag anywhere on image to move view
- Arrow keys (Up/Down/Left/Right) to scroll
- Page Up/Page Down for larger jumps

### Zoom Level Persistence

**Configuration (Settings → Image Viewer):**

**Option A (Default): Reset to 1x per image**
- Every time you switch to a different image, zoom resets to 1x
- Clean slate for each image

**Option B: Remember zoom per image**
- Each image remembers its zoom level independently
- Useful for consistent examination

**Option C: Remember global zoom level**
- All images start at your last zoom level
- Useful for consistent workflow

User selects preference in Settings (radio buttons).

### Image Toolbar Layout

```
[−] [3.2x] [+]  |  [=====Slider=====]  |  [Fit]  |  [Info]
```

---

## 5. SETTINGS/PREFERENCES SYSTEM

### Access Method

- **Menu:** File → Preferences (or Settings)
- **Keyboard shortcut:** Cmd+, (standard Mac/Windows)
- **Opens:** Dedicated Preferences window (modal dialog or sidebar)
- **Applies to:** Current vault only (different vaults can have different settings)

### Settings Organization - Tabbed Interface

**Tab 1: Interface**
- **Tab Mode:**
  - ○ Single-tab mode
  - ● Multi-tab mode (default)
- **Tab Navigation:**
  - Next Tab: [Cmd+Tab] ← editable
  - Previous Tab: [Cmd+Shift+Tab] ← editable
  - Close All Except: [Cmd+Shift+Alt+W] ← editable
  - Back Button: [Cmd+[] ← editable
  - Forward Button: [Cmd+]] ← editable
- **Theme:** Light / Dark / Auto (system)
- **Language:** English / (expandable)
- **Auto-restore tabs on launch:** Toggle [ON/OFF] (default: ON)

**Tab 2: File Management**
- **Recycle Bin Retention:**
  - Dropdown: 1 Month / 3 Months / ● 6 Months / 1 Year / Permanent
- **Auto-save on Tab Close:** Toggle [ON/OFF] (default: ON)
- **Restore Conflict Resolution:**
  - Keep Both (adds " copy" suffix)
  - Overwrite
  - Ask each time (default)

**Tab 3: Copy/Clipboard**
- **Default Copy Format:** Dropdown (Markdown / Base64 / Hex)
- **Show Toast Notifications:** Toggle [ON/OFF] (default: ON)
- **Customize Shortcuts Table:**
  - Format | Current Shortcut | [Edit]
  - Copy as Markdown | Cmd+C | [Edit]
  - Copy as Base64 | Cmd+Shift+C | [Edit]
  - Copy SHA256 | Cmd+Ctrl+H | [Edit]
  - Copy MD5 | Cmd+Ctrl+M | [Edit]
  - Copy CRC | Cmd+Ctrl+R | [Edit]
  - Copy as Hex | Cmd+Ctrl+X | [Edit]
  - (Users can click [Edit] to customize)

**Tab 4: Image Viewer**
- **Zoom Behavior:**
  - ● Reset to 1x per image (default)
  - ○ Remember zoom per image
  - ○ Remember global zoom level
- **Mouse Wheel Zoom:** Toggle [ON/OFF] (default: ON)
- **Pan on Zoom:** Radio buttons
  - ● Allow drag pan (default)
  - ○ Arrow keys only

**Tab 5: General/Advanced**
- **Theme:** Light / Dark / Auto
- **Language:** English
- **Advanced Options:**
  - [Clear All Settings] button
  - [Export Settings] button (creates backup JSON)
  - [Import Settings] button (restore from backup)
  - Cache clear options
- **About:** Version info, links

### Settings Persistence

**Storage:**
- File: `.tomarkdown/vault_config.json` (per vault)
- Format: JSON with sections for each feature
- Loaded on app startup
- Saved on each change (debounced)

**Example Config Structure:**
```json
{
  "interface": {
    "tab_mode": "multi",
    "theme": "system",
    "language": "en",
    "auto_restore_tabs": true,
    "shortcuts": {
      "next_tab": "Cmd+Tab",
      "prev_tab": "Cmd+Shift+Tab",
      "close_all_except": "Cmd+Shift+Alt+W"
    }
  },
  "file_management": {
    "recycle_retention_days": 180,
    "auto_save": true,
    "restore_conflict": "ask"
  },
  "clipboard": {
    "show_toast": true,
    "default_format": "markdown",
    "shortcuts": {
      "copy_markdown": "Cmd+C",
      "copy_base64": "Cmd+Shift+C",
      "copy_sha256": "Cmd+Ctrl+H"
    }
  },
  "image_viewer": {
    "zoom_behavior": "reset_per_image",
    "mouse_wheel_zoom": true,
    "pan_mode": "drag"
  }
}
```

---

## 6. DATA FLOW & INTEGRATION

### State Management

Central state manager tracks:
```rust
VaultViewerState {
  open_tabs: Vec<Tab>,
  active_tab: TabId,
  preview_tab: Option<TabId>,
  tab_history: VecDeque<TabId>,
  deleted_files: Vec<DeletedFile>,
  zoom_levels: HashMap<FileId, f32>,
  user_preferences: PreferencesConfig,
  recycle_bin_size: u64,
}
```

### Feature Integration

**Tab Management ↔ Auto-save ↔ Recycle Bin:**
- Close tab → auto-save triggers → if user later deletes file → goes to recycle bin
- Back button maintains history of all tab switches

**File Operations ↔ Recycle Bin:**
- Delete file → captured original path + timestamp
- Restore file → reads metadata, recreates location
- Settings control retention (auto-cleanup runs on startup)

**Clipboard ↔ All File Types:**
- Right-click any file/tab → Copy submenu appears
- Format selection queries file type → determines available options
- Toast confirms copy success

**Image Zoom ↔ Tab System:**
- When image tab active → zoom controls appear in toolbar
- Zoom level persisted based on user's Settings choice
- Pan only works when image exceeds viewport

**Settings ↔ All Features:**
- Tab mode affects UI layout (single vs. multi-tab)
- Keyboard shortcuts affect all navigation
- Recycle retention affects auto-cleanup frequency
- Copy shortcuts affect right-click menu
- Zoom behavior affects image viewer state

### File Tree Context Menu Integration

Right-click on files shows:
```
Open
Open in New Tab
─────────────────
Copy
  ├─ Copy as Base64
  ├─ Copy as Markdown
  ├─ Copy as Hex-Encoded
  └─ Copy Hash
      ├─ SHA256
      ├─ MD5
      └─ CRC
─────────────────
Delete (→ Recycle Bin)
Rename
Properties
```

### Session Lifecycle

**On App Startup:**
1. Load user preferences from vault config
2. Restore previously open tabs (if enabled in settings)
3. Restore zoom levels for images (if configured to remember)
4. Load recycle bin contents from system folder
5. Run retention cleanup (auto-delete files older than retention period)
6. Restore preview tab state if it was open
7. Show app window with restored state

**On File Open:**
1. Detect file type
2. Create tab with auto-assigned ID
3. Instantiate appropriate viewer (CodeViewer, ImageViewer, etc.)
4. Render content
5. Add tab to tab bar
6. Set as active tab
7. Add to tab history (for back button)
8. Add to recent files

**On Tab Close:**
1. Check if file has unsaved changes
2. Auto-save if changes exist
3. Remove from open tabs list
4. Update tab bar display
5. Save session state to config
6. If all tabs closed → show empty state

**On File Delete:**
1. Move file to recycle bin folder
2. Store metadata (original path, timestamp)
3. Update file tree (show file in Recycle Bin folder)
4. Show toast confirmation

---

## 7. ERROR HANDLING

**Tab Management:**
- Corrupted tab history → Clear history, restart
- Missing tab file → Create new empty tab
- Keyboard shortcut conflict → Show warning, use default

**Recycle Bin:**
- Can't restore → Show error reason (no space, permission denied)
- File deleted from disk → Show "File no longer exists" in recycle
- Retention period invalid → Use default (6 months)

**Clipboard/Copy:**
- Large file copy → Show progress indicator
- Clipboard unavailable → Show "Cannot access clipboard"
- Hash computation failed → Show "Could not compute hash"

**Image Zoom:**
- Image won't load → Show placeholder "Image failed to load"
- Zoom level invalid → Clamp to 1x-15x range
- Pan calculation error → Disable pan, show zoom only

**Settings:**
- Config file corrupted → Load defaults, backup corrupted file
- Invalid shortcut → Reject, show validation error
- Permission denied writing config → Show warning, cache in memory

---

## 8. TESTING STRATEGY

### Unit Tests

**Tab Management:**
- Create/close/switch tabs
- Back button history stack
- Preview tab state updates
- Tab persistence serialization
- Auto-save before close
- Keyboard shortcut routing

**Recycle Bin:**
- Move file to recycle
- Delete file permanently
- Restore to original location
- Restore with conflicts
- Auto-cleanup by retention period
- Metadata storage/retrieval

**Clipboard/Copy:**
- Base64 encoding/decoding
- Markdown formatting for each type
- Hash computation accuracy
- Hex encoding
- Toast notification firing
- Shortcut dispatch

**Image Zoom:**
- Zoom bounds (1x-15x)
- Zoom level persistence
- Pan coordinate calculations
- Fit-to-window algorithm
- Mouse wheel delta handling
- Arrow key pan

**Settings:**
- Config persistence
- Settings load on startup
- Keyboard shortcut customization
- Default value application
- Per-vault configuration

### Integration Tests

1. **Tab + Delete + Restore:** Delete file from open tab → verify in recycle → restore → verify back in tree
2. **Copy All Formats:** Copy file in all formats → verify each works → paste in text editor → verify correct
3. **Zoom + Pan:** Open image → zoom in → pan around → zoom out → verify coordinates correct
4. **Settings Change:** Modify setting → verify all features respond immediately
5. **Back Button Flow:** Switch tabs multiple times → back button returns to each in reverse
6. **Preview Tab:** Click files → preview updates → open in main tab → verify both work
7. **Auto-save + Close:** Edit file → close without manual save → reopen → verify saved
8. **Recycle Retention:** Set 1-day retention → delete files → wait → verify auto-cleanup

### Manual UI Tests

- Tab context menus responsive to right-click
- All keyboard shortcuts work as configured
- Recycle bin folder expands/collapses smoothly
- Copy toast notifications appear and disappear
- Zoom buttons/slider/wheel all functional
- Pan/drag works when zoomed in
- Settings dialog saves changes immediately
- No visual glitches or layout breaks
- All themes (light/dark) display correctly

---

## 9. DELIVERABLES

1. **Tab Management System** - complete with all navigation controls
2. **Recycle Bin System** - with file recovery and retention policy
3. **Clipboard/Copy Options** - with all formats and hash support
4. **Image Zoom System** - with all control methods and pan capability
5. **Settings/Preferences UI** - unified configuration panel
6. **State Management** - persistent configuration and session state
7. **Integration Tests** - 8+ integration test scenarios
8. **Documentation** - user guide for all new features

---

## 10. SUCCESS CRITERIA

- ✅ All four feature groups implemented and functional
- ✅ User can toggle between single and multi-tab modes
- ✅ Deleted files recoverable from recycle bin
- ✅ All clipboard formats work for all file types
- ✅ Image zoom smooth and responsive from 1x to 15x
- ✅ All settings persist across sessions per vault
- ✅ Keyboard shortcuts customizable and functional
- ✅ Integration tests pass
- ✅ No regressions in existing features
- ✅ UI/UX consistent and intuitive

