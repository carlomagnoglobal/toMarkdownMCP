# AI Tools

Beyond the RAG toolkit (`RAG_TOOLS.md`) and second-brain tools (`SECOND_BRAIN_TOOLS.md`), the
server provides AI-oriented tools in two tiers: **local/deterministic** (no network, no cost) and
**optional Claude-backed** (activate only when an API key is set). Every tool works on any of the
~30 supported input formats (converted first) and honors `output_format: "json"` where applicable.

## Local tools (no API key required)

### Retrieval & budgeting
- **`retrieve_context`** — the retrieval step of RAG. Ranks chunks across a `directory` (or
  `file_path`) against a `query` and assembles the top ones into a single context block under a
  `max_tokens` budget (default 2000), with citations `{source, heading_path, score}`. This is what
  you feed an LLM.
- **`count_tokens`** — estimates tokens for a file/content (~words × 1.3) and shows whether it fits
  each model's context window (Opus 4.8 / Sonnet 5 / Haiku 4.5).

### Dedup & clustering
- **`find_duplicates`** — near-duplicate detection across a directory via **SimHash** (64-bit
  fingerprints, Hamming-distance `threshold` in bits, default 3). Returns groups of similar files.
- **`cluster_documents`** — greedy topic clustering by **cosine similarity** over term vectors
  (`min_similarity`, default 0.2). Returns labeled groups with top terms.

### Document intelligence
- **`analyze_readability`** — Flesch Reading Ease + Flesch-Kincaid grade, with word/sentence/
  syllable counts (heuristic syllable counter).
- **`detect_natural_language`** — natural-language detection (English, Spanish, French, German,
  Portuguese, Italian) via function-word hit rate. Ranked guesses with confidence.
- **`classify_document`** — heuristic topic/content-type classification (technical, finance, legal,
  correspondence, academic, marketing) via keyword signals.

## Claude-backed tools (require `ANTHROPIC_API_KEY`)

These call the Anthropic Messages API (`src/llm.rs`). If no key is configured, each returns a clear
setup note instead of erroring, so the server stays fully usable offline.

Configuration (server environment):
- `ANTHROPIC_API_KEY` — required to activate.
- `ANTHROPIC_MODEL` — optional default model (falls back to `claude-haiku-4-5-20251001`).
- `ANTHROPIC_BASE_URL` — optional API base override.
- Each tool also accepts a per-call `model` argument (e.g. `claude-opus-4-8` for higher quality).

Tools:
- **`ai_summarize`** — abstractive summary (`style`, `max_tokens`). Local alternative:
  `summarize_document`.
- **`ai_ask`** — RAG Q&A. Builds context with `retrieve_context`, then answers grounded in it,
  returning the answer plus citations.
- **`ai_tag`** — suggest topical tags. Local alternative: `extract_keywords`.
- **`ai_translate`** — translate to `target_language`, preserving Markdown.
- **`ai_classify`** — classify into caller-provided `labels`. Local alternative:
  `classify_document`.

> Note: Claude-backed tools incur API cost and require network access. Live calls are not exercised
> by the test suite; the client is unit-tested for the no-key path and model resolution only.
