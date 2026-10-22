//! A tiptap (ProseMirror) document, as HTML: how dayz.com keeps its patch notes.

use serde_json::Value;

const MAX_DEPTH: usize = 48;

/// `doc` as HTML; text is escaped, links keep only their address.
pub fn to_html(doc: &Value) -> String {
    let mut out = String::new();
    node(doc, &mut out, 0);
    out
}

fn node(n: &Value, out: &mut String, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    let kind = n.get("type").and_then(Value::as_str).unwrap_or("");
    let attr = |k: &str| n.get("attrs").and_then(|a| a.get(k));
    let (open, close): (String, &str) = match kind {
        "text" => {
            text(n, out);
            return;
        }
        "hardBreak" => {
            out.push_str("<br>");
            return;
        }
        "horizontalRule" => {
            out.push_str("<hr>");
            return;
        }
        "image" => {
            if let Some(src) = attr("src").and_then(Value::as_str) {
                out.push_str("<img src=\"");
                escape(src, out);
                out.push_str("\">");
            }
            return;
        }
        "paragraph" => ("<p>".into(), "</p>"),
        "heading" => {
            let l = attr("level")
                .and_then(Value::as_u64)
                .unwrap_or(3)
                .clamp(1, 6);
            (
                format!("<h{l}>"),
                ["", "</h1>", "</h2>", "</h3>", "</h4>", "</h5>", "</h6>"][l as usize],
            )
        }
        "bulletList" => ("<ul>".into(), "</ul>"),
        "orderedList" => ("<ol>".into(), "</ol>"),
        "listItem" => ("<li>".into(), "</li>"),
        "blockquote" => ("<blockquote>".into(), "</blockquote>"),
        "codeBlock" => ("<pre>".into(), "</pre>"),
        "table" => ("<table>".into(), "</table>"),
        "tableRow" => ("<tr>".into(), "</tr>"),
        "tableCell" | "tableHeader" => ("<td>".into(), "</td>"),
        _ => (String::new(), ""),
    };
    out.push_str(&open);
    for c in n
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        node(c, out, depth + 1);
    }
    out.push_str(close);
}

/// A text node inside its marks.
fn text(n: &Value, out: &mut String) {
    let Some(t) = n.get("text").and_then(Value::as_str) else {
        return;
    };
    let marks: Vec<&Value> = n
        .get("marks")
        .and_then(Value::as_array)
        .map(|m| m.iter().collect())
        .unwrap_or_default();
    let mut closes = Vec::new();
    for m in &marks {
        match m.get("type").and_then(Value::as_str).unwrap_or("") {
            "bold" => {
                out.push_str("<strong>");
                closes.push("</strong>");
            }
            "italic" => {
                out.push_str("<em>");
                closes.push("</em>");
            }
            "code" => {
                out.push_str("<code>");
                closes.push("</code>");
            }
            "link" => {
                let href = m
                    .get("attrs")
                    .and_then(|a| a.get("href"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                out.push_str("<a href=\"");
                escape(href, out);
                out.push_str("\">");
                closes.push("</a>");
            }
            _ => {}
        }
    }
    escape(t, out);
    for c in closes.iter().rev() {
        out.push_str(c);
    }
}

fn escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lists_marks_and_links() {
        let doc = json!({"type": "doc", "content": [
            {"type": "paragraph", "content": [
                {"type": "text", "marks": [{"type": "bold"}], "text": "NOTES"}]},
            {"type": "bulletList", "content": [{"type": "listItem", "content": [
                {"type": "paragraph", "content": [
                    {"type": "text", "text": "See the "},
                    {"type": "text", "marks": [{"type": "link", "attrs": {"href": "https://a.b/?x=1&y=2"}},
                        {"type": "underline"}], "text": "F.A.Q. <here>"}]}]}]},
            {"type": "heading", "attrs": {"level": 9}, "content": [{"type": "text", "text": "H"}]}
        ]});
        assert_eq!(
            to_html(&doc),
            "<p><strong>NOTES</strong></p><ul><li><p>See the <a href=\"https://a.b/?x=1&amp;y=2\">F.A.Q. &lt;here&gt;</a></p></li></ul><h6>H</h6>"
        );
    }

    #[test]
    fn junk_is_empty() {
        assert_eq!(to_html(&json!(null)), "");
        assert_eq!(to_html(&json!({"type": "text"})), "");
        assert_eq!(
            to_html(&json!({"type": "paragraph", "content": 3})),
            "<p></p>"
        );
    }
}
