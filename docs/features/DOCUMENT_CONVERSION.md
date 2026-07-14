# Document, Office, Email, Ebook & Feed Conversion

`toMarkdownMCP` converts a wide range of real-world file formats to Markdown through the
`convert_file`, `batch_convert_files`, and `get_file_summary` tools. Binary formats are detected by
extension and routed to dedicated converters before the plain-text path.

## Supported formats

| Category | Extensions | Module | Notes |
|----------|-----------|--------|-------|
| Documents | `pdf`, `docx`, `doc`, `rtf`, `odt` | `document_converter` | See per-format notes below |
| Spreadsheets | `xlsx`, `xls`, `xlsm`, `ods`, `csv` | `office_converter` | Each sheet → a Markdown table |
| Presentations | `pptx`, `odp` | `office_converter` | One `## Slide N` heading + bullets per slide |
| Email | `eml` | `feed_email_converter` | Headers → YAML frontmatter; HTML body reused via `html_converter` |
| Ebooks | `epub`, `mobi`, `azw`, `azw3` | `feed_email_converter` | Chapters converted through `html_converter` |
| Feeds | `rss`, `atom`, `feed` | `feed_email_converter` | Each item → `##` heading, link, date, content |
| Markup | `wiki`, `mediawiki`, `rst`, `adoc`, `asciidoc`, `org`, `tex`, `latex`, `textile` | `markup_converter` | Converted to real Markdown, not code fences |

## Per-format behavior

- **PDF** (`pdf-extract`): extracts the text layer and normalizes whitespace into paragraphs.
  Scanned/image-only PDFs have no text layer — the tool returns a clear note (no OCR).
- **DOCX** (`zip` + `quick-xml`): maps Word heading styles (`Heading1..6`) to `#`..`######`,
  list paragraphs to `-`, and bold/italic runs to `**`/`*`.
- **DOC** (legacy OLE): best-effort printable-text extraction; if unreliable, returns a note
  recommending conversion to `.docx`.
- **RTF**: control-word stripper that keeps text and paragraph breaks (`\par`).
- **ODT** (`zip` + `quick-xml`): parses `content.xml`, mapping `text:h` outline levels to headings.
- **Spreadsheets** (`calamine`) / **CSV** (`csv`): the first row becomes the table header; pipe and
  newline characters in cells are escaped.
- **Presentations**: text frames (`<a:t>`) are collected per slide as bullet points.
- **EML** (`mail-parser`): `subject`/`from`/`to`/`date` become YAML frontmatter; the HTML body is
  converted, falling back to the plain-text body.
- **EPUB** (`epub`) / **MOBI** (`mobi`): spine/records are iterated and each HTML chapter is
  converted; DRM-protected files may yield no content (documented note).
- **RSS/Atom** (`feed-rs`): channel title/description plus each entry's title, link, date, and
  content/summary.

## Markup formats

See `MARKUP_CONVERSION.md` for the wiki/rST/AsciiDoc/Org/LaTeX/Textile mappings.

## AI / RAG / knowledge tools

See `RAG_TOOLS.md` and `SECOND_BRAIN_TOOLS.md` for chunking, outlines, statistics, search, and the
knowledge-layer tools that operate on any converted format.
