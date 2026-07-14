# Lightweight Markup Conversion

Text-based markup formats are converted to real Markdown (instead of being wrapped in a code
fence) by `src/markup_converter.rs`. These are hand-rolled line/inline mappers — readable output,
not perfect fidelity.

| Format | Extensions | Headings | Inline | Other |
|--------|-----------|----------|--------|-------|
| MediaWiki | `wiki`, `mediawiki` | `== H ==` → `##` | `'''b'''`→`**b**`, `''i''`→`*i*`, `[[A\|L]]`→`[L](A)` | `*`/`#` lists |
| reStructuredText | `rst` | underline style → `#`/`##`/… (order-based levels) | ` ``code`` `→`` `code` `` | `-`/`*` bullets |
| AsciiDoc | `adoc`, `asciidoc` | `= T`, `== S` | — | `NOTE:`/`TIP:`/… → blockquote admonitions, `*` lists |
| Org-mode | `org` | `*` depth → heading level | `[[link][label]]`, `=code=`/`~code~` | `#+BEGIN_SRC lang` → fenced code, `#+TITLE:` → `#` |
| LaTeX | `tex`, `latex` | `\section`→`#`, `\subsection`→`##`, … | `\textbf{}`→`**`, `\emph{}`/`\textit{}`→`*`, `\texttt{}`→`` ` `` | strips preamble, `\item`→`-` |
| Textile | `textile` | `h1.`–`h6.` → `#`–`######` | `@code@`→`` `code` `` | `*` bullets |

Notes:
- reStructuredText heading levels are assigned in the order underline characters first appear.
- LaTeX conversion is best-effort and intentionally lossy (math and complex macros are not
  translated).
- Extensions that were previously detected as source languages (`rst`, `adoc`, `tex`) now route to
  this converter via `handle_convert_file`.
