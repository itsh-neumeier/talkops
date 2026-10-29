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

/// Entries per Cisco directory page (the phone shows at most 32).
pub const CISCO_PAGE: usize = 32;

fn clip(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect()
}

/// Cisco Enterprise SIP directory page (`CiscoIPPhoneDirectory`). One row per
/// number, at most [`CISCO_PAGE`] rows per page. `url` is the absolute
/// directory URL without query; `query` is the percent-encoded search term
/// for the paging links. `page` starts at 1.
pub fn cisco_directory(
    title: &str,
    entries: &[Entry],
    url: &str,
    query: &str,
    page: usize,
) -> String {
    let rows: Vec<(String, &str)> = entries
        .iter()
        .flat_map(|e| {
            let many = e.numbers.len() > 1;
            e.numbers.iter().map(move |(label, number)| {
                let name = if many {
                    format!("{} ({label})", e.name)
                } else {
                    e.name.clone()
                };
                (name, number.as_str())
            })
        })
        .collect();
    let pages = rows.len().div_ceil(CISCO_PAGE).max(1);
    let page = page.clamp(1, pages);
    let link = |page: usize| {
        let mut link = format!("{url}?page={page}");
        if !query.is_empty() {
            link.push_str(&format!("&search={query}"));
        }
        esc(&link)
    };
    let mut out =
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<CiscoIPPhoneDirectory>\n");
    out.push_str(&format!("  <Title>{}</Title>\n", esc(&clip(title, 32))));
    out.push_str(&format!("  <Prompt>{page}/{pages}</Prompt>\n"));
    for (name, number) in rows.iter().skip((page - 1) * CISCO_PAGE).take(CISCO_PAGE) {
        out.push_str(&format!(
            "  <DirectoryEntry><Name>{}</Name><Telephone>{}</Telephone></DirectoryEntry>\n",
            esc(&clip(name, 32)),
            esc(&clip(number, 32))
        ));
    }
    let mut key = |name: &str, target: String, position: u8| {
        out.push_str(&format!(
            "  <SoftKeyItem><Name>{name}</Name><URL>{target}</URL><Position>{position}</Position></SoftKeyItem>\n"
        ));
    };
    key("Dial", "SoftKey:Dial".into(), 1);
    key("Search", esc(&format!("{url}?search=")), 2);
    if page < pages {
        key("Next", link(page + 1), 3);
    }
    if page > 1 {
        key("Previous", link(page - 1), 4);
    }
    key("Exit", "SoftKey:Exit".into(), 5);
    out.push_str("</CiscoIPPhoneDirectory>\n");
    out
}

/// Search form for the Cisco directory. The phone sends the entered text as
/// `search` to `url`.
pub fn cisco_search(url: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<CiscoIPPhoneInput>\n  <Title>Search</Title>\n  \
         <Prompt>Name or number</Prompt>\n  <URL>{}</URL>\n  <InputItem><DisplayName>Name</DisplayName>\
         <QueryStringParam>search</QueryStringParam><DefaultValue></DefaultValue><InputFlags>A</InputFlags>\
         </InputItem>\n</CiscoIPPhoneInput>\n",
        esc(url)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cisco_directory_pages_and_escapes() {
        let entries: Vec<Entry> = (0..40)
            .map(|i| Entry {
                name: format!("Person {i:02}"),
                numbers: vec![("Work".into(), format!("{i}"))],
            })
            .chain([Entry {
                name: "Müller & <Söhne>".into(),
                numbers: vec![
                    ("Work".into(), "089123".into()),
                    ("Mobile".into(), "0171999".into()),
                ],
            }])
            .collect();
        let url = "http://pbx/provisioning/cisco/directory/x/y";
        let first = cisco_directory("TalkOps", &entries, url, "", 1);
        roxmltree::Document::parse(&first).unwrap();
        assert_eq!(first.matches("<DirectoryEntry>").count(), 32);
        assert!(first.contains("<Prompt>1/2</Prompt>"));
        assert!(first.contains("<URL>http://pbx/provisioning/cisco/directory/x/y?page=2</URL>"));
        assert!(!first.contains("Previous"));
        let second = cisco_directory("TalkOps", &entries, url, "M%C3%BC", 2);
        roxmltree::Document::parse(&second).unwrap();
        assert_eq!(second.matches("<DirectoryEntry>").count(), 10);
        assert!(second.contains("<Name>Müller &amp; &lt;Söhne&gt; (Mobile)</Name>"));
        assert!(second.contains("?page=1&amp;search=M%C3%BC"));
        assert!(!second.contains("Next"));
        // Out-of-range pages show the last page.
        assert!(cisco_directory("T", &entries, url, "", 9).contains("<Prompt>2/2</Prompt>"));
        assert!(cisco_directory("T", &[], url, "", 1).contains("<Prompt>1/1</Prompt>"));
        let form = cisco_search(url);
        roxmltree::Document::parse(&form).unwrap();
        assert!(form.contains("<QueryStringParam>search</QueryStringParam>"));
    }

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
                numbers: vec![("Ext".into(), "**21".into())],
            },
            Entry {
                name: "Empty".into(),
                numbers: vec![],
            },
        ];
        let xml = render("Intern", &entries);
        assert!(xml.contains("<Name>Müller &amp; Söhne</Name>"));
        assert!(xml.contains("<Telephone label=\"Mobile\">0171999</Telephone>"));
        assert!(xml.contains("<Telephone label=\"Ext\">**21</Telephone>"));
        assert!(!xml.contains("Empty"));
        assert_eq!(search(entries.clone(), Some("müll")).len(), 1);
        assert_eq!(search(entries.clone(), Some("21")).len(), 1);
        assert_eq!(search(entries.clone(), Some("**21")).len(), 1);
        assert_eq!(search(entries.clone(), Some("#SEARCH")).len(), 3);
        assert_eq!(search(entries, None).len(), 3);
    }
}
