# MCP Functions Reference

A quick-scan index of all 62 MCP tools/functions exposed by `to_markdown_mcp`, grouped by category. Each entry is `tool_name` — one-line purpose, sourced from the tool's `description` in [`MCP_TOOL_SCHEMA.json`](MCP_TOOL_SCHEMA.json).

For full parameter schemas call `get_tool_help` (with or without a `tool_name`), or read `MCP_TOOL_SCHEMA.json` directly. For narrative, example-driven docs per suite see [docs/tools/](../tools/).

## File conversion

- `convert_file` — Convert a text, code, HTML, or document file to Markdown format (HTML/HTM/MHTML/webarchive; PDF, DOCX, DOC, RTF, ODT; spreadsheets; more).
- `convert_text` — Convert plain text content to Markdown format.
- `convert_from_source` — Convert code or HTML from various sources (file, URL, stdin) to Markdown.
- `batch_convert_files` — Convert multiple files to Markdown in one call.

## File & vault management

- `list_directory_files` — List all code files in a directory (recursively).
- `get_file_summary` — Get a lightweight summary of a file including metadata, headings, and preview.
- `search_files` — Search for text across files and return matching snippets.
- `get_recently_modified_files` — List recently modified files in a directory.
- `get_vault_statistics` — Statistics about a directory: file counts by type, top-level folder list, and top 20 most frequent tags from Markdown files.
- `read_file` — Read raw file content as-is, without any conversion or processing.
- `create_or_append_file` — Create a new file with content, or append/overwrite an existing file.
- `move_or_rename_file` — Move or rename a file or directory to a new path.
- `delete_file` — Permanently delete a file or directory (irreversible).
- `batch_create_notes` — Create multiple new files in one call.
- `update_note_properties` — Update YAML frontmatter properties in a Markdown file without changing the body.
- `find_note_by_alias_or_title` — Fuzzy lookup to find a file by name, filename snippet, or frontmatter alias.

## Markdown & document editing

- `extract_active_todos` — Scans for uncompleted tasks (lines containing `- [ ]`) and returns them grouped by file.
- `safe_append_or_replace_section` — Updates content under a specific heading in a Markdown file without touching the rest of the document.
- `resolve_and_validate_links` — Validates a list of note names or file paths against real files on disk.
- `upsert_markdown_table` — Inserts or overwrites a Markdown table under a specific heading; creates the file if it does not exist.
- `get_document_outline` — Extract a nested heading outline (level/title/anchor/children) from a document. Dual output: Markdown list or JSON tree.
- `analyze_text` — Complete text metrics: word/character/space/token counts plus sorted frequency tables for words, characters, and tokens.

## Graph & relationships

- `get_graph_relationships` — Map wiki-link connections: outlinks (references this file makes) and backlinks (files that reference this file).
- `find_related_notes` — Find notes similar to a given note via cosine similarity over TF vectors, boosted by shared tags/links.

## RAG, retrieval & knowledge

- `chunk_markdown` — Split a Markdown/document file into heading-aware, token-bounded chunks for RAG/embeddings.
- `extract_chunks_for_rag` — Convert any supported file to Markdown then chunk it into JSON records (`{id, text, metadata}`) ready for embedding; primary RAG ingestion entry point.
- `build_knowledge_index` — Flagship "second brain" export: one JSON artifact bundling summary, outline, tags, keywords, stats, and RAG chunks for a document.
- `retrieve_context` — RAG retrieval: rank chunks across a directory (or file) against a query and assemble the top ones into one context block under a token budget, with citations.
- `find_duplicates` — Detect near-duplicate documents across a directory using SimHash.
- `cluster_documents` — Cluster documents in a directory by topic via cosine similarity over term vectors.
- `get_corpus_statistics` — Aggregate word statistics across a directory: per-document word/distinct counts plus corpus totals and global distinct-word count.

## Text analytics & NLP

- `search_content` — Search inside converted document content across a directory, ranked by term frequency; returns ranked snippets with source and score.
- `get_text_statistics` — Word/vocabulary statistics for a file: total words, distinct words, vocabulary richness, sentence/paragraph counts, and a per-word frequency table.
- `count_tokens` — Estimate token count for a file/content and show whether it fits each model's context window (Opus 4.8, Sonnet 5, Haiku 4.5).
- `analyze_readability` — Compute Flesch Reading Ease and Flesch-Kincaid grade level with word/sentence/syllable counts.
- `detect_natural_language` — Detect the natural language of a document (English/Spanish/French/German/Portuguese/Italian) via function-word analysis.
- `classify_document` — Heuristic topic/content-type classification (technical, finance, legal, correspondence, academic, marketing) via keyword signals.
- `summarize_document` — Extractive TL;DR: rank sentences by keyword density and position and return the top ones (deterministic, no LLM).
- `extract_qa_pairs` — Mine `Q:`/`A:` lines and `?`-terminated headings into `{question, answer, source}` pairs — flashcards, eval sets, or RAG ground-truth.
- `extract_entities` — Lightweight entity extraction: URLs, emails, dates, and capitalized name phrases, aggregated with counts.
- `extract_tags` — Extract `#tags` and frontmatter tags; vault-wide tag index or per-file tags.
- `extract_keywords` — Salient terms for a document via TF-IDF; answers "what is this note about."

## AI-powered (Claude-backed, needs `ANTHROPIC_API_KEY`)

- `ai_summarize` — Abstractive summary via the Claude API (local alternative: `summarize_document`).
- `ai_ask` — RAG question-answering via the Claude API: retrieves relevant context across a directory/file, then answers grounded in it with citations.
- `ai_tag` — Suggest topical tags for a document via the Claude API (local alternative: `extract_keywords`).
- `ai_translate` — Translate a document to a target language via the Claude API, preserving Markdown.
- `ai_classify` — Classify a document into caller-provided labels via the Claude API (local alternative: `classify_document`).

## Browser (real Chromium)

- `browser_open_url` — Open a URL in a real Chromium browser (executes JavaScript, keeps cookies/session); headless by default, `visible=true` for human-in-the-loop CAPTCHAs/logins.
- `browser_capture_markdown` — Capture the rendered HTML of the page currently open in the browser session and convert it to Markdown.
- `browser_close` — Close the Chromium browser session opened by `browser_open_url` and free its resources.

## Obsidian vault intelligence

- `obsidian_vault_index` — Index an Obsidian vault: note/attachment counts, tag frequencies, aliases, broken and ambiguous wikilinks, optionally orphan notes.
- `obsidian_get_note` — Get a note by path, filename stem, or frontmatter alias: parsed YAML frontmatter, aliases, tags, headings, outgoing wikilinks.
- `obsidian_resolve_link` — Resolve a wikilink string (`[[target#heading|alias]]`, alias, path, or `^block` form) using Obsidian's shortest-path rules.
- `obsidian_get_backlinks` — List all inbound wikilinks to a note — including alias, heading, block, and embed forms — with the linking note and source line as context.
- `obsidian_search` — Search an Obsidian vault by tag (with nested-tag prefix matching), alias, frontmatter field, or full text.
- `obsidian_list_tasks` — List checkbox tasks across the vault or one note, with all states (open, done, in progress, cancelled, forwarded, ...) and nesting depth.
- `obsidian_get_vault_config` — Read the vault's `.obsidian` configuration: attachment folder, new-link format, daily-notes settings, templates folder, enabled core plugins.
- `obsidian_create_note_from_template` — Create a note (or today's daily note) from a template, substituting `{{title}}`, `{{date}}`, `{{time}}`, `{{date:FORMAT}}`.
- `obsidian_rename_note` — Rename or move a note and rewrite every inbound wikilink, like Obsidian's own rename (`dry_run` by default).
- `obsidian_convert_canvas` — Convert an Obsidian `.canvas` file (JsonCanvas: text/file/link/group nodes and edges) to structured Markdown.
- `obsidian_extract_dataview_fields` — Extract Dataview fields — inline `key:: value` forms and frontmatter properties — across the vault or one note.

## Meta / help

- `get_tool_help` — Get help on available tools: specify a `tool_name` for detailed help, or omit for a summary of all tools.
