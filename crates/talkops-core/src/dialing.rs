//! Number normalization and dial rules.
//!
//! All numbers are stored in E.164 (`+4989123456`). Dialed digits from
//! extensions are normalized with the tenant's prefixes; numbers sent to a
//! trunk are formatted as the provider expects.

use serde::{Deserialize, Serialize};

/// Dialing context of a tenant (from `tenant_settings`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialPlanSettings {
    pub country_code: String,
    pub area_code: String,
    pub national_prefix: String,
    pub international_prefix: String,
    pub emergency_numbers: Vec<String>,
}

impl Default for DialPlanSettings {
    fn default() -> Self {
        Self {
            country_code: "49".into(),
            area_code: String::new(),
            national_prefix: "0".into(),
            international_prefix: "00".into(),
            emergency_numbers: vec!["110".into(), "112".into()],
        }
    }
}

/// Result of interpreting digits dialed by an extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialed {
    /// Emergency number; must go out via the default trunk unchanged.
    Emergency(String),
    /// Short service number (e.g. 115, 11833) sent unchanged.
    Service(String),
    /// Regular external number in E.164.
    External(String),
    /// Not dialable (local number without configured area code, garbage, ...).
    Invalid,
}

/// How a provider expects numbers in the request URI and caller ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NumberFormat {
    /// `+4989123456`
    #[default]
    E164,
    /// `4989123456`
    E164NoPlus,
    /// `004989123456`
    International,
    /// `089123456` for domestic numbers, `0044...` for foreign ones.
    National,
}

fn only_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

impl DialPlanSettings {
    /// Interprets digits dialed by an internal device (already known not to be
    /// an extension or feature code).
    pub fn classify(&self, dialed: &str) -> Dialed {
        let dialed: String = dialed
            .chars()
            .filter(|c| !matches!(c, ' ' | '-' | '/' | '(' | ')'))
            .collect();
        if self.emergency_numbers.contains(&dialed) {
            return Dialed::Emergency(dialed);
        }
        if let Some(rest) = dialed.strip_prefix('+') {
            return if only_digits(rest) && (5..=15).contains(&rest.len()) {
                Dialed::External(dialed)
            } else {
                Dialed::Invalid
            };
        }
        if !only_digits(&dialed) {
            return Dialed::Invalid;
        }
        if let Some(rest) = dialed.strip_prefix(self.international_prefix.as_str()) {
            return e164_or_invalid(rest);
        }
        if !self.national_prefix.is_empty() {
            if let Some(rest) = dialed.strip_prefix(self.national_prefix.as_str()) {
                return e164_or_invalid(&format!("{}{rest}", self.country_code));
            }
        }
        // German-style short codes (115, 116xxx, 118xx) never take an area code.
        if dialed.starts_with("11") && dialed.len() <= 6 {
            return Dialed::Service(dialed);
        }
        if self.area_code.is_empty() || dialed.len() < 3 {
            return Dialed::Invalid;
        }
        e164_or_invalid(&format!("{}{}{dialed}", self.country_code, self.area_code))
    }

    /// Normalizes a number received from a trunk (caller ID or DID in any of
    /// the usual formats) to E.164. Returns `None` for anonymous/garbage input.
    pub fn normalize_incoming(&self, raw: &str) -> Option<String> {
        let raw = raw.trim();
        let number = raw.split(['@', ';']).next().unwrap_or_default();
        let number = number.strip_prefix("sip:").unwrap_or(number);
        if let Some(rest) = number.strip_prefix('+') {
            return only_digits(rest).then(|| format!("+{rest}"));
        }
        if !only_digits(number) {
            return None;
        }
        if let Some(rest) = number.strip_prefix(self.international_prefix.as_str()) {
            return (!rest.is_empty()).then(|| format!("+{rest}"));
        }
        if !self.national_prefix.is_empty() {
            if let Some(rest) = number.strip_prefix(self.national_prefix.as_str()) {
                return Some(format!("+{}{rest}", self.country_code));
            }
        }
        // Without prefix: either already includes the country code (49...) or is
        // local. Real numbers with country code have at least 10 digits; shorter
        // ones starting with the country code are local numbers.
        if number.starts_with(self.country_code.as_str()) && number.len() >= 10 {
            return Some(format!("+{number}"));
        }
        if !self.area_code.is_empty() {
            return Some(format!("+{}{}{number}", self.country_code, self.area_code));
        }
        None
    }

    /// Formats an E.164 number for display on internal phones so that the
    /// number can be called back directly (national format for domestic numbers).
    pub fn for_display(&self, e164: &str) -> String {
        self.format(e164, NumberFormat::National)
    }

    /// Formats an E.164 number for a trunk.
    pub fn format(&self, e164: &str, format: NumberFormat) -> String {
        let Some(digits) = e164.strip_prefix('+') else {
            return e164.to_owned();
        };
        match format {
            NumberFormat::E164 => e164.to_owned(),
            NumberFormat::E164NoPlus => digits.to_owned(),
            NumberFormat::International => format!("{}{digits}", self.international_prefix),
            NumberFormat::National => match digits.strip_prefix(self.country_code.as_str()) {
                Some(rest) => format!("{}{rest}", self.national_prefix),
                None => format!("{}{digits}", self.international_prefix),
            },
        }
    }
}

fn e164_or_invalid(digits: &str) -> Dialed {
    if only_digits(digits) && (5..=15).contains(&digits.len()) && !digits.starts_with('0') {
        Dialed::External(format!("+{digits}"))
    } else {
        Dialed::Invalid
    }
}

/// Splits a German E.164 number into area code and local number using the
/// tenant's area code when it matches. Used for username templates like
/// LEONET's `leo49<area><number>`; returns `None` if the split is unknown.
pub fn split_area(e164: &str, country_code: &str, area_code: &str) -> Option<(String, String)> {
    let national = e164.strip_prefix('+')?.strip_prefix(country_code)?;
    if area_code.is_empty() {
        return None;
    }
    national
        .strip_prefix(area_code)
        .map(|local| (area_code.to_owned(), local.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn munich() -> DialPlanSettings {
        DialPlanSettings {
            area_code: "89".into(),
            ..Default::default()
        }
    }

    #[test]
    fn classifies_dialed_digits() {
        let s = munich();
        assert_eq!(s.classify("112"), Dialed::Emergency("112".into()));
        assert_eq!(s.classify("110"), Dialed::Emergency("110".into()));
        assert_eq!(s.classify("115"), Dialed::Service("115".into()));
        assert_eq!(s.classify("11833"), Dialed::Service("11833".into()));
        assert_eq!(
            s.classify("030 1234567"),
            Dialed::External("+49301234567".into())
        );
        assert_eq!(
            s.classify("0043 1 234567"),
            Dialed::External("+431234567".into())
        );
        assert_eq!(
            s.classify("+41 44 1234567"),
            Dialed::External("+41441234567".into())
        );
        assert_eq!(
            s.classify("1234567"),
            Dialed::External("+49891234567".into())
        );
        assert_eq!(s.classify("0"), Dialed::Invalid);
        assert_eq!(s.classify("000"), Dialed::Invalid);
        assert_eq!(s.classify("abc"), Dialed::Invalid);
        let no_area = DialPlanSettings::default();
        assert_eq!(no_area.classify("1234567"), Dialed::Invalid);
    }

    #[test]
    fn normalizes_incoming_numbers() {
        let s = munich();
        for raw in [
            "+4989123456",
            "004989123456",
            "4989123456",
            "089123456",
            "sip:+4989123456@x",
        ] {
            assert_eq!(
                s.normalize_incoming(raw).as_deref(),
                Some("+4989123456"),
                "{raw}"
            );
        }
        assert_eq!(
            s.normalize_incoming("123456").as_deref(),
            Some("+4989123456")
        );
        assert_eq!(
            s.normalize_incoming("4912345").as_deref(),
            Some("+49894912345")
        );
        assert_eq!(s.normalize_incoming("anonymous"), None);
        assert_eq!(DialPlanSettings::default().normalize_incoming("1234"), None);
    }

    #[test]
    fn formats_for_trunks_and_display() {
        let s = munich();
        assert_eq!(s.format("+49891234", NumberFormat::E164), "+49891234");
        assert_eq!(s.format("+49891234", NumberFormat::E164NoPlus), "49891234");
        assert_eq!(
            s.format("+49891234", NumberFormat::International),
            "0049891234"
        );
        assert_eq!(s.format("+49891234", NumberFormat::National), "0891234");
        assert_eq!(s.format("+431234", NumberFormat::National), "00431234");
        assert_eq!(s.for_display("+49891234"), "0891234");
    }

    #[test]
    fn splits_area_code() {
        assert_eq!(
            split_area("+49891234567", "49", "89"),
            Some(("89".into(), "1234567".into()))
        );
        assert_eq!(split_area("+49301234567", "49", "89"), None);
    }
}
