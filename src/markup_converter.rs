//! Lightweight-markup → Markdown converters (wiki, reStructuredText, AsciiDoc,
//! Org-mode, LaTeX, Textile). All are hand-rolled line/inline mappers to avoid
//! heavy dependencies; they aim for readable Markdown, not perfect fidelity.

/// Extensions handled by this module.
pub fn is_markup_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "wiki" | "mediawiki" | "rst" | "adoc" | "asciidoc" | "org" | "tex" | "latex" | "textile"
    )
}

/// Convert markup content to Markdown based on the file extension.
pub fn convert_markup(ext: &str, content: &str) -> String {
    match ext.to_lowercase().as_str() {
        "wiki" | "mediawiki" => mediawiki_to_md(content),
        "rst" => rst_to_md(content),
        "adoc" | "asciidoc" => asciidoc_to_md(content),
        "org" => org_to_md(content),
        "tex" | "latex" => latex_to_md(content),
        "textile" => textile_to_md(content),
        _ => content.to_string(),
    }
}

// ----------------------------------------------------------------------------
// MediaWiki
// ----------------------------------------------------------------------------

fn mediawiki_to_md(input: &str) -> String {
    let mut out = String::new();
    for line in input.lines() {
        let trimmed = line.trim_end();

        // Headings: == Title ==  ->  ## Title
        if let Some((level, text)) = wiki_heading(trimmed) {
            out.push_str(&format!("{} {}\n", "#".repeat(level), inline_wiki(text)));
            continue;
        }

        // Unordered / ordered list items (*, #), possibly nested.
        if let Some(rest) = trimmed.strip_prefix('*') {
            let depth = trimmed.len() - trimmed.trim_start_matches('*').len();
            out.push_str(&format!(
                "{}- {}\n",
                "  ".repeat(depth.saturating_sub(1)),
                inline_wiki(rest.trim_start_matches('*').trim())
            ));
            continue;
        }
        if let Some(_rest) = trimmed.strip_prefix('#') {
            let depth = trimmed.len() - trimmed.trim_start_matches('#').len();
            out.push_str(&format!(
                "{}1. {}\n",
                "  ".repeat(depth.saturating_sub(1)),
                inline_wiki(trimmed.trim_start_matches('#').trim())
            ));
            continue;
        }

        out.push_str(&inline_wiki(trimmed));
        out.push('\n');
    }
    out
}

/// Parse a MediaWiki heading line like `=== Foo ===` into (level, text).
fn wiki_heading(line: &str) -> Option<(usize, &str)> {
    let line = line.trim();
    if !line.starts_with('=') || !line.ends_with('=') || line.len() < 2 {
        return None;
    }
    let level = line.len() - line.trim_start_matches('=').len();
    let closing = line.len() - line.trim_end_matches('=').len();
    if level == 0 || level != closing {
        return None;
    }
    let text = &line[level..line.len() - closing];
    Some((level.min(6), text.trim()))
}

/// Inline MediaWiki markup: '''bold''', ''italic'', [[links]], [ext links].
fn inline_wiki(s: &str) -> String {
    let mut s = s.to_string();
    s = s.replace("'''", "**").replace("''", "*");
    s = replace_wikilinks(&s);
    s
}

/// Convert `[[Target|Label]]` and `[[Target]]` to Markdown links/text.
fn replace_wikilinks(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(end) = after.find("]]") {
            let inner = &after[..end];
            let (target, label) = match inner.split_once('|') {
                Some((t, l)) => (t.trim(), l.trim()),
                None => (inner.trim(), inner.trim()),
            };
            out.push_str(&format!("[{}]({})", label, target.replace(' ', "_")));
            rest = &after[end + 2..];
        } else {
            out.push_str("[[");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

// ----------------------------------------------------------------------------
// reStructuredText (subset)
// ----------------------------------------------------------------------------

fn rst_to_md(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    // Track the order in which underline chars appear to assign heading levels.
    let mut levels: Vec<char> = Vec::new();

    while i < lines.len() {
        let line = lines[i];
        let next = lines.get(i + 1).copied().unwrap_or("");

        // Section heading: a line of text followed by a line of repeated punctuation.
        if !line.trim().is_empty() && is_rst_underline(next, line.trim().len()) {
            let uc = next.trim().chars().next().unwrap();
            let level = match levels.iter().position(|&c| c == uc) {
                Some(p) => p + 1,
                None => {
                    levels.push(uc);
                    levels.len()
                }
            };
            out.push_str(&format!("{} {}\n\n", "#".repeat(level.min(6)), line.trim()));
            i += 2;
            continue;
        }

        // Bullet list.
        let t = line.trim_start();
        if t.starts_with("- ") || t.starts_with("* ") {
            let indent = line.len() - t.len();
            out.push_str(&format!("{}- {}\n", " ".repeat(indent), inline_rst(&t[2..])));
            i += 1;
            continue;
        }

        // Literal block marker.
        if line.trim() == "::" {
            i += 1;
            continue;
        }

        out.push_str(&inline_rst(line));
        out.push('\n');
        i += 1;
    }
    out
}

fn is_rst_underline(line: &str, min_len: usize) -> bool {
    let t = line.trim();
    if t.len() < min_len.max(2) {
        return false;
    }
    let c = match t.chars().next() {
        Some(c) => c,
        None => return false,
    };
    "=-~^\"#*+.:_".contains(c) && t.chars().all(|ch| ch == c)
}

/// Inline rST: **bold**, *italic*, ``code``, `text <url>`_ links.
fn inline_rst(s: &str) -> String {
    let mut s = s.to_string();
    s = s.replace("``", "`");
    s
}

// ----------------------------------------------------------------------------
// AsciiDoc (subset)
// ----------------------------------------------------------------------------

fn asciidoc_to_md(input: &str) -> String {
    let mut out = String::new();
    for line in input.lines() {
        let trimmed = line.trim_end();

        // Headings: "= Title", "== Section", ...
        if let Some(level) = adoc_heading_level(trimmed) {
            let text = trimmed[level..].trim();
            out.push_str(&format!("{} {}\n", "#".repeat(level.min(6)), text));
            continue;
        }

        // Bullet lists use '*' or '-'.
        let t = trimmed.trim_start();
        if let Some(rest) = t.strip_prefix("* ") {
            let depth = t.len() - t.trim_start_matches('*').len();
            out.push_str(&format!("{}- {}\n", "  ".repeat(depth.saturating_sub(1)), rest.trim()));
            continue;
        }

        // Admonitions: NOTE:, TIP:, WARNING:, etc.
        if let Some((kind, body)) = adoc_admonition(trimmed) {
            out.push_str(&format!("> **{}:** {}\n", kind, body.trim()));
            continue;
        }

        out.push_str(&inline_adoc(trimmed));
        out.push('\n');
    }
    out
}

fn adoc_heading_level(line: &str) -> Option<usize> {
    if !line.starts_with('=') {
        return None;
    }
    let level = line.len() - line.trim_start_matches('=').len();
    if level >= 1 && line.as_bytes().get(level) == Some(&b' ') {
        Some(level)
    } else {
        None
    }
}

fn adoc_admonition(line: &str) -> Option<(&str, &str)> {
    for kind in ["NOTE", "TIP", "IMPORTANT", "WARNING", "CAUTION"] {
        if let Some(rest) = line.strip_prefix(kind) {
            if let Some(body) = rest.strip_prefix(": ") {
                return Some((kind, body));
            }
        }
    }
    None
}

/// Inline AsciiDoc: *bold*, _italic_, `mono`.
fn inline_adoc(s: &str) -> String {
    s.to_string()
}

// ----------------------------------------------------------------------------
// Org-mode (subset)
// ----------------------------------------------------------------------------

fn org_to_md(input: &str) -> String {
    let mut out = String::new();
    let mut in_src = false;
    for line in input.lines() {
        let trimmed = line.trim_end();
        let lower = trimmed.trim().to_lowercase();

        // Source blocks.
        if lower.starts_with("#+begin_src") {
            let lang = trimmed.trim()[11..].split_whitespace().next().unwrap_or("");
            out.push_str(&format!("```{}\n", lang));
            in_src = true;
            continue;
        }
        if lower.starts_with("#+end_src") {
            out.push_str("```\n");
            in_src = false;
            continue;
        }
        if in_src {
            out.push_str(trimmed);
            out.push('\n');
            continue;
        }

        // Other #+KEYWORD lines (title, author) -> skip or render title.
        if let Some(title) = trimmed.trim().strip_prefix("#+TITLE:").or_else(|| trimmed.trim().strip_prefix("#+title:")) {
            out.push_str(&format!("# {}\n", title.trim()));
            continue;
        }
        if lower.starts_with("#+") {
            continue;
        }

        // Headings: leading '*' count is the level.
        if trimmed.starts_with('*') {
            let level = trimmed.len() - trimmed.trim_start_matches('*').len();
            if trimmed.as_bytes().get(level) == Some(&b' ') {
                let text = trimmed[level..].trim();
                out.push_str(&format!("{} {}\n", "#".repeat(level.min(6)), inline_org(text)));
                continue;
            }
        }

        // Lists: '-' or '+'.
        let t = trimmed.trim_start();
        if t.starts_with("- ") || t.starts_with("+ ") {
            let indent = trimmed.len() - t.len();
            out.push_str(&format!("{}- {}\n", " ".repeat(indent), inline_org(&t[2..])));
            continue;
        }

        out.push_str(&inline_org(trimmed));
        out.push('\n');
    }
    out
}

/// Inline Org: *bold*, /italic/, =code=, ~code~, [[link][label]].
fn inline_org(s: &str) -> String {
    let mut s = s.to_string();
    s = s.replace(['=', '~'], "`");
    // [[link][label]] -> [label](link)
    while let Some(start) = s.find("[[") {
        if let Some(end) = s[start..].find("]]") {
            let inner = &s[start + 2..start + end];
            let replacement = match inner.split_once("][") {
                Some((link, label)) => format!("[{}]({})", label, link),
                None => format!("[{}]({})", inner, inner),
            };
            s.replace_range(start..start + end + 2, &replacement);
        } else {
            break;
        }
    }
    s
}

// ----------------------------------------------------------------------------
// LaTeX (best-effort subset)
// ----------------------------------------------------------------------------

fn latex_to_md(input: &str) -> String {
    let mut out = String::new();
    let mut in_body = !input.contains("\\begin{document}");

    for line in input.lines() {
        let trimmed = line.trim();

        if trimmed.contains("\\begin{document}") {
            in_body = true;
            continue;
        }
        if trimmed.contains("\\end{document}") {
            break;
        }
        if !in_body || trimmed.starts_with('%') {
            continue;
        }

        // Sectioning commands.
        if let Some((level, text)) = latex_section(trimmed) {
            out.push_str(&format!("{} {}\n\n", "#".repeat(level), inline_latex(text)));
            continue;
        }
        if let Some(item) = trimmed.strip_prefix("\\item") {
            out.push_str(&format!("- {}\n", inline_latex(item.trim())));
            continue;
        }
        // Skip environment markers we don't translate.
        if trimmed.starts_with("\\begin{") || trimmed.starts_with("\\end{") {
            continue;
        }

        out.push_str(&inline_latex(trimmed));
        out.push('\n');
    }
    out
}

fn latex_section(line: &str) -> Option<(usize, &str)> {
    for (cmd, level) in [
        ("\\section{", 1usize),
        ("\\subsection{", 2),
        ("\\subsubsection{", 3),
        ("\\paragraph{", 4),
    ] {
        if let Some(rest) = line.strip_prefix(cmd) {
            if let Some(end) = rest.find('}') {
                return Some((level, &rest[..end]));
            }
        }
    }
    None
}

/// Inline LaTeX: \textbf{}, \emph{}/\textit{}, \texttt{}.
fn inline_latex(s: &str) -> String {
    let mut s = s.to_string();
    for (cmd, open, close) in [
        ("\\textbf{", "**", "**"),
        ("\\textit{", "*", "*"),
        ("\\emph{", "*", "*"),
        ("\\texttt{", "`", "`"),
    ] {
        s = replace_braced_command(&s, cmd, open, close);
    }
    s
}

/// Replace `\cmd{content}` occurrences with `open content close`.
fn replace_braced_command(s: &str, cmd: &str, open: &str, close: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find(cmd) {
        out.push_str(&rest[..start]);
        let after = &rest[start + cmd.len()..];
        if let Some(end) = after.find('}') {
            out.push_str(open);
            out.push_str(&after[..end]);
            out.push_str(close);
            rest = &after[end + 1..];
        } else {
            out.push_str(cmd);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

// ----------------------------------------------------------------------------
// Textile (subset)
// ----------------------------------------------------------------------------

fn textile_to_md(input: &str) -> String {
    let mut out = String::new();
    for line in input.lines() {
        let trimmed = line.trim_end();
        // Headings: "h1. Title" .. "h6. Title"
        if trimmed.len() > 3
            && trimmed.starts_with('h')
            && trimmed.as_bytes()[1].is_ascii_digit()
            && trimmed[2..].starts_with(". ")
        {
            let level = (trimmed.as_bytes()[1] - b'0') as usize;
            out.push_str(&format!("{} {}\n", "#".repeat(level.min(6)), trimmed[4..].trim()));
            continue;
        }
        // Bullet list.
        if let Some(rest) = trimmed.strip_prefix("* ") {
            out.push_str(&format!("- {}\n", rest.trim()));
            continue;
        }
        out.push_str(&inline_textile(trimmed));
        out.push('\n');
    }
    out
}

/// Inline Textile: *bold*, _italic_, @code@.
fn inline_textile(s: &str) -> String {
    s.replace('@', "`")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mediawiki_headings_and_inline() {
        let md = mediawiki_to_md("== Section ==\n'''bold''' and ''italic''\n[[Page|Label]]");
        assert!(md.contains("## Section"), "got: {md}");
        assert!(md.contains("**bold**"), "got: {md}");
        assert!(md.contains("*italic*"), "got: {md}");
        assert!(md.contains("[Label](Page)"), "got: {md}");
    }

    #[test]
    fn test_mediawiki_lists() {
        let md = mediawiki_to_md("* one\n* two");
        assert!(md.contains("- one"), "got: {md}");
        assert!(md.contains("- two"), "got: {md}");
    }

    #[test]
    fn test_rst_headings() {
        let md = rst_to_md("Title\n=====\n\nSub\n---\n");
        assert!(md.contains("# Title"), "got: {md}");
        assert!(md.contains("## Sub"), "got: {md}");
    }

    #[test]
    fn test_asciidoc_headings_and_admonition() {
        let md = asciidoc_to_md("= Doc Title\n== Section\nNOTE: be careful");
        assert!(md.contains("# Doc Title"), "got: {md}");
        assert!(md.contains("## Section"), "got: {md}");
        assert!(md.contains("> **NOTE:** be careful"), "got: {md}");
    }

    #[test]
    fn test_org_headings_and_src() {
        let md = org_to_md("* Head\n#+BEGIN_SRC rust\nfn main() {}\n#+END_SRC");
        assert!(md.contains("# Head"), "got: {md}");
        assert!(md.contains("```rust"), "got: {md}");
        assert!(md.contains("fn main()"), "got: {md}");
    }

    #[test]
    fn test_latex_sections_and_bold() {
        let md = latex_to_md("\\begin{document}\n\\section{Intro}\n\\textbf{hi}\n\\end{document}");
        assert!(md.contains("# Intro"), "got: {md}");
        assert!(md.contains("**hi**"), "got: {md}");
    }

    #[test]
    fn test_textile_headings() {
        let md = textile_to_md("h2. Hello\n* item");
        assert!(md.contains("## Hello"), "got: {md}");
        assert!(md.contains("- item"), "got: {md}");
    }
}
