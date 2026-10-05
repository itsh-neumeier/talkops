//! Phone provisioning: model catalog, Yealink configuration files and the
//! XML remote phonebook. Pure rendering – data access lives in the API crate.

pub mod catalog;
pub mod phonebook;
pub mod yealink;

pub use catalog::{PhoneCatalog, PhoneFamily, PhoneModel};

/// Removes characters that would break a `key = value` line of a Yealink
/// configuration file (line breaks) and trims surrounding whitespace.
/// Values are capped at 511 characters, the longest Yealink accepts (URLs).
pub fn cfg_value(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(511)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Normalizes a MAC address to 12 lowercase hex digits.
pub fn normalize_mac(mac: &str) -> Option<String> {
    let hex: String = mac
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
        .to_ascii_lowercase();
    let separators_ok = mac
        .chars()
        .all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '-' | '.'));
    (hex.len() == 12 && separators_ok).then_some(hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_values() {
        assert_eq!(cfg_value(" a\nb\r = c "), "ab = c");
        assert_eq!(
            normalize_mac("80:5E:C0:12:34:56").as_deref(),
            Some("805ec0123456")
        );
        assert_eq!(
            normalize_mac("805ec0123456").as_deref(),
            Some("805ec0123456")
        );
        assert_eq!(normalize_mac("80:5E"), None);
        assert_eq!(normalize_mac("80:5E:C0:12:34:5G"), None);
    }
}
