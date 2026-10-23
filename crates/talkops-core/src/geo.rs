//! Number geolocation for the call origin map (ADR 0044).
//!
//! The data files in `presets/geo/*.tsv` map E.164 prefixes to areas: local
//! network areas (DACH), places (other countries), mobile and service ranges
//! and countries. The data is static and loaded once at startup. A lookup is
//! a longest-prefix match on the digits of an E.164 number.
//!
//! File format (tab-separated, UTF-8, `#` starts a comment line):
//!
//! ```text
//! # source: <text>       metadata comments: title, source, license, updated
//! A <id> <country> <kind> <lat> <lon> <cc rest> <name>
//! P <digits> <id>
//! ```
//!
//! An `A` line defines an area with its display prefix (calling code and
//! the rest, separated by a space). A `P` line maps a prefix to an area. An
//! area can have several prefixes; a `P` line can refer to an area in
//! another file. Each prefix and each area ID must be unique.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

/// Longest number of digits in an E.164 number.
const MAX_DIGITS: usize = 15;
/// Shortest number (digits after `+`) that the lookup accepts.
const MIN_DIGITS: usize = 6;

#[derive(Debug, thiserror::Error)]
pub enum GeoError {
    #[error("cannot read geo data from {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("invalid geo data {file}:{line}: {message}")]
    Invalid {
        file: String,
        line: usize,
        message: String,
    },
}

/// What an area stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum GeoKind {
    /// A local network area or a place.
    Area,
    /// Mobile numbers of a country (no location).
    Mobile,
    /// Service and other non-geographic numbers of a country.
    Service,
    /// Only the country is known.
    Country,
}

impl GeoKind {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "area" => Self::Area,
            "mobile" => Self::Mobile,
            "service" => Self::Service,
            "country" => Self::Country,
            _ => return None,
        })
    }

    /// True if the area has a location below country level.
    pub fn is_located(self) -> bool {
        self == Self::Area
    }
}

/// The area of a number.
#[derive(Debug, Clone, PartialEq, Serialize, utoipa::ToSchema)]
pub struct GeoArea {
    /// Stable identifier, e.g. `DE-951`, `DE-mobile`, `DE`.
    pub id: String,
    /// ISO 3166-1 alpha-2 country code.
    pub country: String,
    /// Display prefix, e.g. `+49 951`.
    pub prefix: String,
    /// Area or place name in the language of the source.
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub kind: GeoKind,
}

/// Origin and license of one data file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct GeoSource {
    pub file: String,
    pub title: String,
    pub source: String,
    pub license: String,
    pub updated: String,
}

/// All areas and prefixes, ready for lookups.
#[derive(Debug, Default)]
pub struct GeoIndex {
    areas: Vec<GeoArea>,
    /// Key: [`key`] of the prefix digits; value: index in `areas`.
    prefixes: HashMap<u64, u32>,
    longest: usize,
    sources: Vec<GeoSource>,
}

/// Packs a digit string (at most 15 digits) and its length into one value.
/// The length keeps `49` and `049` apart.
fn key(value: u64, len: usize) -> u64 {
    (value << 4) | len as u64
}

fn digits_value(digits: &str) -> Option<u64> {
    if digits.is_empty() || digits.len() > MAX_DIGITS {
        return None;
    }
    digits.bytes().try_fold(0u64, |acc, b| {
        b.is_ascii_digit().then(|| acc * 10 + u64::from(b - b'0'))
    })
}

/// Collects the files before the prefixes are resolved to areas.
#[derive(Default)]
struct Builder {
    index: GeoIndex,
    ids: HashMap<String, u32>,
    /// Prefix lines: digits, area ID, file, line.
    pending: Vec<(String, String, String, usize)>,
}

impl Builder {
    fn add_file(&mut self, file: &str, text: &str) -> Result<(), GeoError> {
        let invalid = |line: usize, message: String| GeoError::Invalid {
            file: file.to_owned(),
            line,
            message,
        };
        let mut source = GeoSource {
            file: file.to_owned(),
            ..GeoSource::default()
        };
        for (n, line) in text.lines().enumerate() {
            let n = n + 1;
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            if let Some(comment) = line.strip_prefix('#') {
                if let Some((k, v)) = comment.trim().split_once(':') {
                    let v = v.trim().to_owned();
                    match k.trim() {
                        "title" => source.title = v,
                        "source" => source.source = v,
                        "license" => source.license = v,
                        "updated" => source.updated = v,
                        _ => {}
                    }
                }
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            match fields.as_slice() {
                ["A", id, country, kind, lat, lon, prefix, name] => {
                    let kind = GeoKind::parse(kind)
                        .ok_or_else(|| invalid(n, format!("unknown kind {kind:?}")))?;
                    let lat: f64 = lat
                        .parse()
                        .ok()
                        .filter(|v: &f64| (-90.0..=90.0).contains(v))
                        .ok_or_else(|| invalid(n, format!("invalid latitude {lat:?}")))?;
                    let lon: f64 = lon
                        .parse()
                        .ok()
                        .filter(|v: &f64| (-180.0..=180.0).contains(v))
                        .ok_or_else(|| invalid(n, format!("invalid longitude {lon:?}")))?;
                    if country.len() != 2 || !country.bytes().all(|b| b.is_ascii_uppercase()) {
                        return Err(invalid(n, format!("invalid country {country:?}")));
                    }
                    let shown: String = prefix.chars().filter(|c| *c != ' ').collect();
                    if id.is_empty() || digits_value(&shown).is_none() || name.trim().is_empty() {
                        return Err(invalid(n, "empty ID, name or invalid prefix".into()));
                    }
                    let idx = u32::try_from(self.index.areas.len())
                        .map_err(|_| invalid(n, "too many areas".into()))?;
                    if self.ids.insert((*id).to_owned(), idx).is_some() {
                        return Err(invalid(n, format!("duplicate area {id}")));
                    }
                    self.index.areas.push(GeoArea {
                        id: (*id).to_owned(),
                        country: (*country).to_owned(),
                        prefix: format!("+{}", prefix.trim()),
                        name: name.trim().to_owned(),
                        lat,
                        lon,
                        kind,
                    });
                }
                ["P", digits, id] => {
                    self.pending
                        .push(((*digits).to_owned(), (*id).to_owned(), file.to_owned(), n));
                }
                _ => return Err(invalid(n, "expected an A or P line".into())),
            }
        }
        self.index.sources.push(source);
        Ok(())
    }

    fn finish(mut self) -> Result<GeoIndex, GeoError> {
        for (digits, id, file, line) in std::mem::take(&mut self.pending) {
            let invalid = |message: String| GeoError::Invalid {
                file: file.clone(),
                line,
                message,
            };
            let value = digits_value(&digits)
                .ok_or_else(|| invalid(format!("invalid prefix {digits:?}")))?;
            let idx = *self
                .ids
                .get(&id)
                .ok_or_else(|| invalid(format!("unknown area {id}")))?;
            if self
                .index
                .prefixes
                .insert(key(value, digits.len()), idx)
                .is_some()
            {
                return Err(invalid(format!("duplicate prefix {digits}")));
            }
            self.index.longest = self.index.longest.max(digits.len());
        }
        Ok(self.index)
    }
}

impl GeoIndex {
    /// Loads all `*.tsv` files of a directory (sorted by name).
    pub fn load_dir(dir: &Path) -> Result<Self, GeoError> {
        let io = |source| GeoError::Io {
            path: dir.display().to_string(),
            source,
        };
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .map_err(io)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "tsv"))
            .collect();
        files.sort();
        let mut builder = Builder::default();
        for path in files {
            let text = std::fs::read_to_string(&path).map_err(|source| GeoError::Io {
                path: path.display().to_string(),
                source,
            })?;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            builder.add_file(&name, &text)?;
        }
        builder.finish()
    }

    /// Builds an index from files in memory: (file name, content).
    pub fn from_files<'a>(
        files: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<Self, GeoError> {
        let mut builder = Builder::default();
        for (name, text) in files {
            builder.add_file(name, text)?;
        }
        builder.finish()
    }

    /// The area of an E.164 number (`+4995112345`). Returns `None` for
    /// other formats, too short or too long numbers, and unknown prefixes.
    /// Numbers that are only a prefix (`+49951`) have no area.
    pub fn lookup(&self, e164: &str) -> Option<&GeoArea> {
        let digits = e164.strip_prefix('+')?.as_bytes();
        if !(MIN_DIGITS..=MAX_DIGITS).contains(&digits.len())
            || !digits.iter().all(u8::is_ascii_digit)
        {
            return None;
        }
        let mut values = [0u64; MAX_DIGITS + 1];
        for (i, d) in digits.iter().enumerate() {
            values[i + 1] = values[i] * 10 + u64::from(d - b'0');
        }
        // At least one digit after the prefix.
        let longest = self.longest.min(digits.len() - 1);
        (1..=longest)
            .rev()
            .find_map(|len| self.prefixes.get(&key(values[len], len)))
            .map(|&idx| &self.areas[idx as usize])
    }

    /// An area by its ID.
    pub fn area(&self, id: &str) -> Option<&GeoArea> {
        self.areas.iter().find(|a| a.id == id)
    }

    /// The country area (kind [`GeoKind::Country`]) of an ISO code.
    pub fn country(&self, iso: &str) -> Option<&GeoArea> {
        self.areas
            .iter()
            .find(|a| a.kind == GeoKind::Country && a.country == iso)
    }

    pub fn sources(&self) -> &[GeoSource] {
        &self.sources
    }

    /// Number of areas.
    pub fn len(&self) -> usize {
        self.areas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.areas.is_empty()
    }

    /// Number of prefixes.
    pub fn prefix_count(&self) -> usize {
        self.prefixes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialing::DialPlanSettings;

    const SAMPLE: &str = "# TalkOps call origin data, format 1\n\
        # title: Sample\n\
        # license: test\n\
        A\tDE\tDE\tcountry\t51\t10\t49\tGermany\n\
        P\t49\tDE\n\
        A\tDE-951\tDE\tarea\t49.89\t10.89\t49 951\tBamberg\n\
        P\t49951\tDE-951\n\
        A\tDE-mobile\tDE\tmobile\t51\t10\t49\tGermany\n\
        P\t4917\tDE-mobile\n";

    #[test]
    fn parses_and_matches_the_longest_prefix() {
        let geo = GeoIndex::from_files([("sample.tsv", SAMPLE)]).unwrap();
        assert_eq!(geo.len(), 3);
        assert_eq!(geo.prefix_count(), 3);
        assert_eq!(geo.sources()[0].title, "Sample");
        let a = geo.lookup("+4995112345").unwrap();
        assert_eq!((a.id.as_str(), a.prefix.as_str()), ("DE-951", "+49 951"));
        assert_eq!(geo.lookup("+491711234567").unwrap().kind, GeoKind::Mobile);
        assert_eq!(geo.lookup("+49301234567").unwrap().kind, GeoKind::Country);
        assert!(geo.lookup("+49951").is_none(), "prefix only");
        assert!(geo.lookup("+4312345678").is_none(), "unknown country");
        assert!(geo.lookup("4995112345").is_none(), "not E.164");
        assert!(geo.lookup("+49 951 12345").is_none(), "not compact");
        assert!(geo.lookup("+4912345678901234").is_none(), "too long");
    }

    #[test]
    fn rejects_invalid_files() {
        for (text, message) in [
            ("A\tX\tDE\tplanet\t1\t1\t49\tX\n", "unknown kind"),
            ("A\tX\tDE\tarea\t91\t1\t49\tX\n", "invalid latitude"),
            ("A\tX\tde\tarea\t1\t1\t49\tX\n", "invalid country"),
            (
                "A\tX\tDE\tarea\t1\t1\t49\tX\nA\tX\tDE\tarea\t1\t1\t49\tY\n",
                "duplicate area",
            ),
            (
                "A\tX\tDE\tarea\t1\t1\t49\tX\nP\t49\tX\nP\t49\tX\n",
                "duplicate prefix",
            ),
            ("P\t49\tY\n", "unknown area"),
            ("A\tX\tDE\tarea\t1\t1\t49\tX\nP\t4a\tX\n", "invalid prefix"),
            ("B\t1\n", "expected an A or P line"),
        ] {
            let err = GeoIndex::from_files([("bad.tsv", text)])
                .unwrap_err()
                .to_string();
            assert!(err.contains(message), "{text:?}: {err}");
        }
    }

    #[test]
    fn prefixes_can_refer_to_areas_in_other_files() {
        let geo = GeoIndex::from_files([
            ("a.tsv", "P\t4932\tDE-service\n"),
            ("b.tsv", "A\tDE-service\tDE\tservice\t51\t10\t49\tGermany\n"),
        ])
        .unwrap();
        assert_eq!(geo.lookup("+49321234567").unwrap().kind, GeoKind::Service);
    }

    fn data_dir() -> std::path::PathBuf {
        std::env::var_os("TALKOPS_GEO_DIR")
            .map(Into::into)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets/geo"))
    }

    /// The data files of the repository: every file loads and the
    /// acceptance examples of ADR 0044 resolve.
    #[test]
    fn resolves_numbers_with_the_shipped_data() {
        let started = std::time::Instant::now();
        let geo = GeoIndex::load_dir(&data_dir()).unwrap();
        eprintln!(
            "loaded {} areas, {} prefixes in {:?}",
            geo.len(),
            geo.prefix_count(),
            started.elapsed()
        );
        let at = |n: &str| {
            geo.lookup(n)
                .map(|a| (a.kind, a.country.as_str(), a.name.as_str()))
        };
        use GeoKind::*;
        // German local network areas with 2- to 5-digit area codes.
        assert_eq!(at("+4930123456"), Some((Area, "DE", "Berlin")));
        assert_eq!(at("+4989123456"), Some((Area, "DE", "München")));
        assert_eq!(at("+4995112345"), Some((Area, "DE", "Bamberg")));
        assert_eq!(geo.lookup("+4995112345").unwrap().prefix, "+49 951");
        assert_eq!(at("+49954312345"), Some((Area, "DE", "Hirschaid")));
        assert_eq!(at("+49343712345"), Some((Area, "DE", "Grimma")));
        assert_eq!(at("+493320312345"), Some((Area, "DE", "Kleinmachnow")));
        assert_eq!(geo.lookup("+493320312345").unwrap().prefix, "+49 33203");
        // Austria and Switzerland.
        assert_eq!(at("+4311234567"), Some((Area, "AT", "Wien")));
        assert_eq!(at("+43662123456"), Some((Area, "AT", "Salzburg")));
        assert_eq!(at("+41441234567"), Some((Area, "CH", "Zürich")));
        assert_eq!(at("+41431234567"), Some((Area, "CH", "Zürich")));
        assert_eq!(at("+41311234567"), Some((Area, "CH", "Bern")));
        // German mobile and service numbers.
        for mobile in [
            "+491511234567",
            "+491601234567",
            "+491711234567",
            "+491791234567",
        ] {
            assert_eq!(at(mobile), Some((Mobile, "DE", "Germany")), "{mobile}");
        }
        assert_eq!(at("+43664123456").map(|a| a.0), Some(Mobile));
        assert_eq!(at("+41791234567").map(|a| a.0), Some(Mobile));
        assert_eq!(at("+498001234567").map(|a| a.0), Some(Service));
        assert_eq!(at("+4918061234567").map(|a| a.0), Some(Service));
        assert_eq!(at("+49900123456").map(|a| a.0), Some(Service));
        // Other countries: places from the geocoding data, else the country.
        assert_eq!(at("+442071234567"), Some((Area, "GB", "London")));
        assert_eq!(geo.lookup("+442071234567").unwrap().prefix, "+44 20");
        assert_eq!(at("+390212345678"), Some((Area, "IT", "Milan")));
        assert_eq!(at("+33123456789"), Some((Country, "FR", "France")));
        assert_eq!(at("+18765551234").map(|a| a.1), Some("JM"));
        // Garbage and too short numbers.
        assert_eq!(at("+49"), None);
        assert_eq!(at("+4912"), None);
        assert_eq!(at("anonymous"), None);
        assert_eq!(at(""), None);
        assert_eq!(at("+999123456789"), None);
        // Every area has a country area for the "no location" bubble.
        for area in &geo.areas {
            assert!(geo.country(&area.country).is_some(), "{}", area.id);
        }
    }

    #[test]
    fn national_formats_resolve_after_normalization() {
        let geo = GeoIndex::load_dir(&data_dir()).unwrap();
        let dial = DialPlanSettings {
            area_code: "951".into(),
            ..DialPlanSettings::default()
        };
        for raw in ["095112345", "004995112345", "+4995112345", "12345"] {
            let e164 = dial.normalize_incoming(raw).unwrap();
            assert_eq!(geo.lookup(&e164).unwrap().name, "Bamberg", "{raw}");
        }
        let e164 = dial.normalize_incoming("0043 1 234567".replace(' ', "").as_str());
        assert_eq!(geo.lookup(&e164.unwrap()).unwrap().name, "Wien");
    }
}
