//! Public holidays in Germany, nationwide and per federal state.
//!
//! Only holidays that apply to the whole state are included; holidays that
//! depend on the municipality (Fronleichnam in parts of Saxony and
//! Thuringia, Mariä Himmelfahrt in Catholic Bavarian municipalities, the
//! Augsburger Friedensfest) are not – add them as closed dates if needed.

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde::Serialize;

/// Supported regions: `DE` (nationwide holidays only) and `DE-XX`.
pub const REGIONS: &[(&str, &str)] = &[
    ("DE", "Deutschland (bundesweit)"),
    ("DE-BW", "Baden-Württemberg"),
    ("DE-BY", "Bayern"),
    ("DE-BE", "Berlin"),
    ("DE-BB", "Brandenburg"),
    ("DE-HB", "Bremen"),
    ("DE-HH", "Hamburg"),
    ("DE-HE", "Hessen"),
    ("DE-MV", "Mecklenburg-Vorpommern"),
    ("DE-NI", "Niedersachsen"),
    ("DE-NW", "Nordrhein-Westfalen"),
    ("DE-RP", "Rheinland-Pfalz"),
    ("DE-SL", "Saarland"),
    ("DE-SN", "Sachsen"),
    ("DE-ST", "Sachsen-Anhalt"),
    ("DE-SH", "Schleswig-Holstein"),
    ("DE-TH", "Thüringen"),
];

pub fn is_region(region: &str) -> bool {
    REGIONS.iter().any(|(code, _)| *code == region)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct Holiday {
    #[schema(value_type = String, format = Date)]
    pub date: NaiveDate,
    pub name: &'static str,
}

/// Easter Sunday (Gregorian calendar, anonymous algorithm).
pub fn easter(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).expect("valid Easter date")
}

/// Holidays of `year` in `region`, sorted by date.
pub fn holidays(region: &str, year: i32) -> Vec<Holiday> {
    let state = region.strip_prefix("DE-").unwrap_or("");
    let in_state = |states: &[&str]| states.contains(&state);
    let date = |m: u32, d: u32| NaiveDate::from_ymd_opt(year, m, d).expect("valid date");
    let e = easter(year);
    let mut list = vec![
        Holiday {
            date: date(1, 1),
            name: "Neujahr",
        },
        Holiday {
            date: e - Duration::days(2),
            name: "Karfreitag",
        },
        Holiday {
            date: e + Duration::days(1),
            name: "Ostermontag",
        },
        Holiday {
            date: date(5, 1),
            name: "Tag der Arbeit",
        },
        Holiday {
            date: e + Duration::days(39),
            name: "Christi Himmelfahrt",
        },
        Holiday {
            date: e + Duration::days(50),
            name: "Pfingstmontag",
        },
        Holiday {
            date: date(10, 3),
            name: "Tag der Deutschen Einheit",
        },
        Holiday {
            date: date(12, 25),
            name: "1. Weihnachtstag",
        },
        Holiday {
            date: date(12, 26),
            name: "2. Weihnachtstag",
        },
    ];
    if in_state(&["BW", "BY", "ST"]) {
        list.push(Holiday {
            date: date(1, 6),
            name: "Heilige Drei Könige",
        });
    }
    if (state == "BE" && year >= 2019) || (state == "MV" && year >= 2023) {
        list.push(Holiday {
            date: date(3, 8),
            name: "Internationaler Frauentag",
        });
    }
    if state == "BB" {
        list.push(Holiday {
            date: e,
            name: "Ostersonntag",
        });
        list.push(Holiday {
            date: e + Duration::days(49),
            name: "Pfingstsonntag",
        });
    }
    if in_state(&["BW", "BY", "HE", "NW", "RP", "SL"]) {
        list.push(Holiday {
            date: e + Duration::days(60),
            name: "Fronleichnam",
        });
    }
    if state == "SL" {
        list.push(Holiday {
            date: date(8, 15),
            name: "Mariä Himmelfahrt",
        });
    }
    if state == "TH" && year >= 2019 {
        list.push(Holiday {
            date: date(9, 20),
            name: "Weltkindertag",
        });
    }
    // Nationwide in the anniversary year 2017.
    if in_state(&["BB", "MV", "SN", "ST", "TH"])
        || (in_state(&["HB", "HH", "NI", "SH"]) && year >= 2018)
        || year == 2017
    {
        list.push(Holiday {
            date: date(10, 31),
            name: "Reformationstag",
        });
    }
    if in_state(&["BW", "BY", "NW", "RP", "SL"]) {
        list.push(Holiday {
            date: date(11, 1),
            name: "Allerheiligen",
        });
    }
    if state == "SN" {
        // Wednesday before 23 November.
        let mut d = date(11, 22);
        while d.weekday() != Weekday::Wed {
            d -= Duration::days(1);
        }
        list.push(Holiday {
            date: d,
            name: "Buß- und Bettag",
        });
    }
    list.sort_by_key(|h| h.date);
    list
}

/// The holiday on `date` in `region`, if any.
pub fn holiday_on(region: &str, date: NaiveDate) -> Option<Holiday> {
    holidays(region, date.year())
        .into_iter()
        .find(|h| h.date == date)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn easter_dates() {
        assert_eq!(easter(2024), d(2024, 3, 31));
        assert_eq!(easter(2025), d(2025, 4, 20));
        assert_eq!(easter(2026), d(2026, 4, 5));
        assert_eq!(easter(2027), d(2027, 3, 28));
        assert_eq!(easter(2038), d(2038, 4, 25));
    }

    #[test]
    fn regional_holidays() {
        let by = holidays("DE-BY", 2026);
        let names: Vec<_> = by.iter().map(|h| h.name).collect();
        assert!(names.contains(&"Heilige Drei Könige"));
        assert!(names.contains(&"Fronleichnam"));
        assert!(!names.contains(&"Reformationstag"));
        assert_eq!(by.len(), 12);
        assert_eq!(
            holiday_on("DE-BY", d(2026, 6, 4)).unwrap().name,
            "Fronleichnam"
        );
        assert_eq!(holidays("DE", 2026).len(), 9);
        assert_eq!(
            holiday_on("DE-SN", d(2026, 11, 18)).unwrap().name,
            "Buß- und Bettag"
        );
        assert!(holiday_on("DE-NI", d(2026, 10, 31)).is_some());
        assert!(holiday_on("DE-NI", d(2017, 10, 31)).is_some());
        assert!(holiday_on("DE-NW", d(2026, 10, 31)).is_none());
        assert!(holiday_on("DE-BE", d(2026, 3, 8)).is_some());
        assert!(holiday_on("DE-TH", d(2026, 9, 20)).is_some());
        assert!(holiday_on("DE", d(2026, 12, 24)).is_none());
        assert!(REGIONS.iter().all(|(r, _)| !holidays(r, 2026).is_empty()));
    }
}
