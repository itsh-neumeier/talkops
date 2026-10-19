//! Mail rendering (ADR 0034).
//!
//! Each mail has a type ([MailKind]) with whitelisted placeholders and a
//! default text per language. Admins can change the subject and the body.
//! The body uses a small Markdown subset (paragraphs, `**bold**`, `*italic*`,
//! `[links](https://…)`, `-` and `1.` lists) and MiniJinja expressions
//! (`{{ caller.name }}`, `{% if transcript %}…{% endif %}`).
//!
//! Rendering order: the Markdown subset becomes an HTML template (literal
//! text is HTML-escaped, template tags stay unchanged), then MiniJinja fills
//! in the values with HTML auto-escaping. A value can thus never add markup.
//! The plain-text part comes from the same source without markup.
//!
//! The result goes into a fixed, table-based layout with the branding
//! (logo, accent color, header and footer text). The line
//! "Powered by TalkOps vX.Y" and the instance information are part of the
//! layout, not of an editable text.

use std::collections::{BTreeMap, HashSet};

use minijinja::{AutoEscape, Environment, Value};
use serde::Serialize;

/// Version of this TalkOps build (workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Content-ID of the inline logo image.
pub const LOGO_CID: &str = "logo@talkops";

/// Largest accepted subject template.
pub const MAX_SUBJECT: usize = 300;
/// Largest accepted body template.
pub const MAX_BODY: usize = 20_000;

/// Languages with own texts. Index 0 = German, 1 = English.
pub const LANGUAGES: [&str; 2] = ["de", "en"];

/// A placeholder that a template may use, with sample values (de, en) for
/// the preview.
#[derive(Debug, Clone, Copy, Serialize, utoipa::ToSchema)]
pub struct Placeholder {
    /// Name in the template, e.g. `caller.name`.
    pub name: &'static str,
    #[serde(skip)]
    pub sample: [&'static str; 2],
}

const fn ph(name: &'static str, de: &'static str, en: &'static str) -> Placeholder {
    Placeholder {
        name,
        sample: [de, en],
    }
}

/// Placeholders available in every mail and in the header and footer text.
pub const COMMON: &[Placeholder] = &[
    ph("instance.name", "Büro Musterstadt", "Office Springfield"),
    ph(
        "instance.url",
        "https://pbx.example.com",
        "https://pbx.example.com",
    ),
    ph("instance.hostname", "talkops", "talkops"),
    ph("instance.version", VERSION, VERSION),
    ph("recipient.name", "Anna Muster", "Anna Smith"),
    ph("now", "01.07.2026 12:05", "01.07.2026 12:05"),
];

/// A mail type: its placeholders and default texts (index 0 = de, 1 = en).
#[derive(Debug, Clone, Copy)]
pub struct MailKind {
    pub key: &'static str,
    /// Placeholders in addition to [COMMON].
    pub placeholders: &'static [Placeholder],
    pub subject: [&'static str; 2],
    pub body: [&'static str; 2],
}

impl MailKind {
    /// All placeholders of this type, the common ones first.
    pub fn all_placeholders(&self) -> impl Iterator<Item = &'static Placeholder> {
        COMMON.iter().chain(self.placeholders.iter())
    }
}

/// Voicemail notification.
pub const VOICEMAIL: MailKind = MailKind {
    key: "voicemail",
    placeholders: &[
        ph("extension.number", "20", "20"),
        ph("extension.name", "Büro", "Office"),
        ph("caller.name", "Max Mustermann", "John Doe"),
        ph("caller.number", "030 123456", "030 123456"),
        ph("caller.display", "Max Mustermann (030 123456)", "John Doe (030 123456)"),
        ph("call.time", "01.07.2026 12:05", "01.07.2026 12:05"),
        ph("call.duration", "1:15", "1:15"),
        ph(
            "transcript",
            "Hallo, hier ist Max. Bitte ruf mich zurück.",
            "Hi, this is John. Please call me back.",
        ),
    ],
    subject: [
        "Neue Sprachnachricht von {{ caller.display }}",
        "New voicemail from {{ caller.display }}",
    ],
    body: [
        "Hallo {{ recipient.name }},

für die Nebenstelle **{{ extension.number }}** ist eine neue Sprachnachricht eingegangen.

- **Von:** {{ caller.display }}
- **Zeit:** {{ call.time }}
- **Länge:** {{ call.duration }}

{% if transcript %}
**Transkript (automatisch):**
*{{ transcript }}*
{% endif %}

Abhören in der TalkOps-Weboberfläche oder per Anruf auf \\*97.{% if instance.url %} [TalkOps öffnen]({{ instance.url }}){% endif %}",
        "Hello {{ recipient.name }},

there is a new voicemail for extension **{{ extension.number }}**.

- **From:** {{ caller.display }}
- **Received:** {{ call.time }}
- **Length:** {{ call.duration }}

{% if transcript %}
**Transcript (automatic):**
*{{ transcript }}*
{% endif %}

Listen to it in the TalkOps web interface or by dialing \\*97.{% if instance.url %} [Open TalkOps]({{ instance.url }}){% endif %}",
    ],
};

/// Test mail from the SMTP settings and the template editor.
pub const TEST: MailKind = MailKind {
    key: "test",
    placeholders: &[],
    subject: ["Testnachricht", "Test mail"],
    body: [
        "Hallo,

diese Testnachricht zeigt: Der E-Mail-Versand der TalkOps-Telefonanlage funktioniert.

- **Server:** {{ instance.hostname }}
- **Version:** {{ instance.version }}
- **Gesendet:** {{ now }}",
        "Hello,

this test mail shows that your TalkOps phone system can send e-mail.

- **Server:** {{ instance.hostname }}
- **Version:** {{ instance.version }}
- **Sent:** {{ now }}",
    ],
};

/// Operations notification (ADR 0035): alerts, recoveries and digests.
pub const NOTIFICATION: MailKind = MailKind {
    key: "notification",
    placeholders: &[
        ph("event.title", "Trunk Telekom: Registrierung verloren (FAIL_WAIT)", "Trunk Telekom: registration lost (FAIL_WAIT)"),
        ph("event.category", "Trunks", "Trunks"),
        ph("event.severity", "Warnung", "Warning"),
        ph("event.status", "Problem", "Problem"),
        ph("event.time", "01.07.2026 12:05", "01.07.2026 12:05"),
        ph("event.details", "name: Telekom\nstate: FAIL_WAIT", "name: Telekom\nstate: FAIL_WAIT"),
        ph("event.count", "1", "1"),
        ph("event.recovery", "", ""),
        ph("event.digest", "", ""),
    ],
    subject: [
        "{{ event.severity }}: {{ event.title }}",
        "{{ event.severity }}: {{ event.title }}",
    ],
    body: [
        "Hallo,

{% if event.digest %}in den letzten Minuten gab es **{{ event.count }}** Ereignisse der Art „{{ event.category }}“:

{{ event.digest }}{% elif event.recovery %}das Problem ist behoben:

**{{ event.title }}**{% else %}TalkOps meldet ein Ereignis:

**{{ event.title }}**{% endif %}

- **Bereich:** {{ event.category }}
- **Schweregrad:** {{ event.severity }}
- **Zeit:** {{ event.time }}

{% if event.details %}
**Details:**
{{ event.details }}
{% endif %}

{% if instance.url %}[TalkOps öffnen]({{ instance.url }}){% endif %}",
        "Hello,

{% if event.digest %}there were **{{ event.count }}** events of the type “{{ event.category }}” in the last minutes:

{{ event.digest }}{% elif event.recovery %}the problem has ended:

**{{ event.title }}**{% else %}TalkOps reports an event:

**{{ event.title }}**{% endif %}

- **Area:** {{ event.category }}
- **Severity:** {{ event.severity }}
- **Time:** {{ event.time }}

{% if event.details %}
**Details:**
{{ event.details }}
{% endif %}

{% if instance.url %}[Open TalkOps]({{ instance.url }}){% endif %}",
    ],
};

/// Registry of all mail types. New mail types add their entry here; the
/// editor and the API list them automatically.
pub const FAX_RESULT: MailKind = MailKind {
    key: "fax_sent",
    placeholders: &[
        ph("fax.id", "123", "123"),
        ph("fax.source", "+49123456", "+49123456"),
        ph("fax.destination", "+49654321", "+49654321"),
        ph("fax.pages", "2", "2"),
        ph(
            "fax.reason",
            "Faxübertragung abgeschlossen",
            "Fax transfer completed",
        ),
    ],
    subject: ["Faxbericht {{ fax.id }}", "Fax report {{ fax.id }}"],
    body: [
        "Fax: {{ fax.id }}\n\nVon: {{ fax.source }}\n\nAn: {{ fax.destination }}\n\nSeiten: {{ fax.pages }}\n\nErgebnis: {{ fax.reason }}",
        "Fax: {{ fax.id }}\n\nFrom: {{ fax.source }}\n\nTo: {{ fax.destination }}\n\nPages: {{ fax.pages }}\n\nResult: {{ fax.reason }}",
    ],
};
pub const FAX_RECEIVED: MailKind = MailKind {
    key: "fax_received",
    ..FAX_RESULT
};
pub const FAX_FAILED: MailKind = MailKind {
    key: "fax_failed",
    ..FAX_RESULT
};
pub const FAX_REJECTED: MailKind = MailKind {
    key: "fax_rejected",
    subject: ["Mail-to-Fax abgelehnt", "Mail-to-fax rejected"],
    ..FAX_RESULT
};
pub const KINDS: &[&MailKind] = &[
    &VOICEMAIL,
    &NOTIFICATION,
    &TEST,
    &FAX_RESULT,
    &FAX_RECEIVED,
    &FAX_FAILED,
    &FAX_REJECTED,
];

pub fn kind(key: &str) -> Option<&'static MailKind> {
    KINDS.iter().copied().find(|k| k.key == key)
}

/// 0 for German (and unknown), 1 for English.
pub fn lang_index(lang: &str) -> usize {
    usize::from(lang == "en")
}

/// Values for the placeholders, by placeholder name. Missing values are
/// empty (a false condition in `{% if %}`).
pub type Values = BTreeMap<String, String>;

/// Sample values of a mail type for the preview.
pub fn sample_values(kind: &MailKind, lang: &str) -> Values {
    let i = lang_index(lang);
    kind.all_placeholders()
        .map(|p| (p.name.to_owned(), p.sample[i].to_owned()))
        .collect()
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RenderError {
    #[error("{0}")]
    Template(String),
    #[error("unknown placeholder: {0}")]
    UnknownPlaceholder(String),
    #[error("{0} is too long")]
    TooLong(&'static str),
    #[error("{0} must not be empty")]
    Empty(&'static str),
}

// ----------------------------------------------------------------------------
// Markdown subset
// ----------------------------------------------------------------------------

/// Style of a paragraph. `pre-line` keeps line breaks inside values
/// (transcripts).
const P_STYLE: &str = "margin:0 0 16px 0;white-space:pre-line;";
const LIST_STYLE: &str = "margin:0 0 16px 0;padding:0 0 0 24px;";
const LI_STYLE: &str = "margin:0 0 4px 0;";

fn escape_html(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
}

/// Length of a template tag (`{{ … }}`, `{% … %}`, `{# … #}`) at the start
/// of `s`, or `None`.
fn tag_len(s: &str) -> Option<usize> {
    let close = if s.starts_with("{{") {
        "}}"
    } else if s.starts_with("{%") {
        "%}"
    } else if s.starts_with("{#") {
        "#}"
    } else {
        return None;
    };
    s[2..].find(close).map(|end| end + 4)
}

/// Escapes literal text for HTML and copies template tags unchanged.
fn escape_keep_tags(s: &str, out: &mut String) {
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        if let Some(n) = tag_len(rest) {
            out.push_str(&rest[..n]);
            i += n;
        } else {
            let c = rest.chars().next().expect("non-empty");
            escape_html(c.encode_utf8(&mut [0; 4]), out);
            i += c.len_utf8();
        }
    }
}

/// Link targets that are allowed in a template: http(s) and mailto URLs,
/// or a URL that starts with the `instance.url` placeholder. Other
/// placeholders (for example a caller name from the SIP network) cannot
/// set a link target.
fn safe_url(url: &str) -> bool {
    let u = url.trim_start().to_ascii_lowercase();
    if let Some(rest) = u.strip_prefix("{{") {
        return rest
            .trim_start()
            .strip_prefix("instance.url")
            .is_some_and(|r| r.trim_start().starts_with("}}"));
    }
    (u.starts_with("https://") || u.starts_with("http://") || u.starts_with("mailto:"))
        && !u.contains("{{")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Out {
    Html,
    Text,
}

/// Finds the closing delimiter `delim` after position 0 of `s`, skipping
/// template tags.
fn find_closing(s: &str, delim: &str) -> Option<usize> {
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        if let Some(n) = tag_len(rest) {
            i += n;
            continue;
        }
        if rest.starts_with(delim) {
            return Some(i);
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }
    None
}

/// Converts inline Markdown (bold, italic, links, escapes) of one line.
fn inline(s: &str, mode: Out, link_color: &str, out: &mut String) {
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        if let Some(n) = tag_len(rest) {
            out.push_str(&rest[..n]);
            i += n;
            continue;
        }
        if let Some(c) = rest
            .strip_prefix('\\')
            .and_then(|r| r.chars().next())
            .filter(|c| "*_[]()\\".contains(*c))
        {
            push_literal(c, mode, out);
            i += 1 + c.len_utf8();
            continue;
        }
        if let Some(inner) = rest.strip_prefix("**")
            && let Some(end) = find_closing(inner, "**").filter(|&e| e > 0)
        {
            let (open, close) = if mode == Out::Html {
                ("<strong>", "</strong>")
            } else {
                ("", "")
            };
            out.push_str(open);
            inline(&inner[..end], mode, link_color, out);
            out.push_str(close);
            i += 2 + end + 2;
            continue;
        }
        if let Some(inner) = rest.strip_prefix('*')
            && !inner.starts_with([' ', '*'])
            && let Some(end) = find_closing(inner, "*").filter(|&e| e > 0)
        {
            let (open, close) = if mode == Out::Html {
                ("<em>", "</em>")
            } else {
                ("", "")
            };
            out.push_str(open);
            inline(&inner[..end], mode, link_color, out);
            out.push_str(close);
            i += 1 + end + 1;
            continue;
        }
        if let Some(inner) = rest.strip_prefix('[')
            && let Some(end) = find_closing(inner, "](")
            && let Some(url_len) = find_closing(&inner[end + 2..], ")")
        {
            let text = &inner[..end];
            let url = inner[end + 2..end + 2 + url_len].trim();
            if safe_url(url) && !text.is_empty() {
                if mode == Out::Html {
                    out.push_str("<a href=\"");
                    escape_keep_tags(url, out);
                    out.push_str("\" style=\"color:");
                    out.push_str(link_color);
                    out.push_str(";text-decoration:underline;\">");
                    inline(text, mode, link_color, out);
                    out.push_str("</a>");
                } else {
                    inline(text, mode, link_color, out);
                    if text != url {
                        out.push_str(" (");
                        out.push_str(url);
                        out.push(')');
                    }
                }
                i += 1 + end + 2 + url_len + 1;
                continue;
            }
        }
        let c = rest.chars().next().expect("non-empty");
        push_literal(c, mode, out);
        i += c.len_utf8();
    }
}

fn push_literal(c: char, mode: Out, out: &mut String) {
    if mode == Out::Html {
        escape_html(c.encode_utf8(&mut [0; 4]), out);
    } else {
        out.push(c);
    }
}

/// A line that holds only a block tag (`{% if … %}`, `{% endif %}`).
fn is_control_line(line: &str) -> bool {
    let l = line.trim();
    l.starts_with("{%") && tag_len(l) == Some(l.len())
}

fn list_item(line: &str) -> Option<(bool, &str)> {
    let l = line.trim_start();
    if let Some(rest) = l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")) {
        return Some((false, rest));
    }
    let digits = l.bytes().take_while(u8::is_ascii_digit).count();
    if (1..=3).contains(&digits) {
        if let Some(rest) = l[digits..].strip_prefix(". ") {
            return Some((true, rest));
        }
    }
    None
}

enum Block {
    None,
    Para(Vec<String>),
    List(bool, Vec<String>),
}

/// Converts the Markdown subset to a template for HTML (`Out::Html`) or
/// plain text (`Out::Text`). Template tags stay unchanged.
fn markdown(src: &str, mode: Out, link_color: &str) -> String {
    let mut out = String::new();
    let mut block = Block::None;
    let flush = |block: &mut Block, out: &mut String| match std::mem::replace(block, Block::None) {
        Block::None => {}
        Block::Para(lines) => {
            if mode == Out::Html {
                out.push_str("<p style=\"");
                out.push_str(P_STYLE);
                out.push_str("\">");
                for (n, line) in lines.iter().enumerate() {
                    if n > 0 {
                        out.push_str("<br>");
                    }
                    inline(line, mode, link_color, out);
                }
                out.push_str("</p>\n");
            } else {
                for line in &lines {
                    inline(line, mode, link_color, out);
                    out.push('\n');
                }
                out.push('\n');
            }
        }
        Block::List(ordered, items) => {
            if mode == Out::Html {
                let tag = if ordered { "ol" } else { "ul" };
                out.push_str(&format!("<{tag} style=\"{LIST_STYLE}\">"));
                for item in &items {
                    out.push_str("<li style=\"");
                    out.push_str(LI_STYLE);
                    out.push_str("\">");
                    inline(item, mode, link_color, out);
                    out.push_str("</li>");
                }
                out.push_str(&format!("</{tag}>\n"));
            } else {
                for (n, item) in items.iter().enumerate() {
                    if ordered {
                        out.push_str(&format!("{}. ", n + 1));
                    } else {
                        out.push_str("- ");
                    }
                    inline(item, mode, link_color, out);
                    out.push('\n');
                }
                out.push('\n');
            }
        }
    };
    for raw in src.replace("\r\n", "\n").split('\n') {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            flush(&mut block, &mut out);
        } else if is_control_line(line) {
            flush(&mut block, &mut out);
            out.push_str(line.trim());
        } else if let Some((ordered, item)) = list_item(line) {
            match &mut block {
                Block::List(o, items) if *o == ordered => items.push(item.to_owned()),
                _ => {
                    flush(&mut block, &mut out);
                    block = Block::List(ordered, vec![item.to_owned()]);
                }
            }
        } else {
            match &mut block {
                Block::Para(lines) => lines.push(line.trim().to_owned()),
                _ => {
                    flush(&mut block, &mut out);
                    block = Block::Para(vec![line.trim().to_owned()]);
                }
            }
        }
    }
    flush(&mut block, &mut out);
    out
}

// ----------------------------------------------------------------------------
// Template engine (sandbox)
// ----------------------------------------------------------------------------

/// A MiniJinja environment for user templates: no loader (no `include`,
/// `import` or `extends`), HTML auto-escaping for `*.html`, and a fuel limit
/// against endless loops. Globals such as `range` are rejected by the
/// placeholder check because they are undeclared names.
fn sandbox() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|name| {
        if name.ends_with(".html") {
            AutoEscape::Html
        } else {
            AutoEscape::None
        }
    });
    env.set_fuel(Some(50_000));
    env.set_recursion_limit(20);
    env
}

/// Nests flat values (`caller.name`) into objects for the template.
fn context(values: &Values) -> Value {
    let mut root = serde_json::Map::new();
    for (name, value) in values {
        let mut parts: Vec<&str> = name.split('.').collect();
        let last = parts.pop().unwrap_or_default();
        let mut map = &mut root;
        for part in parts {
            let entry = map
                .entry(part.to_owned())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if !entry.is_object() {
                *entry = serde_json::Value::Object(serde_json::Map::new());
            }
            map = entry.as_object_mut().expect("object");
        }
        map.insert(last.to_owned(), serde_json::Value::String(value.to_owned()));
    }
    Value::from_serialize(serde_json::Value::Object(root))
}

fn template_error(err: minijinja::Error) -> RenderError {
    let mut msg = err.detail().unwrap_or("syntax error").to_owned();
    if let Some(line) = err.line() {
        msg = format!("line {line}: {msg}");
    }
    RenderError::Template(msg)
}

/// Compiles `source` and checks that it uses only `allowed` placeholders.
fn check(
    env: &mut Environment<'static>,
    name: &'static str,
    source: String,
    allowed: &HashSet<&str>,
) -> Result<(), RenderError> {
    env.add_template_owned(name, source)
        .map_err(template_error)?;
    let tmpl = env.get_template(name).map_err(template_error)?;
    let mut unknown: Vec<String> = tmpl
        .undeclared_variables(true)
        .into_iter()
        .filter(|v| !allowed.contains(v.as_str()))
        .collect();
    unknown.sort();
    match unknown.into_iter().next() {
        Some(v) => Err(RenderError::UnknownPlaceholder(v)),
        None => Ok(()),
    }
}

fn render_one(env: &Environment<'static>, name: &str, ctx: &Value) -> Result<String, RenderError> {
    env.get_template(name)
        .and_then(|t| t.render(ctx))
        .map_err(template_error)
}

/// Compiled subject and body of one mail (text and HTML versions).
struct Compiled {
    env: Environment<'static>,
}

impl Compiled {
    fn new(
        subject: &str,
        body: &str,
        allowed: &HashSet<&str>,
        link_color: &str,
    ) -> Result<Self, RenderError> {
        let mut env = sandbox();
        check(&mut env, "subject.txt", subject.to_owned(), allowed)?;
        check(
            &mut env,
            "body.html",
            markdown(body, Out::Html, link_color),
            allowed,
        )?;
        check(
            &mut env,
            "body.txt",
            markdown(body, Out::Text, link_color),
            allowed,
        )?;
        Ok(Self { env })
    }
}

fn allowed_names(kind: &MailKind) -> HashSet<&'static str> {
    kind.all_placeholders().map(|p| p.name).collect()
}

/// Checks a subject and body for a mail type: length, syntax, only known
/// placeholders, and a test render with the sample values.
pub fn validate(kind: &MailKind, subject: &str, body: &str) -> Result<(), RenderError> {
    if subject.trim().is_empty() {
        return Err(RenderError::Empty("subject"));
    }
    if body.trim().is_empty() {
        return Err(RenderError::Empty("body"));
    }
    if subject.chars().count() > MAX_SUBJECT {
        return Err(RenderError::TooLong("subject"));
    }
    if body.chars().count() > MAX_BODY {
        return Err(RenderError::TooLong("body"));
    }
    let compiled = Compiled::new(subject, body, &allowed_names(kind), DEFAULT_ACCENT)?;
    let ctx = context(&sample_values(kind, "en"));
    for name in ["subject.txt", "body.html", "body.txt"] {
        render_one(&compiled.env, name, &ctx)?;
    }
    Ok(())
}

/// Checks a header or footer text (common placeholders only).
pub fn validate_fragment(field: &'static str, text: &str, max: usize) -> Result<(), RenderError> {
    if text.chars().count() > max {
        return Err(RenderError::TooLong(field));
    }
    let allowed: HashSet<&str> = COMMON.iter().map(|p| p.name).collect();
    let mut env = sandbox();
    check(
        &mut env,
        "f.html",
        markdown(text, Out::Html, DEFAULT_ACCENT),
        &allowed,
    )?;
    let values: Values = COMMON
        .iter()
        .map(|p| (p.name.to_owned(), p.sample[0].to_owned()))
        .collect();
    render_one(&env, "f.html", &context(&values)).map(|_| ())
}

// ----------------------------------------------------------------------------
// Layout
// ----------------------------------------------------------------------------

/// Default accent color (TalkOps teal).
pub const DEFAULT_ACCENT: &str = "#0d9488";

/// Information about this installation, shown in every mail.
#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct InstanceInfo {
    /// Instance name from the settings; may be empty.
    pub name: String,
    /// Public URL of the web interface; may be empty.
    pub url: String,
    pub hostname: String,
    pub version: String,
}

impl InstanceInfo {
    pub fn new(name: &str, url: &str) -> Self {
        Self {
            name: name.trim().to_owned(),
            url: url.trim().trim_end_matches('/').to_owned(),
            hostname: hostname(),
            version: VERSION.to_owned(),
        }
    }
}

/// Host name of this server (container), or `talkops`.
pub fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty() && h.len() <= 253)
        .unwrap_or_else(|| "talkops".to_owned())
}

/// Logo size in the mail (the image itself is not scaled).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogoSize {
    pub width: u32,
    pub height: u32,
}

impl LogoSize {
    /// Display size: at most 48 px high and 240 px wide, aspect kept.
    pub fn display(self) -> (u32, u32) {
        let (w, h) = (self.width.max(1), self.height.max(1));
        let mut dh = h.min(48);
        let mut dw = (w * dh).div_ceil(h);
        if dw > 240 {
            dw = 240;
            dh = (h * 240).div_ceil(w).max(1);
        }
        (dw, dh)
    }
}

/// Branding that the layout uses.
#[derive(Debug, Clone, Default)]
pub struct Design {
    pub sender_name: String,
    /// `#rrggbb`.
    pub accent_color: String,
    pub header_text: String,
    pub footer_text: String,
    /// Size of the uploaded logo; `None` without logo.
    pub logo: Option<LogoSize>,
    /// Image source for the logo: `cid:…` in mails, a `data:` URL in the
    /// preview.
    pub logo_src: String,
    pub instance: InstanceInfo,
}

/// A rendered mail.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct RenderedMail {
    pub subject: String,
    pub html: String,
    pub text: String,
}

fn parse_hex(color: &str) -> Option<(u8, u8, u8)> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some((p(0)?, p(2)?, p(4)?))
}

/// Accepts `#rgb`/`#rrggbb` (any case) and returns `#rrggbb` lower case.
pub fn normalize_color(color: &str) -> Option<String> {
    let c = color.trim().to_ascii_lowercase();
    let c = match c.len() {
        4 if c.starts_with('#') => {
            let mut s = String::from("#");
            for ch in c[1..].chars() {
                s.push(ch);
                s.push(ch);
            }
            s
        }
        _ => c,
    };
    parse_hex(&c).map(|_| c)
}

fn luminance(color: &str) -> f64 {
    let (r, g, b) = parse_hex(color).unwrap_or((13, 148, 136));
    let ch = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)
}

/// Link color: the accent color if it is readable on white (contrast at
/// least 3:1), else dark teal.
pub fn link_color(accent: &str) -> String {
    let contrast = 1.05 / (luminance(accent) + 0.05);
    if parse_hex(accent).is_some() && contrast >= 3.0 {
        accent.to_owned()
    } else {
        "#0f766e".to_owned()
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    escape_html(s, &mut out);
    out
}

/// Replaces links whose target is not http(s) or mailto after rendering
/// (a value can only come from validated settings, but check anyway).
fn sanitize_links(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find("<a href=\"") {
        let (head, tail) = rest.split_at(pos + 9);
        out.push_str(head);
        let end = tail.find('"').unwrap_or(tail.len());
        let url = &tail[..end];
        let lower = url.trim_start().to_ascii_lowercase();
        if lower.starts_with("https://")
            || lower.starts_with("http://")
            || lower.starts_with("mailto:")
        {
            out.push_str(url);
        } else {
            out.push('#');
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

fn remove_empty_paragraphs(html: &str) -> String {
    html.replace(&format!("<p style=\"{P_STYLE}\"></p>"), "")
}

/// Collapses runs of more than one empty line (from removed `{% if %}`
/// blocks) in the text part.
fn tidy_text(text: &str) -> String {
    let mut out = String::new();
    let mut blank = 0;
    for line in text.lines() {
        if line.trim().is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
            out.push('\n');
        } else {
            blank = 0;
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    out.trim().to_owned()
}

fn fragment(text: &str, mode: Out, link: &str, ctx: &Value) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    let mut env = sandbox();
    let name = if mode == Out::Html { "f.html" } else { "f.txt" };
    let rendered = env
        .add_template_owned(name, markdown(text, mode, link))
        .and_then(|()| env.get_template(name))
        .and_then(|t| t.render(ctx));
    match rendered {
        Ok(s) => s,
        // Saved texts are validated; an old invalid text is left out.
        Err(err) => {
            tracing::warn!(error = %err, "mail header/footer text left out");
            String::new()
        }
    }
}

/// Renders a mail: `subject`/`body` templates of `kind` with `values`, in the
/// layout of `design`. `lang` sets the language of the fixed layout text.
pub fn render(
    design: &Design,
    kind: &MailKind,
    lang: &str,
    subject: &str,
    body: &str,
    values: &Values,
) -> Result<RenderedMail, RenderError> {
    let accent = normalize_color(&design.accent_color).unwrap_or_else(|| DEFAULT_ACCENT.into());
    let link = link_color(&accent);
    let compiled = Compiled::new(subject, body, &allowed_names(kind), &link)?;

    let mut all = values.clone();
    let inst = &design.instance;
    for (k, v) in [
        ("instance.name", &inst.name),
        ("instance.url", &inst.url),
        ("instance.hostname", &inst.hostname),
        ("instance.version", &inst.version),
    ] {
        all.insert(k.to_owned(), v.clone());
    }
    for p in kind.all_placeholders() {
        all.entry(p.name.to_owned()).or_default();
    }
    let ctx = context(&all);

    let mut subject = render_one(&compiled.env, "subject.txt", &ctx)?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if !inst.name.is_empty() {
        let prefix = format!("[{}]", inst.name);
        if !subject.starts_with(&prefix) {
            subject = format!("{prefix} {subject}");
        }
    }
    let body_html = remove_empty_paragraphs(&render_one(&compiled.env, "body.html", &ctx)?);
    let body_text = render_one(&compiled.env, "body.txt", &ctx)?;
    let header_html = fragment(&design.header_text, Out::Html, &link, &ctx);
    let header_text = fragment(&design.header_text, Out::Text, &link, &ctx);
    let footer_html = fragment(&design.footer_text, Out::Html, &link, &ctx);
    let footer_text = fragment(&design.footer_text, Out::Text, &link, &ctx);

    let html = sanitize_links(&layout_html(
        design,
        lang,
        &accent,
        &link,
        &subject,
        &header_html,
        &body_html,
        &footer_html,
    ));
    let text = layout_text(design, &header_text, &body_text, &footer_text);
    Ok(RenderedMail {
        subject,
        html,
        text,
    })
}

/// The fixed instance line of the footer, e.g. `Office · talkops · https://…`.
fn instance_line(inst: &InstanceInfo) -> Vec<String> {
    let mut parts = Vec::new();
    if !inst.name.is_empty() {
        parts.push(inst.name.clone());
    }
    if !inst.hostname.is_empty() {
        parts.push(inst.hostname.clone());
    }
    if !inst.url.is_empty() {
        parts.push(inst.url.clone());
    }
    parts
}

fn powered_by(inst: &InstanceInfo) -> String {
    format!("Powered by TalkOps v{}", inst.version)
}

fn layout_text(design: &Design, header: &str, body: &str, footer: &str) -> String {
    let mut out = String::new();
    let header = tidy_text(header);
    if !header.is_empty() {
        out.push_str(&header);
        out.push_str("\n\n");
    }
    out.push_str(&tidy_text(body));
    out.push_str("\n\n-- \n");
    let footer = tidy_text(footer);
    if !footer.is_empty() {
        out.push_str(&footer);
        out.push('\n');
    }
    let line = instance_line(&design.instance);
    if !line.is_empty() {
        out.push_str(&line.join(" · "));
        out.push('\n');
    }
    out.push_str(&powered_by(&design.instance));
    out.push('\n');
    out
}

#[allow(clippy::too_many_arguments)]
fn layout_html(
    design: &Design,
    lang: &str,
    accent: &str,
    link: &str,
    subject: &str,
    header: &str,
    body: &str,
    footer: &str,
) -> String {
    let inst = &design.instance;
    let brand = if inst.name.is_empty() {
        "TalkOps".to_owned()
    } else {
        inst.name.clone()
    };
    let logo = match design.logo {
        Some(size) if !design.logo_src.is_empty() => {
            let (w, h) = size.display();
            format!(
                "<img src=\"{src}\" width=\"{w}\" height=\"{h}\" alt=\"{alt}\" \
                 style=\"display:block;border:0;outline:none;text-decoration:none;width:{w}px;height:{h}px;\">",
                src = esc(&design.logo_src),
                alt = esc(&brand),
            )
        }
        _ => format!(
            "<span class=\"text\" style=\"font-size:18px;font-weight:700;color:#0f172a;\">{}</span>",
            esc(&brand)
        ),
    };
    let header_row = if header.is_empty() {
        String::new()
    } else {
        format!(
            "<tr><td class=\"text px\" style=\"padding:0 32px 8px 32px;font-size:14px;line-height:20px;color:#475569;\">{header}</td></tr>"
        )
    };
    let footer_row = if footer.is_empty() {
        String::new()
    } else {
        format!(
            "<tr><td class=\"muted\" style=\"padding:0 0 8px 0;font-size:12px;line-height:18px;color:#64748b;\">{footer}</td></tr>"
        )
    };
    let mut line = Vec::new();
    if !inst.name.is_empty() {
        line.push(esc(&inst.name));
    }
    if !inst.hostname.is_empty() {
        line.push(esc(&inst.hostname));
    }
    if !inst.url.is_empty() {
        line.push(format!(
            "<a href=\"{u}\" style=\"color:{link};text-decoration:underline;\">{u}</a>",
            u = esc(&inst.url)
        ));
    }
    let instance_row = if line.is_empty() {
        String::new()
    } else {
        format!(
            "<tr><td class=\"muted\" style=\"padding:0 0 4px 0;font-size:12px;line-height:18px;color:#64748b;\">{}</td></tr>",
            line.join(" &middot; ")
        )
    };
    let font = "-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif";
    let lang = if lang == "en" { "en" } else { "de" };
    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}" xmlns="http://www.w3.org/1999/xhtml">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="x-apple-disable-message-reformatting">
<meta name="color-scheme" content="light dark">
<meta name="supported-color-schemes" content="light dark">
<title>{title}</title>
<style>
body{{margin:0;padding:0;width:100%;-webkit-text-size-adjust:100%;-ms-text-size-adjust:100%;}}
table{{border-collapse:collapse;}}
img{{border:0;outline:none;text-decoration:none;}}
a{{color:{link};}}
@media screen and (max-width:620px){{.container{{width:100%!important;}}.px{{padding-left:20px!important;padding-right:20px!important;}}}}
@media (prefers-color-scheme:dark){{.bg{{background:#0f172a!important;}}.card{{background:#1e293b!important;}}.text{{color:#e2e8f0!important;}}.muted{{color:#94a3b8!important;}}}}
</style>
</head>
<body class="bg" style="margin:0;padding:0;background:#f1f5f9;">
<div style="display:none;max-height:0;overflow:hidden;mso-hide:all;">{title}</div>
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" class="bg" style="background:#f1f5f9;">
<tr><td align="center" style="padding:24px 12px;">
<table role="presentation" class="container" width="600" cellpadding="0" cellspacing="0" border="0" style="width:600px;max-width:600px;">
<tr><td class="card" style="background:#ffffff;border-top:4px solid {accent};border-radius:12px;font-family:{font};">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0">
<tr><td class="px" style="padding:24px 32px 16px 32px;">{logo}</td></tr>
{header_row}
<tr><td class="text px" style="padding:8px 32px 16px 32px;font-size:15px;line-height:23px;color:#1e293b;">
{body}</td></tr>
</table>
</td></tr>
<tr><td class="px" style="padding:16px 32px 0 32px;font-family:{font};">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0">
{footer_row}
{instance_row}
<tr><td class="muted" style="padding:0;font-size:11px;line-height:16px;color:#94a3b8;">{powered}</td></tr>
</table>
</td></tr>
</table>
</td></tr>
</table>
</body>
</html>
"#,
        title = esc(subject),
        powered = esc(&powered_by(inst)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn design() -> Design {
        Design {
            accent_color: "#0d9488".into(),
            instance: InstanceInfo {
                name: "Office".into(),
                url: "https://pbx.example.com".into(),
                hostname: "talkops-1".into(),
                version: "1.2.3".into(),
            },
            ..Design::default()
        }
    }

    #[test]
    fn markdown_subset() {
        let html = markdown(
            "Hello **bold** and *it* [x](https://a.example/?q=1&b=2)\nnext\n\n- one\n- two\n\n1. a\n2. b",
            Out::Html,
            "#123456",
        );
        assert!(html.contains("<strong>bold</strong>"), "{html}");
        assert!(html.contains("<em>it</em>"));
        assert!(html.contains("href=\"https://a.example/?q=1&amp;b=2\""));
        assert!(html.contains("color:#123456"));
        assert!(html.contains("<br>next"));
        assert!(html.contains("<ul") && html.contains("<li style=\"margin:0 0 4px 0;\">one</li>"));
        assert!(html.contains("<ol"));
        let text = markdown("**b** [x](https://a.example)\n\n- one", Out::Text, "");
        assert_eq!(text, "b x (https://a.example)\n\n- one\n\n");
    }

    #[test]
    fn markdown_escapes_literal_html_and_unsafe_links() {
        let html = markdown(
            "<script>alert(1)</script> [x](javascript:alert(1)) \\*97 *97",
            Out::Html,
            "#000000",
        );
        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("href=\"javascript"));
        assert!(html.contains("*97 *97"));
        // Only the instance URL may set a link target.
        let html = markdown(
            "[a]({{ caller.name }}) [b]({{instance.url}}/x)",
            Out::Html,
            "#000",
        );
        assert!(!html.contains("href=\"{{ caller.name"), "{html}");
        assert!(html.contains("href=\"{{instance.url}}/x\""), "{html}");
    }

    #[test]
    fn values_are_escaped() {
        let mut values = sample_values(&VOICEMAIL, "de");
        values.insert(
            "caller.name".into(),
            "<b>Evil</b> **x** [y](javascript:z)".into(),
        );
        values.insert("caller.display".into(), "<img src=x onerror=1>".into());
        let mail = render(
            &design(),
            &VOICEMAIL,
            "de",
            "Von {{ caller.name }}",
            "Name: {{ caller.name }} / {{ caller.display }}",
            &values,
        )
        .unwrap();
        assert!(
            mail.html.contains("&lt;b&gt;Evil&lt;/b&gt; **x**"),
            "{}",
            mail.html
        );
        assert!(!mail.html.contains("<img src=x"));
        assert!(!mail.html.contains("<strong>x"));
        // Plain text and subject are not HTML-escaped.
        assert!(mail.text.contains("<b>Evil</b>"));
        assert_eq!(
            mail.subject,
            "[Office] Von <b>Evil</b> **x** [y](javascript:z)"
        );
    }

    #[test]
    fn unknown_placeholders_are_rejected() {
        assert_eq!(
            validate(&VOICEMAIL, "x", "{{ secret.key }}"),
            Err(RenderError::UnknownPlaceholder("secret.key".into()))
        );
        assert_eq!(
            validate(&TEST, "{{ caller.name }}", "x"),
            Err(RenderError::UnknownPlaceholder("caller.name".into()))
        );
        assert!(matches!(
            validate(&TEST, "x", "{% for i in range(100000000) %}x{% endfor %}"),
            Err(RenderError::UnknownPlaceholder(_))
        ));
        assert!(matches!(
            validate(&TEST, "x", "{% include 'a' %}"),
            Err(RenderError::Template(_))
        ));
        assert!(matches!(
            validate(&TEST, "x", "{{ now"),
            Err(RenderError::Template(_))
        ));
        assert_eq!(
            validate(&TEST, " ", "x"),
            Err(RenderError::Empty("subject"))
        );
        assert!(
            validate(
                &VOICEMAIL,
                "{{ caller.name|upper }}",
                "{% if transcript %}{{ transcript }}{% endif %}"
            )
            .is_ok()
        );
    }

    #[test]
    fn defaults_are_valid() {
        for kind in KINDS {
            for i in 0..2 {
                validate(kind, kind.subject[i], kind.body[i])
                    .unwrap_or_else(|e| panic!("{} {i}: {e}", kind.key));
            }
        }
    }

    #[test]
    fn voicemail_default_renders() {
        let mut values = sample_values(&VOICEMAIL, "en");
        let mail = render(
            &design(),
            &VOICEMAIL,
            "en",
            VOICEMAIL.subject[1],
            VOICEMAIL.body[1],
            &values,
        )
        .unwrap();
        assert_eq!(
            mail.subject,
            "[Office] New voicemail from John Doe (030 123456)"
        );
        assert!(mail.html.contains("Transcript (automatic)"));
        assert!(mail.html.contains("href=\"https://pbx.example.com\""));
        assert!(mail.html.contains("Powered by TalkOps v1.2.3"));
        assert!(mail.html.contains("talkops-1"));
        assert!(
            mail.text.contains("- From: John Doe (030 123456)"),
            "{}",
            mail.text
        );
        assert!(
            mail.text
                .contains("by dialing *97. Open TalkOps (https://pbx.example.com)")
        );
        assert!(mail.text.ends_with(
            "Office · talkops-1 · https://pbx.example.com\nPowered by TalkOps v1.2.3\n"
        ));

        values.insert("transcript".into(), String::new());
        let mut d = design();
        d.instance.url.clear();
        d.instance.name.clear();
        let mail = render(
            &d,
            &VOICEMAIL,
            "en",
            VOICEMAIL.subject[1],
            VOICEMAIL.body[1],
            &values,
        )
        .unwrap();
        assert!(!mail.html.contains("Transcript"));
        assert!(!mail.text.contains("Transcript"));
        assert!(!mail.text.contains("\n\n\n"), "{}", mail.text);
        assert!(!mail.subject.starts_with('['));
        assert!(!mail.html.contains("Open TalkOps"));
    }

    #[test]
    fn powered_by_cannot_be_removed() {
        let mut d = design();
        d.footer_text = "Own footer {{ instance.name }}".into();
        d.header_text = "Header **x**".into();
        let mail = render(&d, &TEST, "de", "s", "b", &Values::new()).unwrap();
        assert!(mail.html.contains("Own footer Office"));
        assert!(mail.html.contains("Header <strong>x</strong>"));
        assert!(mail.html.contains("Powered by TalkOps v1.2.3"));
        assert!(
            mail.text
                .contains("Header x\n\nb\n\n-- \nOwn footer Office\n")
        );
        assert!(mail.text.contains("Powered by TalkOps v1.2.3"));
    }

    #[test]
    fn logo_and_colors() {
        let mut d = design();
        d.logo = Some(LogoSize {
            width: 400,
            height: 100,
        });
        d.logo_src = format!("cid:{LOGO_CID}");
        d.accent_color = "#FFEE00".into();
        let mail = render(
            &d,
            &TEST,
            "de",
            "s",
            "[l](https://x.example)",
            &Values::new(),
        )
        .unwrap();
        assert!(
            mail.html
                .contains("src=\"cid:logo@talkops\" width=\"192\" height=\"48\"")
        );
        assert!(mail.html.contains("border-top:4px solid #ffee00"));
        // Yellow is unreadable on white: links use dark teal.
        assert!(mail.html.contains("color:#0f766e"));
        assert_eq!(normalize_color("#ABC").as_deref(), Some("#aabbcc"));
        assert_eq!(normalize_color("red"), None);
        assert_eq!(
            LogoSize {
                width: 1000,
                height: 50
            }
            .display(),
            (240, 12)
        );
        assert_eq!(
            LogoSize {
                width: 20,
                height: 20
            }
            .display(),
            (20, 20)
        );
    }

    #[test]
    fn fragments_are_validated() {
        assert!(validate_fragment("footer", "Hi {{ instance.name }}", 100).is_ok());
        assert!(validate_fragment("footer", "{{ caller.name }}", 100).is_err());
        assert!(validate_fragment("footer", "xxxxx", 3).is_err());
    }

    #[test]
    fn links_are_sanitized_after_rendering() {
        assert_eq!(
            sanitize_links("<a href=\"javascript:x\">a</a><a href=\"https://ok\">b</a>"),
            "<a href=\"#\">a</a><a href=\"https://ok\">b</a>"
        );
    }
}
