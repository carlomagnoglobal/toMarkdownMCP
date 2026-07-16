//! Markdown → HTML rendering for the viewer: syntect-highlighted code,
//! Obsidian syntax (callouts, ==highlights==, %%comments%%, wikilinks,
//! embeds with transclusion), math/mermaid placeholders for the vendored
//! client-side renderers, and local-media inlining as data: URLs.

use std::path::{Path, PathBuf};

use pulldown_cmark::{html, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use syntect::html::{css_for_theme_with_class_style, ClassStyle, ClassedHTMLGenerator};
use syntect::util::LinesWithEndings;

use to_markdown_mcp::obsidian::{callout, vault, wikilink};

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico"];
const AUDIO_EXTS: &[&str] = &["mp3", "wav", "m4a", "ogg", "flac"];
const VIDEO_EXTS: &[&str] = &["mp4", "webm", "mov", "ogv"];
const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_MEDIA_BYTES: u64 = 20 * 1024 * 1024;
const MAX_EMBED_DEPTH: usize = 2;

pub struct RenderOpts<'a> {
    /// Directory of the file being rendered (for relative media paths).
    pub file_dir: Option<&'a Path>,
    /// Vault root: enables wikilinks, transclusion, and attachment lookup.
    pub vault_root: Option<&'a Path>,
}

impl RenderOpts<'_> {
    #[cfg_attr(not(test), allow(dead_code))] // used by render tests and plain contexts
    pub const PLAIN: RenderOpts<'static> = RenderOpts { file_dir: None, vault_root: None };
}

static SYNTECT: once_cell::sync::Lazy<(
    syntect::parsing::SyntaxSet,
    syntect::highlighting::ThemeSet,
)> = once_cell::sync::Lazy::new(|| {
    (
        syntect::parsing::SyntaxSet::load_defaults_newlines(),
        syntect::highlighting::ThemeSet::load_defaults(),
    )
});

/// Class-based syntax CSS for both app themes, scoped so the frontend can
/// inject it once and have code follow the active theme.
pub fn syntax_css() -> String {
    let themes = &SYNTECT.1;
    let light = css_for_theme_with_class_style(&themes.themes["InspiredGitHub"], ClassStyle::Spaced)
        .unwrap_or_default();
    let dark = css_for_theme_with_class_style(&themes.themes["base16-eighties.dark"], ClassStyle::Spaced)
        .unwrap_or_default();
    // Scope: light rules by default and for the light/sepia themes; dark rules
    // under data-theme="dark" and under system-dark when following the OS.
    let scope = |css: &str, prefix: &str| -> String {
        css.lines()
            .map(|l| {
                let t = l.trim_start();
                if t.starts_with('.') || t.starts_with("code") {
                    format!("{} {}", prefix, l)
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "{}\n{}\n@media (prefers-color-scheme: dark) {{\n{}\n}}\n",
        scope(&light, ":root"),
        scope(&dark, ":root[data-theme=\"dark\"]"),
        scope(&dark, ":root[data-theme=\"system\"]"),
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn percent_encode(s: &str) -> String {
    s.replace('%', "%25").replace(' ', "%20").replace('"', "%22")
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b1 = chunk[0] as u32;
        let b2 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b3 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b1 << 16) | (b2 << 8) | b3;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "ogv" => "video/ogg",
        _ => "application/octet-stream",
    }
}

// ---- Inline Obsidian syntax (outside code fences) ----

/// Line-based pass applying `%%comment%%` stripping and `==highlight==` →
/// `<mark>` outside fenced code blocks; also extracts `$math$` into
/// placeholder tags the frontend renders with vendored KaTeX.
fn apply_inline_syntax(md: &str) -> String {
    let mut out = Vec::new();
    let mut in_fence = false;
    let mut in_comment = false;
    for line in md.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push(line.to_string());
            continue;
        }
        if in_fence {
            out.push(line.to_string());
            continue;
        }
        out.push(transform_inline_line(line, &mut in_comment));
    }
    out.join("\n")
}

fn transform_inline_line(line: &str, in_comment: &mut bool) -> String {
    let mut result = String::with_capacity(line.len() + 16);
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut in_backtick = false;
    let mut in_mark = false;
    let mut math: Option<(bool, String)> = None; // (block, buffer)
    while i < chars.len() {
        let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
        if let Some((block, buf)) = &mut math {
            // Collect math until the closing delimiter.
            if (*block && two == "$$") || (!*block && chars[i] == '$') {
                let tag = if *block { "div class=\"math-block\"" } else { "span class=\"math-inline\"" };
                let close = if *block { "div" } else { "span" };
                result.push_str(&format!(
                    "<{} data-tex=\"{}\"></{}>",
                    tag,
                    percent_encode(&escape_html(buf)),
                    close
                ));
                i += if *block { 2 } else { 1 };
                math = None;
            } else {
                buf.push(chars[i]);
                i += 1;
            }
            continue;
        }
        if *in_comment {
            if two == "%%" {
                *in_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if chars[i] == '`' {
            in_backtick = !in_backtick;
            result.push('`');
            i += 1;
            continue;
        }
        if in_backtick {
            result.push(chars[i]);
            i += 1;
            continue;
        }
        match two.as_str() {
            "%%" => {
                *in_comment = true;
                i += 2;
            }
            "==" => {
                result.push_str(if in_mark { "</mark>" } else { "<mark>" });
                in_mark = !in_mark;
                i += 2;
            }
            "$$" => {
                math = Some((true, String::new()));
                i += 2;
            }
            _ if chars[i] == '$'
                && i + 1 < chars.len()
                && !chars[i + 1].is_whitespace()
                && chars[i + 1] != '$' =>
            {
                math = Some((false, String::new()));
                i += 1;
            }
            _ if chars[i] == '#'
                && (i == 0 || chars[i - 1].is_whitespace())
                && i + 1 < chars.len()
                && (chars[i + 1].is_alphanumeric() || chars[i + 1] == '_') =>
            {
                // Inline #tag → clickable anchor (headings have '# ' and never match).
                let mut j = i + 1;
                while j < chars.len()
                    && (chars[j].is_alphanumeric() || matches!(chars[j], '_' | '-' | '/'))
                {
                    j += 1;
                }
                let tag: String = chars[i + 1..j].iter().collect();
                result.push_str(&format!("[#{}](hashtag:{})", tag, percent_encode(&tag)));
                i = j;
            }
            _ => {
                result.push(chars[i]);
                i += 1;
            }
        }
    }
    // Unterminated constructs degrade gracefully to literal text.
    if let Some((block, buf)) = math {
        result.push_str(if block { "$$" } else { "$" });
        result.push_str(&buf);
    }
    if in_mark {
        result.push_str("</mark>");
    }
    result
}

// ---- Callouts ----

fn render_callouts(md: &str, opts: &RenderOpts, depth: usize) -> String {
    let callouts = callout::parse_callouts(md);
    if callouts.is_empty() {
        return md.to_string();
    }
    let lines: Vec<&str> = md.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut skip_until = 0usize; // 1-indexed line after the current callout
    let mut idx = 0usize;
    for (n, line) in lines.iter().enumerate() {
        let lineno = n + 1;
        if lineno < skip_until {
            continue;
        }
        if idx < callouts.len() && callouts[idx].line == lineno {
            let c = &callouts[idx];
            idx += 1;
            skip_until = lineno + 1 + c.body.lines().count();
            let title = c.title.clone().unwrap_or_else(|| {
                let mut k = c.kind.clone();
                if let Some(f) = k.get_mut(0..1) {
                    f.make_ascii_uppercase();
                }
                k
            });
            let body_html = render_note(&c.body, opts, depth + 1);
            let inner = format!(
                "<div class=\"callout-title\">{}</div><div class=\"callout-body\">{}</div>",
                escape_html(&title),
                body_html
            );
            let html = match c.fold {
                Some(f) => format!(
                    "<details class=\"callout callout-{}\"{}><summary>{}</summary><div class=\"callout-body\">{}</div></details>",
                    escape_html(&c.kind),
                    if f == '+' { " open" } else { "" },
                    escape_html(&title),
                    body_html
                ),
                None => format!(
                    "<div class=\"callout callout-{}\">{}</div>",
                    escape_html(&c.kind),
                    inner
                ),
            };
            out.push(String::new());
            out.push(html);
            out.push(String::new());
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

// ---- Wikilink embeds & links ----

fn resolve_media(target: &str, opts: &RenderOpts) -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = [
        opts.file_dir.map(|d| d.join(target)),
        opts.vault_root.map(|r| r.join(target)),
    ]
    .into_iter()
    .flatten()
    .collect();
    for c in &candidates {
        if c.is_file() {
            return Some(c.clone());
        }
    }
    // Obsidian-style shortest-path lookup anywhere in the vault by filename.
    let root = opts.vault_root?;
    let name = Path::new(target).file_name()?.to_string_lossy().to_lowercase();
    let mut found: Vec<PathBuf> = Vec::new();
    walk_for_file(root, &name, &mut found, 0);
    found.sort_by_key(|p| p.components().count());
    found.into_iter().next()
}

fn walk_for_file(dir: &Path, name: &str, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 8 || out.len() > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let fname = e.file_name().to_string_lossy().to_lowercase();
        if fname.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            walk_for_file(&p, name, out, depth + 1);
        } else if fname == name {
            out.push(p);
        }
    }
}

fn note_section(body: &str, heading: Option<&str>) -> String {
    let Some(h) = heading else { return body.to_string() };
    let mut out = Vec::new();
    let mut level = 0usize;
    let mut collecting = false;
    for line in body.lines() {
        if let Some(stripped) = line.strip_prefix('#') {
            let l = 1 + stripped.len() - stripped.trim_start_matches('#').len();
            let title = stripped.trim_start_matches('#').trim();
            if collecting && l <= level {
                break;
            }
            if !collecting && title.eq_ignore_ascii_case(h.trim()) {
                collecting = true;
                level = l;
                out.push(line.to_string());
                continue;
            }
        }
        if collecting {
            out.push(line.to_string());
        }
    }
    if out.is_empty() { body.to_string() } else { out.join("\n") }
}

/// Expand `![[...]]` embeds: images/PDF/audio/video become media tags
/// (resolved to absolute paths, inlined later), note embeds transclude the
/// target's rendered body. Non-embed wikilinks become `wikilink:` anchors.
fn expand_wikilinks(md: &str, opts: &RenderOpts, depth: usize) -> String {
    let mut out = md.to_string();
    for link in wikilink::parse_wikilinks(md) {
        let replacement = if link.embed {
            embed_html(&link, opts, depth)
        } else {
            anchor_markdown(&link)
        };
        out = out.replace(&link.raw, &replacement);
    }
    out
}

fn anchor_markdown(link: &wikilink::WikiLink) -> String {
    let mut label = link.alias.clone().unwrap_or_else(|| link.target.clone());
    if link.alias.is_none() {
        if let Some(h) = &link.heading {
            label = format!("{} › {}", label, h);
        }
    }
    let href = format!(
        "wikilink:{}{}",
        percent_encode(&link.target),
        link.heading.as_ref().map(|h| format!("#{}", percent_encode(h))).unwrap_or_default(),
    );
    format!("[{}]({})", label, href)
}

fn embed_html(link: &wikilink::WikiLink, opts: &RenderOpts, depth: usize) -> String {
    let ext = Path::new(&link.target)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let media = |tag: &str| -> Option<String> {
        let path = resolve_media(&link.target, opts)?;
        let src = escape_html(&path.to_string_lossy());
        Some(match tag {
            "img" => format!("<img src=\"{}\" alt=\"{}\">", src, escape_html(&link.target)),
            "audio" => format!("<audio controls src=\"{}\"></audio>", src),
            "video" => format!("<video controls src=\"{}\"></video>", src),
            "pdf" => format!(
                "<embed class=\"pdf-embed\" type=\"application/pdf\" src=\"{}\">",
                src
            ),
            _ => unreachable!(),
        })
    };
    let rendered = if IMAGE_EXTS.contains(&ext.as_str()) {
        media("img")
    } else if AUDIO_EXTS.contains(&ext.as_str()) {
        media("audio")
    } else if VIDEO_EXTS.contains(&ext.as_str()) {
        media("video")
    } else if ext == "pdf" {
        media("pdf")
    } else if depth < MAX_EMBED_DEPTH {
        transclude_note(link, opts, depth)
    } else {
        None
    };
    match rendered {
        // Blank lines make pulldown treat the tag as a raw HTML block.
        Some(html) => format!("\n\n{}\n\n", html),
        None => anchor_markdown(link),
    }
}

fn transclude_note(link: &wikilink::WikiLink, opts: &RenderOpts, depth: usize) -> Option<String> {
    let root = opts.vault_root?;
    let idx = vault::get_index(root).ok()?;
    let rel = match vault::resolve_target(&idx, &link.target, None) {
        vault::Resolution::Resolved(r) => r,
        vault::Resolution::Ambiguous(mut c) => {
            c.sort_by_key(|p| p.len());
            c.into_iter().next()?
        }
        vault::Resolution::Broken => return None,
    };
    let abs = root.join(&rel);
    let content = std::fs::read_to_string(&abs).ok()?;
    let (_, body) = to_markdown_mcp::obsidian::frontmatter::split(&content);
    let section = note_section(body, link.heading.as_deref());
    let inner_opts = RenderOpts { file_dir: abs.parent(), vault_root: Some(root) };
    let inner = render_note(&section, &inner_opts, depth + 1);
    let title = link.heading.as_ref().map(|h| format!("{} › {}", link.target, h)).unwrap_or_else(|| link.target.clone());
    Some(format!(
        "<div class=\"transclusion\"><div class=\"trans-title\"><a href=\"wikilink:{}\">{}</a></div><div class=\"trans-body\">{}</div></div>",
        percent_encode(&link.target),
        escape_html(&title),
        inner
    ))
}

// ---- Markdown → HTML with highlighted code ----

fn highlight_code(code: &str, lang: &str) -> String {
    let (syntax_set, _) = &*SYNTECT;
    let syntax = syntax_set
        .find_syntax_by_token(lang)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let mut gen = ClassedHTMLGenerator::new_with_class_style(syntax, syntax_set, ClassStyle::Spaced);
    for line in LinesWithEndings::from(code) {
        if gen.parse_html_for_line_which_includes_newline(line).is_err() {
            return format!("<pre><code>{}</code></pre>", escape_html(code));
        }
    }
    format!("<pre class=\"code\"><code>{}</code></pre>", gen.finalize())
}

fn md_to_html(md: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(md, options);

    let mut out = String::with_capacity(md.len() * 2);
    let mut events: Vec<Event> = Vec::new();
    let mut code_lang: Option<String> = None;
    let mut code_buf = String::new();
    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                code_lang = Some(match kind {
                    CodeBlockKind::Fenced(l) => l.to_string(),
                    CodeBlockKind::Indented => String::new(),
                });
                code_buf.clear();
            }
            Event::Text(t) if code_lang.is_some() => code_buf.push_str(&t),
            Event::End(TagEnd::CodeBlock) => {
                let lang = code_lang.take().unwrap_or_default();
                let html = if lang.trim() == "mermaid" {
                    format!("<pre class=\"mermaid\">{}</pre>", escape_html(&code_buf))
                } else {
                    highlight_code(&code_buf, lang.split_whitespace().next().unwrap_or(""))
                };
                events.push(Event::Html(html.into()));
            }
            e => {
                if code_lang.is_none() {
                    events.push(e);
                }
            }
        }
    }
    html::push_html(&mut out, events.into_iter());
    out
}

// ---- Local media inlining ----

/// Rewrite `src="..."` attributes on media tags: local paths become data:
/// URLs so the webview can display them.
fn inline_local_media(html: &str, opts: &RenderOpts) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find("src=\"") {
        let (before, after) = rest.split_at(pos + 5);
        out.push_str(before);
        let Some(end) = after.find('"') else {
            rest = after;
            break;
        };
        let src = &after[..end];
        out.push_str(&resolve_src(src, opts));
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn resolve_src(src: &str, opts: &RenderOpts) -> String {
    if src.starts_with("data:")
        || src.starts_with("http://")
        || src.starts_with("https://")
        || src.starts_with("wikilink:")
    {
        return src.to_string();
    }
    // HTML-unescape the minimal set that can appear in a path attribute.
    let raw = src.replace("&amp;", "&").replace("&quot;", "\"");
    let raw = raw.split('#').next().unwrap_or(&raw);
    let path = PathBuf::from(raw);
    let resolved = if path.is_absolute() {
        path.is_file().then_some(path)
    } else {
        resolve_media(raw, opts)
    };
    let Some(path) = resolved else { return src.to_string() };
    let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default();
    let cap = if IMAGE_EXTS.contains(&ext.as_str()) { MAX_IMAGE_BYTES } else { MAX_MEDIA_BYTES };
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(u64::MAX);
    if size > cap {
        return src.to_string();
    }
    match std::fs::read(&path) {
        Ok(bytes) => format!("data:{};base64,{}", mime_for_ext(&ext), base64_encode(&bytes)),
        Err(_) => src.to_string(),
    }
}

// ---- Entry point ----

/// Full pipeline: Obsidian inline syntax → callouts → wikilinks/embeds →
/// Markdown → HTML with highlighted code → local media inlined.
pub fn render_note(md: &str, opts: &RenderOpts, depth: usize) -> String {
    let md = apply_inline_syntax(md);
    let md = render_callouts(&md, opts, depth);
    let md = if opts.vault_root.is_some() {
        expand_wikilinks(&md, opts, depth)
    } else {
        md
    };
    let html = md_to_html(&md);
    inline_local_media(&html, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_comments_marks_and_math() {
        let html = render_note(
            "keep ==this== and %%not this%% plus $x^2$\n\n```rust\nfn main() {}\n```",
            &RenderOpts::PLAIN,
            0,
        );
        assert!(html.contains("<mark>this</mark>"));
        assert!(!html.contains("not this"));
        assert!(html.contains("math-inline"));
        assert!(html.contains("data-tex=\"x^2\""));
        // Code is class-highlighted, not plain.
        assert!(html.contains("<pre class=\"code\">"), "html: {}", html);
        assert!(html.contains("class=\"source rust\"") || html.contains("keyword"), "html: {}", html);
        // Inline syntax must not fire inside fences.
        let html = render_note("```\n==literal== $a$\n```", &RenderOpts::PLAIN, 0);
        assert!(html.contains("==literal== $a$"));
    }

    #[test]
    fn inline_tags_become_clickable_but_headings_do_not() {
        let html = render_note("# Heading\n\nAbout #project/alpha here.", &RenderOpts::PLAIN, 0);
        assert!(html.contains("href=\"hashtag:project/alpha\""), "html: {}", html);
        assert!(html.contains("<h1>Heading</h1>"), "html: {}", html);
    }

    #[test]
    fn mermaid_fences_become_mermaid_pre_blocks() {
        let html = render_note("```mermaid\ngraph TD; A-->B;\n```", &RenderOpts::PLAIN, 0);
        assert!(html.contains("<pre class=\"mermaid\">"));
        assert!(html.contains("A--&gt;B"));
    }

    #[test]
    fn callouts_render_styled_and_folded() {
        let md = "> [!note] Heads up\n> Body line\n\n> [!tip]- Folded\n> Hidden";
        let html = render_note(md, &RenderOpts::PLAIN, 0);
        assert!(html.contains("callout callout-note"));
        assert!(html.contains("Heads up"));
        assert!(html.contains("<details class=\"callout callout-tip\">"));
        assert!(html.contains("<summary>Folded</summary>"));
    }

    fn vault() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/mini_vault")
            .canonicalize()
            .unwrap()
    }

    #[test]
    fn image_embeds_inline_as_data_urls() {
        let root = vault();
        let opts = RenderOpts { file_dir: Some(&root), vault_root: Some(&root) };
        let html = render_note("![[pixel.png]]", &opts, 0);
        assert!(html.contains("data:image/png;base64,"), "html: {}", html);
        // Standard markdown relative image inlines too.
        let html = render_note("![alt](attachments/pixel.png)", &opts, 0);
        assert!(html.contains("data:image/png;base64,"), "html: {}", html);
        // Missing image degrades to the original src.
        let html = render_note("![alt](nope.png)", &opts, 0);
        assert!(html.contains("nope.png"));
    }

    #[test]
    fn note_embeds_transclude_inline() {
        let root = vault();
        let opts = RenderOpts { file_dir: Some(&root), vault_root: Some(&root) };
        let html = render_note("Intro\n\n![[Note B]]\n", &opts, 0);
        assert!(html.contains("class=\"transclusion\""), "html: {}", html);
        assert!(html.contains("trans-title"));
        // Non-embed wikilinks stay anchors.
        let html = render_note("See [[Note B|second]]", &opts, 0);
        assert!(html.contains("href=\"wikilink:Note%20B\""));
    }

    #[test]
    fn syntax_css_covers_both_themes() {
        let css = syntax_css();
        assert!(css.contains(":root ."));
        assert!(css.contains(":root[data-theme=\"dark\"]"));
    }
}
