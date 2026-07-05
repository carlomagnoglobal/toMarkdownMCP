use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

/// JsonCanvas 1.0 (Obsidian .canvas) structures.
#[derive(Debug, Deserialize)]
pub struct Canvas {
    #[serde(default)]
    pub nodes: Vec<CanvasNode>,
    #[serde(default)]
    pub edges: Vec<CanvasEdge>,
}

#[derive(Debug, Deserialize)]
pub struct CanvasNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String, // text | file | link | group
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
}

#[derive(Debug, Deserialize)]
pub struct CanvasEdge {
    pub id: String,
    #[serde(rename = "fromNode")]
    pub from_node: String,
    #[serde(rename = "toNode")]
    pub to_node: String,
    #[serde(default)]
    pub label: Option<String>,
}

pub fn parse_canvas(json: &str) -> Result<Canvas> {
    serde_json::from_str(json).context("Invalid .canvas JSON")
}

fn node_title(node: &CanvasNode) -> String {
    match node.node_type.as_str() {
        "text" => node
            .text
            .as_deref()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("(empty)")
            .trim_start_matches('#')
            .trim()
            .to_string(),
        "file" => node.file.clone().unwrap_or_else(|| "(file)".into()),
        "link" => node.url.clone().unwrap_or_else(|| "(link)".into()),
        "group" => node.label.clone().unwrap_or_else(|| "(group)".into()),
        other => format!("({})", other),
    }
}

/// Render a canvas as structured Markdown: groups, nodes (reading order:
/// top-to-bottom, left-to-right), and connections.
pub fn canvas_to_markdown(canvas: &Canvas, name: &str) -> String {
    let mut out = format!("# Canvas: {}\n\n", name);

    let mut nodes: Vec<&CanvasNode> =
        canvas.nodes.iter().filter(|n| n.node_type != "group").collect();
    nodes.sort_by(|a, b| {
        a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });
    let groups: Vec<&CanvasNode> =
        canvas.nodes.iter().filter(|n| n.node_type == "group").collect();

    if !groups.is_empty() {
        out.push_str("## Groups\n\n");
        for g in &groups {
            let members: Vec<String> = nodes
                .iter()
                .filter(|n| {
                    n.x >= g.x && n.y >= g.y && n.x + n.width <= g.x + g.width && n.y + n.height <= g.y + g.height
                })
                .map(|n| node_title(n))
                .collect();
            out.push_str(&format!(
                "- **{}**: {}\n",
                g.label.as_deref().unwrap_or("(unnamed)"),
                if members.is_empty() { "(empty)".into() } else { members.join(", ") }
            ));
        }
        out.push('\n');
    }

    out.push_str("## Nodes\n\n");
    for n in &nodes {
        match n.node_type.as_str() {
            "text" => {
                out.push_str(&format!("### {}\n\n", node_title(n)));
                if let Some(text) = &n.text {
                    let rest: Vec<&str> = text.lines().skip(1).collect();
                    if !rest.is_empty() {
                        out.push_str(rest.join("\n").trim());
                        out.push_str("\n\n");
                    }
                }
            }
            "file" => out.push_str(&format!("### 📄 [[{}]]\n\n", n.file.as_deref().unwrap_or(""))),
            "link" => out.push_str(&format!("### 🔗 <{}>\n\n", n.url.as_deref().unwrap_or(""))),
            _ => {}
        }
    }

    if !canvas.edges.is_empty() {
        let titles: HashMap<&str, String> =
            canvas.nodes.iter().map(|n| (n.id.as_str(), node_title(n))).collect();
        out.push_str("## Connections\n\n");
        for e in &canvas.edges {
            let from = titles.get(e.from_node.as_str()).cloned().unwrap_or_else(|| e.from_node.clone());
            let to = titles.get(e.to_node.as_str()).cloned().unwrap_or_else(|| e.to_node.clone());
            match &e.label {
                Some(l) => out.push_str(&format!("- {} —({})→ {}\n", from, l, to)),
                None => out.push_str(&format!("- {} → {}\n", from, to)),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn converts_fixture_canvas() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini_vault/Board.canvas");
        let json = std::fs::read_to_string(path).unwrap();
        let canvas = parse_canvas(&json).unwrap();
        assert_eq!(canvas.nodes.len(), 4);
        assert_eq!(canvas.edges.len(), 2);

        let md = canvas_to_markdown(&canvas, "Board");
        assert!(md.contains("# Canvas: Board"));
        assert!(md.contains("**Workspace**"));
        assert!(md.contains("[[Note A.md]]"));
        assert!(md.contains("<https://example.com>"));
        assert!(md.contains("Idea —(expands on)→ Note A.md"));
    }

    #[test]
    fn rejects_bad_json() {
        assert!(parse_canvas("not json").is_err());
    }
}
