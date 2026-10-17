//! Standard UTC offsets come from IANA data, including fractional-hour zones.
use chrono::{Datelike, TimeZone};
use chrono_tz::{OffsetComponents, Tz};

pub fn standard_offset(zone: &str) -> i32 {
    let zone = zone.parse::<Tz>().unwrap_or(chrono_tz::UTC);
    zone.with_ymd_and_hms(chrono::Utc::now().year(), 1, 15, 12, 0, 0)
        .single()
        .map(|date| date.offset().base_utc_offset().num_seconds() as i32)
        .unwrap_or(0)
}
pub fn hours(zone: &str) -> String {
    let seconds = standard_offset(zone);
    let minutes = seconds.unsigned_abs() / 60;
    let sign = if seconds < 0 {
        "-"
    } else if seconds > 0 {
        "+"
    } else {
        ""
    };
    if minutes % 60 == 0 {
        format!("{sign}{}", minutes / 60)
    } else {
        format!("{sign}{}:{:02}", minutes / 60, minutes % 60)
    }
}
/// Explicit POSIX rules for the shared European defaults. Other zones use
/// their IANA identifier in profiles that support IANA names.
pub fn posix(zone: &str) -> Option<&'static str> {
    match zone {
        "UTC" | "Etc/UTC" => Some("UTC0"),
        "Europe/Berlin" | "Europe/Vienna" | "Europe/Zurich" | "Europe/Paris" | "Europe/Rome"
        | "Europe/Madrid" | "Europe/Amsterdam" => {
            Some("CET-1CEST-2,M3.5.0/02:00:00,M10.5.0/03:00:00")
        }
        "Europe/London" => Some("GMT+0BST-1,M3.5.0/01:00:00,M10.5.0/02:00:00"),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offsets_do_not_fall_back_to_germany() {
        assert_eq!(hours("UTC"), "0");
        assert_eq!(hours("Asia/Kolkata"), "+5:30");
        assert_eq!(hours("Asia/Kathmandu"), "+5:45");
        assert_eq!(hours("America/New_York"), "-5");
        assert_eq!(hours("Australia/Adelaide"), "+9:30");
    }
}
