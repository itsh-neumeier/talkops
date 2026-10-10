//! CSV import for the phone book.
//!
//! Accepts the TalkOps sample file as well as the usual exports of Outlook
//! (German and English) and Google Contacts: the delimiter (`;`, `,` or tab)
//! is detected, columns are matched by their header name.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::phones::{self, ContactInput};
use crate::tenant::TenantId;

/// Upper bound for one import, keeps a single request bounded.
pub const MAX_ROWS: usize = 5000;
/// Row errors listed in the report; the rest are only counted.
const MAX_LISTED_ERRORS: usize = 100;

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct ImportRequest {
    /// File content (UTF-8).
    pub csv: String,
    /// Section for rows without a `Bereich`/`Section` column value; none = global.
    #[serde(default)]
    pub section_id: Option<Uuid>,
    /// Contacts with the same name in the same section: update them (true)
    /// or leave them alone (false).
    #[serde(default)]
    pub update_existing: bool,
    /// Only check the file and report what would happen.
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct ImportReport {
    pub created: u32,
    pub updated: u32,
    /// Existing contacts that already had exactly these values.
    pub unchanged: u32,
    /// Existing contacts left alone because `update_existing` was off.
    pub skipped: u32,
    /// Sections named in the file that did not exist (and were created).
    pub sections_created: Vec<String>,
    /// Number of rows with errors (not imported).
    pub failed: u32,
    /// The first row errors.
    pub errors: Vec<RowError>,
    /// Recognised columns, e.g. `["name", "phone_mobile"]`.
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct RowError {
    /// Line in the file (1 = header).
    pub line: u32,
    pub message: String,
}

// --- CSV parsing ----------------------------------------------------------------

/// A record with the line it starts on.
type Record = (u32, Vec<String>);

/// Picks the delimiter that occurs most often in the header line.
fn detect_delimiter(text: &str) -> char {
    let header = text.lines().next().unwrap_or_default();
    let count = |d: char| {
        let mut quoted = false;
        header
            .chars()
            .filter(|&c| {
                if c == '"' {
                    quoted = !quoted;
                }
                !quoted && c == d
            })
            .count()
    };
    [';', ',', '\t']
        .into_iter()
        .max_by_key(|&d| (count(d), d == ';'))
        .unwrap_or(';')
}

/// RFC 4180 parser: quoted fields may contain delimiters, line breaks and
/// doubled quotes. Blank lines are dropped.
fn parse_csv(text: &str, delim: char) -> Vec<Record> {
    let mut records = Vec::new();
    let mut field = String::new();
    let mut row = Vec::new();
    let mut quoted = false;
    let mut line = 1u32;
    let mut start = 1u32;
    let mut chars = text.chars().peekable();
    let mut finish = |row: &mut Vec<String>, field: &mut String, start: u32| {
        row.push(std::mem::take(field));
        if row.iter().any(|f| !f.trim().is_empty()) {
            records.push((start, std::mem::take(row)));
        } else {
            row.clear();
        }
    };
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                '\n' => {
                    line += 1;
                    field.push('\n');
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.trim().is_empty() => {
                field.clear();
                quoted = true;
            }
            '\r' => {}
            '\n' => {
                finish(&mut row, &mut field, start);
                line += 1;
                start = line;
            }
            c if c == delim => row.push(std::mem::take(&mut field)),
            _ => field.push(c),
        }
    }
    finish(&mut row, &mut field, start);
    records
}

// --- column mapping -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Column {
    Name,
    First,
    Last,
    Company,
    Work,
    Mobile,
    Other,
    Section,
}

impl Column {
    fn key(self) -> &'static str {
        match self {
            Column::Name => "name",
            Column::First => "first_name",
            Column::Last => "last_name",
            Column::Company => "company",
            Column::Work => "phone_work",
            Column::Mobile => "phone_mobile",
            Column::Other => "phone_other",
            Column::Section => "section",
        }
    }
}

/// Lower case, letters and digits only: "Telefon (geschäftlich)" → "telefongeschäftlich".
fn normalize_header(h: &str) -> String {
    h.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn column_for(header: &str) -> Option<Column> {
    Some(match normalize_header(header).as_str() {
        "name" | "anzeigename" | "displayname" | "fullname" | "vollständigername" | "kontakt"
        | "contact" => Column::Name,
        "vorname" | "firstname" | "givenname" => Column::First,
        "nachname" | "lastname" | "familyname" | "surname" | "familienname" => Column::Last,
        "firma" | "company" | "organisation" | "organization" | "organizationname"
        | "organization1name" | "unternehmen" => Column::Company,
        "geschäftlich"
        | "telefongeschäftlich"
        | "arbeit"
        | "büro"
        | "work"
        | "business"
        | "businessphone"
        | "workphone"
        | "phonework"
        | "office"
        | "telefon"
        | "phone"
        | "rufnummer"
        | "nummer"
        | "number" => Column::Work,
        "mobil" | "mobile" | "mobiltelefon" | "handy" | "mobilephone" | "phonemobile" | "cell"
        | "cellphone" => Column::Mobile,
        "privat" | "telefonprivat" | "home" | "homephone" | "sonstige" | "andere" | "weitere"
        | "other" | "otherphone" | "phoneother" => Column::Other,
        "bereich" | "section" | "telefonbuch" | "phonebook" => Column::Section,
        _ => return None,
    })
}

/// Google Contacts: "Phone 1 - Value" with the kind in "Phone 1 - Label"
/// (older exports: "Phone 1 - Type").
fn google_phone(header: &str) -> Option<(u32, bool)> {
    let h = normalize_header(header);
    let rest = h.strip_prefix("phone")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let n = digits.parse().ok()?;
    match &rest[digits.len()..] {
        "value" => Some((n, true)),
        "label" | "type" => Some((n, false)),
        _ => None,
    }
}

struct Layout {
    columns: Vec<(usize, Column)>,
    /// Google phone slots: (value index, label index).
    google: Vec<(usize, Option<usize>)>,
}

impl Layout {
    fn from_header(header: &[String]) -> CoreResult<Layout> {
        let mut columns: Vec<(usize, Column)> = Vec::new();
        let mut values: HashMap<u32, usize> = HashMap::new();
        let mut labels: HashMap<u32, usize> = HashMap::new();
        for (i, h) in header.iter().enumerate() {
            if let Some(col) = column_for(h) {
                // First column wins (Outlook has "Business Phone 2" etc.).
                if !columns.iter().any(|(_, c)| *c == col) {
                    columns.push((i, col));
                }
            } else if let Some((n, is_value)) = google_phone(h) {
                if is_value {
                    values.insert(n, i);
                } else {
                    labels.insert(n, i);
                }
            }
        }
        let mut slots: Vec<_> = values.into_iter().collect();
        slots.sort();
        let google = slots
            .into_iter()
            .map(|(n, i)| (i, labels.get(&n).copied()))
            .collect::<Vec<_>>();
        let has = |c: Column| columns.iter().any(|(_, x)| *x == c);
        if !has(Column::Name) && !has(Column::First) && !has(Column::Last) && !has(Column::Company)
        {
            return Err(CoreError::Validation(
                "no name column found (expected e.g. Name, Vorname/Nachname)".into(),
            ));
        }
        if !has(Column::Work) && !has(Column::Mobile) && !has(Column::Other) && google.is_empty() {
            return Err(CoreError::Validation(
                "no phone number column found (expected e.g. Geschäftlich, Mobil, Privat)".into(),
            ));
        }
        Ok(Layout { columns, google })
    }

    fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.columns.iter().map(|(_, c)| c.key().into()).collect();
        if !self.google.is_empty() {
            keys.push("google_phones".into());
        }
        keys
    }
}

/// One row turned into a contact plus the section name it names, if any.
#[derive(Debug, Default, PartialEq)]
struct Row {
    contact: RowContact,
    section: String,
}

#[derive(Debug, Default, PartialEq)]
struct RowContact {
    name: String,
    company: String,
    work: String,
    mobile: String,
    other: String,
}

fn read_row(layout: &Layout, fields: &[String]) -> Row {
    let get = |i: usize| {
        fields
            .get(i)
            .map(|s| s.trim().to_owned())
            .unwrap_or_default()
    };
    let mut row = Row::default();
    let (mut first, mut last) = (String::new(), String::new());
    for &(i, col) in &layout.columns {
        let v = get(i);
        let c = &mut row.contact;
        match col {
            Column::Name => c.name = v,
            Column::First => first = v,
            Column::Last => last = v,
            Column::Company => c.company = v,
            Column::Work => c.work = v,
            Column::Mobile => c.mobile = v,
            Column::Other => c.other = v,
            Column::Section => row.section = v,
        }
    }
    for &(vi, li) in &layout.google {
        // Google joins several numbers of one slot with " ::: ".
        let value = get(vi);
        let Some(number) = value.split(":::").map(str::trim).find(|s| !s.is_empty()) else {
            continue;
        };
        let label = li.map(get).unwrap_or_default().to_lowercase();
        let c = &mut row.contact;
        let preferred = if label.contains("mobil") || label.contains("cell") {
            &mut c.mobile
        } else if label.contains("work") || label.contains("arbeit") || label.contains("gesch") {
            &mut c.work
        } else {
            &mut c.other
        };
        if preferred.is_empty() {
            *preferred = number.to_owned();
        } else if let Some(free) = [&mut c.work, &mut c.mobile, &mut c.other]
            .into_iter()
            .find(|s| s.is_empty())
        {
            *free = number.to_owned();
        }
    }
    let c = &mut row.contact;
    if c.name.is_empty() {
        c.name = [first, last]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }
    if c.name.is_empty() && !c.company.is_empty() {
        c.name = std::mem::take(&mut c.company);
    }
    row
}

// --- import ---------------------------------------------------------------------

fn validation_message(e: CoreError) -> CoreResult<String> {
    match e {
        CoreError::Validation(m) => Ok(m),
        e => Err(e),
    }
}

/// Imports (or with `dry_run` only checks) a CSV file. Valid rows are
/// imported, invalid rows are reported; everything runs in one transaction.
pub async fn import(
    pool: &PgPool,
    tenant: TenantId,
    req: &ImportRequest,
) -> CoreResult<ImportReport> {
    let text = req.csv.strip_prefix('\u{feff}').unwrap_or(&req.csv);
    let records = parse_csv(text, detect_delimiter(text));
    let Some(((_, header), rows)) = records.split_first() else {
        return Err(CoreError::Validation("the file is empty".into()));
    };
    if rows.len() > MAX_ROWS {
        return Err(CoreError::Validation(format!(
            "too many rows ({}, at most {MAX_ROWS})",
            rows.len()
        )));
    }
    let layout = Layout::from_header(header)?;
    if let Some(id) = req.section_id {
        phones::get_section(pool, tenant, id)
            .await
            .map_err(|e| match e {
                CoreError::NotFound => CoreError::Validation("unknown phone book section".into()),
                e => e,
            })?;
    }

    let mut report = ImportReport {
        columns: layout.keys(),
        ..ImportReport::default()
    };
    let mut tx = pool.begin().await?;
    let mut sections: HashMap<String, Uuid> = phones::list_sections(&mut *tx, tenant)
        .await?
        .into_iter()
        .map(|s| (s.name.to_lowercase(), s.id))
        .collect();
    fn fail(report: &mut ImportReport, line: u32, message: String) {
        report.failed += 1;
        if report.errors.len() < MAX_LISTED_ERRORS {
            report.errors.push(RowError { line, message });
        }
    }

    for (line, fields) in rows {
        let row = read_row(&layout, fields);
        let section_id = if row.section.is_empty() {
            req.section_id
        } else if let Some(id) = sections.get(&row.section.to_lowercase()) {
            Some(*id)
        } else {
            let input = phones::SectionInput {
                name: row.section.clone(),
            };
            match phones::create_section(&mut *tx, tenant, &input).await {
                Ok(s) => {
                    report.sections_created.push(s.name);
                    sections.insert(row.section.to_lowercase(), s.id);
                    Some(s.id)
                }
                Err(e) => {
                    fail(&mut report, *line, validation_message(e)?);
                    continue;
                }
            }
        };
        let c = row.contact;
        let input = ContactInput {
            name: c.name,
            company: c.company,
            phone_work: c.work,
            phone_mobile: c.mobile,
            phone_other: c.other,
            section_id,
        };
        let numbers = match phones::validate_contact(&input) {
            Ok(n) => n,
            Err(e) => {
                let message = validation_message(e)?;
                let message = if input.name.trim().is_empty() {
                    message
                } else {
                    format!("{}: {message}", input.name.trim())
                };
                fail(&mut report, *line, message);
                continue;
            }
        };
        let existing =
            phones::find_contact(&mut *tx, tenant, input.name.trim(), section_id).await?;
        match existing {
            None => {
                phones::insert_contact(&mut *tx, tenant, &input, &numbers).await?;
                report.created += 1;
            }
            Some(old) => {
                let same = old.company == input.company.trim()
                    && [&old.phone_work, &old.phone_mobile, &old.phone_other]
                        .into_iter()
                        .eq(numbers.iter());
                if same {
                    report.unchanged += 1;
                } else if req.update_existing {
                    phones::write_contact(&mut *tx, tenant, old.id, &input, &numbers).await?;
                    report.updated += 1;
                } else {
                    report.skipped += 1;
                }
            }
        }
    }

    if req.dry_run {
        tx.rollback().await?;
    } else {
        tx.commit().await?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(header: &str) -> Layout {
        let records = parse_csv(header, detect_delimiter(header));
        Layout::from_header(&records[0].1).unwrap()
    }

    #[test]
    fn parses_quotes_line_breaks_and_blank_lines() {
        let text = "Name;Notiz\r\n\"Müller; Hans\";\"zwei\nZeilen \"\"x\"\"\"\r\n\r\n;\nOma;1\n";
        let records = parse_csv(text, detect_delimiter(text));
        assert_eq!(records.len(), 3);
        assert_eq!(
            records[1],
            (2, vec!["Müller; Hans".into(), "zwei\nZeilen \"x\"".into()])
        );
        assert_eq!(records[2], (6, vec!["Oma".into(), "1".into()]));
    }

    #[test]
    fn detects_delimiters() {
        assert_eq!(detect_delimiter("Name;Mobil\n"), ';');
        assert_eq!(detect_delimiter("Name,Mobile\n"), ',');
        assert_eq!(detect_delimiter("Name\tMobile\n"), '\t');
        assert_eq!(detect_delimiter("\"Name, full\";Mobil\n"), ';');
    }

    #[test]
    fn reads_the_sample_layout() {
        let l = layout("Name;Firma;Geschäftlich;Mobil;Privat;Bereich");
        let fields: Vec<String> = ["Max", "ACME", "089 123", "", "", "Büro"]
            .map(String::from)
            .to_vec();
        let row = read_row(&l, &fields);
        assert_eq!(row.contact.name, "Max");
        assert_eq!(row.contact.work, "089 123");
        assert_eq!(row.section, "Büro");
    }

    #[test]
    fn reads_outlook_and_google_exports() {
        let l = layout(
            "Vorname,Nachname,Firma,Telefon geschäftlich,Telefon geschäftlich 2,Mobiltelefon",
        );
        let fields: Vec<String> = ["Erika", "Muster", "", "0301", "0302", "0171"]
            .map(String::from)
            .to_vec();
        let row = read_row(&l, &fields);
        assert_eq!(row.contact.name, "Erika Muster");
        assert_eq!(
            (row.contact.work.as_str(), row.contact.mobile.as_str()),
            ("0301", "0171")
        );

        let l = layout(
            "First Name,Last Name,Organization Name,Phone 1 - Label,Phone 1 - Value,Phone 2 - Label,Phone 2 - Value",
        );
        let fields: Vec<String> = [
            "",
            "",
            "Pizza Roma",
            "Mobile",
            "0151 ::: 0152",
            "Home",
            "089",
        ]
        .map(String::from)
        .to_vec();
        let row = read_row(&l, &fields);
        assert_eq!(row.contact.name, "Pizza Roma");
        assert_eq!(row.contact.company, "");
        assert_eq!(
            (row.contact.mobile.as_str(), row.contact.other.as_str()),
            ("0151", "089")
        );
    }

    #[test]
    fn rejects_files_without_name_or_number_columns() {
        let header = |h: &str| Layout::from_header(&parse_csv(h, ';')[0].1).is_err();
        assert!(header("Foo;Mobil"));
        assert!(header("Name;Foo"));
    }
}

#[cfg(test)]
mod number_tests {
    use crate::phones::{ContactInput, validate_contact};

    #[test]
    fn cleans_common_number_spellings() {
        let input = |n: &str| ContactInput {
            name: "x".into(),
            company: String::new(),
            phone_work: n.into(),
            phone_mobile: String::new(),
            phone_other: String::new(),
            section_id: None,
        };
        let work = |n: &str| validate_contact(&input(n)).map(|[w, ..]| w);
        assert_eq!(work("+49 (0)89 / 123-45.6").unwrap(), "+4989123456");
        assert_eq!(work("(089) 12345").unwrap(), "08912345");
        assert!(work("4.91711E+11").is_err());
    }
}
