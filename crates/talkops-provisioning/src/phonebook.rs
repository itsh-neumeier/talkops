//! Yealink XML remote phonebook (`YealinkIPPhoneDirectory`).

/// One phonebook entry with one or more numbers.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    /// (label, dialable number)
    pub numbers: Vec<(String, String)>,
}

fn esc(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .map(|c| match c {
            '&' => "&amp;".to_owned(),
            '<' => "&lt;".to_owned(),
            '>' => "&gt;".to_owned(),
            '"' => "&quot;".to_owned(),
            '\'' => "&apos;".to_owned(),
            c => c.to_string(),
        })
        .collect()
}

/// Filters entries by a search term (name or number contains it). Yealink
/// sends `#SEARCH` literally when the phone does not substitute it.
pub fn search(entries: Vec<Entry>, term: Option<&str>) -> Vec<Entry> {
    let term = term
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "#SEARCH")
        .map(str::to_lowercase);
    match term {
        None => entries,
        Some(t) => entries
            .into_iter()
            .filter(|e| {
                e.name.to_lowercase().contains(&t) || e.numbers.iter().any(|(_, n)| n.contains(&t))
            })
            .collect(),
    }
}

pub fn render(title: &str, entries: &[Entry]) -> String {
    let mut out =
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<YealinkIPPhoneDirectory>\n");
    out.push_str(&format!("  <Title>{}</Title>\n", esc(title)));
    for e in entries.iter().filter(|e| !e.numbers.is_empty()) {
        out.push_str("  <DirectoryEntry>\n");
        out.push_str(&format!("    <Name>{}</Name>\n", esc(&e.name)));
        for (label, number) in &e.numbers {
            out.push_str(&format!(
                "    <Telephone label=\"{}\">{}</Telephone>\n",
                esc(label),
                esc(number)
            ));
        }
        out.push_str("  </DirectoryEntry>\n");
    }
    out.push_str("</YealinkIPPhoneDirectory>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_and_searches() {
        let entries = vec![
            Entry {
                name: "Müller & Söhne".into(),
                numbers: vec![
                    ("Work".into(), "089123".into()),
                    ("Mobile".into(), "0171999".into()),
                ],
            },
            Entry {
                name: "Lab".into(),
                numbers: vec![("Ext".into(), "21".into())],
            },
            Entry {
                name: "Empty".into(),
                numbers: vec![],
            },
        ];
        let xml = render("Intern", &entries);
        assert!(xml.contains("<Name>Müller &amp; Söhne</Name>"));
        assert!(xml.contains("<Telephone label=\"Mobile\">0171999</Telephone>"));
        assert!(!xml.contains("Empty"));
        assert_eq!(search(entries.clone(), Some("müll")).len(), 1);
        assert_eq!(search(entries.clone(), Some("21")).len(), 1);
        assert_eq!(search(entries.clone(), Some("#SEARCH")).len(), 3);
        assert_eq!(search(entries, None).len(), 3);
    }
}
