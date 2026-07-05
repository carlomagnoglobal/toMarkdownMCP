use serde::Serialize;

/// A parsed Obsidian wikilink: `[[target#heading^block|alias]]` or `![[embed]]`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WikiLink {
    pub raw: String,
    pub target: String,
    pub heading: Option<String>,
    pub block: Option<String>,
    pub alias: Option<String>,
    pub embed: bool,
    /// 1-indexed line number.
    pub line: usize,
    /// 0-indexed byte column of the opening `[[` (or `!`).
    pub col: usize,
}

/// Parse all wikilinks in a Markdown document, skipping fenced code blocks
/// (``` / ~~~) and inline code spans.
pub fn parse_wikilinks(md: &str) -> Vec<WikiLink> {
    let mut links = Vec::new();
    let mut in_fence: Option<char> = None;

    for (line_idx, line) in md.lines().enumerate() {
        let trimmed = line.trim_start();
        // Fence open/close (``` or ~~~, at least 3 chars)
        for marker in ['`', '~'] {
            if trimmed.starts_with(&marker.to_string().repeat(3)) {
                match in_fence {
                    Some(f) if f == marker => {
                        in_fence = None;
                    }
                    None => {
                        in_fence = Some(marker);
                    }
                    _ => {}
                }
            }
        }
        if in_fence.is_some() {
            continue;
        }

        parse_line(line, line_idx + 1, &mut links);
    }

    links
}

fn parse_line(line: &str, line_no: usize, links: &mut Vec<WikiLink>) {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_code = false; // inside an inline `code` span

    while i < bytes.len() {
        if bytes[i] == b'`' {
            in_code = !in_code;
            i += 1;
            continue;
        }
        if in_code {
            i += 1;
            continue;
        }
        if bytes[i] == b'[' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let embed = i > 0 && bytes[i - 1] == b'!';
            let start = if embed { i - 1 } else { i };
            if let Some(rel_end) = line[i + 2..].find("]]") {
                let inner = &line[i + 2..i + 2 + rel_end];
                if !inner.is_empty() {
                    let raw = &line[start..i + 2 + rel_end + 2];
                    links.push(parse_inner(inner, raw, embed, line_no, start));
                }
                i = i + 2 + rel_end + 2;
                continue;
            }
        }
        i += 1;
    }
}

/// Split the inside of `[[...]]` into target / heading / block / alias.
fn parse_inner(inner: &str, raw: &str, embed: bool, line: usize, col: usize) -> WikiLink {
    // Alias separator: `|`, written as `\|` inside Markdown tables — both
    // separate target from alias in Obsidian.
    let (link_part, alias) = match inner.find("\\|") {
        Some(i) => (&inner[..i], Some(inner[i + 2..].trim().to_string())),
        None => match inner.find('|') {
            Some(i) => (&inner[..i], Some(inner[i + 1..].trim().to_string())),
            None => (inner, None),
        },
    };

    // Heading/block: first `#`. `#^id` is a block reference; nested
    // `#a#b` headings are kept whole ("a#b").
    let (target, heading, block) = match link_part.find('#') {
        Some(idx) => {
            let target = &link_part[..idx];
            let frag = &link_part[idx + 1..];
            if let Some(id) = frag.strip_prefix('^') {
                (target, None, Some(id.trim().to_string()))
            } else {
                (target, Some(frag.trim().to_string()), None)
            }
        }
        None => (link_part, None, None),
    };

    WikiLink {
        raw: raw.to_string(),
        target: target.trim().to_string(),
        heading: heading.filter(|s| !s.is_empty()),
        block: block.filter(|s| !s.is_empty()),
        alias: alias.filter(|s| !s.is_empty()),
        embed,
        line,
        col,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(md: &str) -> WikiLink {
        let links = parse_wikilinks(md);
        assert_eq!(links.len(), 1, "expected exactly one link in {:?}", md);
        links.into_iter().next().unwrap()
    }

    #[test]
    fn plain_link() {
        let l = one("see [[Note A]] here");
        assert_eq!(l.target, "Note A");
        assert!(l.alias.is_none() && l.heading.is_none() && l.block.is_none() && !l.embed);
        assert_eq!((l.line, l.col), (1, 4));
    }

    #[test]
    fn alias_heading_block_embed() {
        let l = one("[[Note A|friendly]]");
        assert_eq!(l.alias.as_deref(), Some("friendly"));

        let l = one("[[Note A#Section One]]");
        assert_eq!(l.heading.as_deref(), Some("Section One"));

        let l = one("[[Note A#^abc123]]");
        assert_eq!(l.block.as_deref(), Some("abc123"));

        let l = one("![[image.png]]");
        assert!(l.embed);
        assert_eq!(l.target, "image.png");

        let l = one("[[folder/Note B#Head|Alias]]");
        assert_eq!(l.target, "folder/Note B");
        assert_eq!(l.heading.as_deref(), Some("Head"));
        assert_eq!(l.alias.as_deref(), Some("Alias"));
    }

    #[test]
    fn escaped_pipe_in_table() {
        let l = one("| cell [[Note A\\|alias]] |");
        assert_eq!(l.target, "Note A");
        assert_eq!(l.alias.as_deref(), Some("alias"));
    }

    #[test]
    fn skips_code() {
        assert!(parse_wikilinks("`[[not a link]]`").is_empty());
        assert!(parse_wikilinks("```\n[[not a link]]\n```").is_empty());
        assert!(parse_wikilinks("~~~\n[[nope]]\n~~~").is_empty());
        // Fence closes, link after counts
        let links = parse_wikilinks("```\n[[nope]]\n```\n[[yes]]");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "yes");
        assert_eq!(links[0].line, 4);
    }

    #[test]
    fn multiple_per_line_and_empty() {
        let links = parse_wikilinks("[[A]] and [[B|b]]");
        assert_eq!(links.len(), 2);
        assert!(parse_wikilinks("[[]]").is_empty());
        assert!(parse_wikilinks("[ [not] ]").is_empty());
    }
}
