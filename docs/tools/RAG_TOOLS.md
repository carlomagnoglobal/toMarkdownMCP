# AI / RAG Toolkit

These tools turn Markdown (or any convertible document) into machine-consumable retrieval units.
Every tool accepts `output_format: "markdown"` (default, human-readable) or `"json"`
(machine-readable), so the MCP serves both humans and machines. All accept either `file_path`
(any supported format, converted first) or inline `content`.

## `chunk_markdown`
Heading-aware, token-bounded chunking.

- `max_tokens` (default 512), `overlap` (default 64 words).
- Splits on heading boundaries first, then by token budget with word overlap.
- JSON: `[{ id, text, metadata: { source, heading_path, chunk_index, token_estimate } }]`.
- Token counts are estimated (~words × 1.3) — no model dependency.

## `extract_chunks_for_rag`
Same as `chunk_markdown` but defaults to JSON output. Primary ingestion entry point: convert any
file (PDF, DOCX, EPUB, …) to Markdown then chunk it for embeddings.

## `get_document_outline`
Nested heading outline `{ level, title, anchor, children }` — Markdown list or JSON tree. Anchors
use GitHub-style slugs.

## `search_content`
Search inside converted content across a directory (recursive), ranked by term frequency. Returns
`{ source, score, snippet }` with a context snippet around the first match.

- `directory` (default `.`), `query` (required), `max_results` (default 10).

## `get_text_statistics`
Word/vocabulary statistics for one file/content:

- Totals: total words, distinct words, vocabulary richness (distinct ÷ total), character count,
  sentence & paragraph counts, average words per sentence.
- A per-word frequency table (word, count, % of total), top-`top_n` (default 25).
- Options: `min_length`, `stopwords` (exclude common English stopwords).

## `get_corpus_statistics`
Aggregates `get_text_statistics` across a directory: per-document word/distinct counts, corpus
totals, and a global distinct-word count, plus corpus-wide top words. `stopwords` defaults to true.

## Shared tokenizer
All of these (and the knowledge tools) share `rag::tokenize_words` and a common stopword list, so
search ranking, statistics, and TF-IDF keyword extraction tokenize identically.

## Related
See `AI_TOOLS.md` for the retrieval step (`retrieve_context`), token budgeting (`count_tokens`),
dedup/clustering, document intelligence, and the optional Claude-backed generative tools.
