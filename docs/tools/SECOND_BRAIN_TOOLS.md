# "Second Brain" Knowledge Tools

These tools turn a Markdown vault into an AI-ready knowledge layer. All are extractive and
deterministic (no LLM dependency) but emit clean JSON (`output_format: "json"`) so an external
LLM, agent, or vector DB can consume them. They reuse the `rag` tokenizer and the existing vault
tools (`get_graph_relationships`, `resolve_and_validate_links`, `extract_active_todos`).

## `extract_tags`
Collect `#tags` (inline hashtags, requiring a letter so `#1` is ignored) and YAML frontmatter
`tags:`. With `directory` it builds a vault-wide index `{ tag, count, files }`; with
`file_path`/`content` it returns tags for one note.

## `extract_keywords`
Salient terms via **TF-IDF**. Pass `directory` to use the vault as the IDF corpus; without it the
score degrades to term frequency. Answers "what is this note about" for retrieval and auto-tagging.

## `find_related_notes`
Given a `file_path`, ranks other notes in `directory` by cosine similarity over term-frequency
vectors, boosted slightly by shared tags. Returns `{ path, score, shared_terms, shared_tags }` —
powers "see also" links and RAG neighbor expansion.

## `summarize_document`
Extractive TL;DR: ranks sentences by keyword density plus a small position bias and returns the
top `sentences` (default 3) in original order. Deterministic, no model.

## `extract_qa_pairs`
Mines `Q:/A:` lines and `?`-terminated headings (answer = following body) into
`{ question, answer, source }` — flashcards, eval sets, or RAG ground-truth.

## `extract_entities`
Lightweight entity extraction: URLs, emails, `YYYY-MM-DD`/`YYYY/MM/DD` dates, and capitalized
multi-word name phrases, aggregated with counts. Heuristic/best-effort.

## `build_knowledge_index`
The flagship export. For a file/content it emits **one JSON artifact** combining:

```json
{
  "source": "...",
  "summary": ["..."],
  "outline": [ { "level", "title", "anchor", "children" } ],
  "tags": [ { "tag", "count" } ],
  "keywords": [ { "term", "score" } ],
  "stats": { "total_words", "distinct_words" },
  "chunks": [ { "id", "text", "metadata" } ]
}
```

An AI agent or vector DB can load this single object to "know" the note — the human/machine bridge.
It composes `chunk_markdown`, `get_document_outline`, `get_text_statistics`, `extract_tags`, and
`extract_keywords` internally.
