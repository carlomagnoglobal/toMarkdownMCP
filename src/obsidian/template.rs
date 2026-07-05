use chrono::Local;

/// Substitute Obsidian template placeholders: `{{title}}`, `{{date}}`,
/// `{{time}}`, plus `{{date:FORMAT}}` / `{{time:FORMAT}}` with Moment-style
/// formats (YYYY, MM, DD, HH, mm, ss).
pub fn render_template(template: &str, title: &str) -> String {
    let now = Local::now();
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let token = &after[..end];
                let replacement = match token {
                    "title" => title.to_string(),
                    "date" => now.format("%Y-%m-%d").to_string(),
                    "time" => now.format("%H:%M").to_string(),
                    _ => {
                        if let Some(fmt) = token.strip_prefix("date:") {
                            now.format(&super::config::moment_to_chrono(fmt)).to_string()
                        } else if let Some(fmt) = token.strip_prefix("time:") {
                            now.format(&super::config::moment_to_chrono(fmt)).to_string()
                        } else {
                            // Unknown placeholder: keep verbatim.
                            format!("{{{{{}}}}}", token)
                        }
                    }
                };
                out.push_str(&replacement);
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn substitutes_placeholders() {
        let out = render_template("# {{title}}\n{{date}} {{time}} {{unknown}}", "My Day");
        assert!(out.starts_with("# My Day"));
        let year = Local::now().year().to_string();
        assert!(out.contains(&year));
        assert!(out.contains("{{unknown}}"));
    }

    #[test]
    fn custom_date_format() {
        let out = render_template("{{date:YYYY}}", "t");
        assert_eq!(out, Local::now().format("%Y").to_string());
    }
}
