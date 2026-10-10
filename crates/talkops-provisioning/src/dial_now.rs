//! Yealink "dial now" rules: numbers the phone sends as soon as they are
//! complete instead of waiting for the inter-digit timeout or the send key.
//!
//! Parameters `dialplan.dialnow.rule.X` (X = 1–20); the rule matches the
//! whole input, digits and `*` are literal, `[1,2,3]` matches one character
//! from the list. Source: Yealink documentation "Dial Plan" (SIP-T73U,
//! support.yealink.com) and "Using Dial Plan Feature on Yealink SIP-T3XG
//! Phones" (special characters: "Digit 0-9 *").
//!
//! A number only gets a rule if nothing else that can be dialed starts with
//! it – otherwise the phone would cut off the longer number. System codes
//! (`*1`–`*99`) always get their rule; `*<internal>` starts at `*100`.

use std::collections::BTreeMap;

/// Dial-now rules a phone accepts.
pub const MAX_RULES: usize = 20;

/// What can be dialed on the PBX besides the internal numbers.
#[derive(Debug, Clone, Default)]
pub struct DialNowInput<'a> {
    /// Emergency numbers (110, 112, …); always first.
    pub emergency: &'a [String],
    /// Feature codes with a fixed length (`*97`, `*51`, …).
    pub fixed_codes: &'a [&'a str],
    /// Prefixes of feature codes followed by more digits (`*72`, `**`, …).
    pub code_prefixes: &'a [&'a str],
    /// Internal numbers of extensions, groups, menus and queues.
    pub internal: &'a [String],
    /// Numbers without trunk prefix are local calls (an area code is set),
    /// so internal numbers may be the start of an external number.
    pub local_dialing: bool,
}

/// Computes the rules, most important first, at most [`MAX_RULES`].
pub fn rules(input: &DialNowInput<'_>) -> Vec<String> {
    // Everything that can be dialed. `*1`–`*99` are reserved for system
    // codes, which win over `*<internal>` (from `*100`): with extension 510,
    // `*51` still parks at once and 510 is dialed without the star.
    let mut dialable: Vec<String> = input.internal.to_vec();
    dialable.extend(input.fixed_codes.iter().map(|c| (*c).to_owned()));
    dialable.extend(input.emergency.iter().cloned());
    let safe = |n: &str| {
        !n.is_empty()
            && n.bytes().all(|b| b.is_ascii_digit() || b == b'*')
            && !dialable
                .iter()
                .any(|d| d.len() > n.len() && d.starts_with(n))
            && !input
                .code_prefixes
                .iter()
                .any(|p| p.starts_with(n) || n.starts_with(p))
    };

    let mut out = group(
        input
            .emergency
            .iter()
            .map(String::as_str)
            .filter(|n| safe(n)),
    );
    out.extend(group(input.fixed_codes.iter().copied().filter(|n| safe(n))));
    if !input.local_dialing {
        // 11x are service numbers (115, 116116, 11833 …).
        out.extend(group(
            input
                .internal
                .iter()
                .map(String::as_str)
                .filter(|n| !n.starts_with("11") && safe(n)),
        ));
    }
    out.truncate(MAX_RULES);
    out
}

/// Merges numbers that only differ in the last digit: `20 21 22` → `2[0,1,2]`.
fn group<'a>(numbers: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut groups: BTreeMap<(usize, &str), Vec<char>> = BTreeMap::new();
    for n in numbers {
        let Some(last) = n.chars().last() else {
            continue;
        };
        let entry = groups.entry((n.len(), &n[..n.len() - 1])).or_default();
        if !entry.contains(&last) {
            entry.push(last);
        }
    }
    groups
        .into_iter()
        .map(|((_, prefix), mut last)| {
            last.sort_unstable();
            match last.as_slice() {
                [one] => format!("{prefix}{one}"),
                many => format!(
                    "{prefix}[{}]",
                    many.iter()
                        .map(char::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXED: &[&str] = &["*51", "*52", "*59", "*73", "*78", "*79", "*97", "*98"];
    const PREFIXES: &[&str] = &["**", "*30", "*31", "*72", "*85", "*86", "#31#"];

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn groups_and_skips_ambiguous_numbers() {
        let emergency = s(&["110", "112"]);
        let internal = s(&["20", "21", "22", "30", "300", "471", "510", "8001", "116"]);
        let input = DialNowInput {
            emergency: &emergency,
            fixed_codes: FIXED,
            code_prefixes: PREFIXES,
            internal: &internal,
            local_dialing: false,
        };
        assert_eq!(
            rules(&input),
            [
                "11[0,2]",
                // System codes win over `*510` (= internal 510).
                "*5[1,2,9]",
                "*7[3,8,9]",
                "*9[7,8]",
                "2[0,1,2]",
                // 30 is the start of 300; 116 is a service number.
                "300",
                "471",
                "510",
                "8001",
            ]
        );
    }

    #[test]
    fn local_dialing_keeps_internal_numbers_waiting() {
        let emergency = s(&["110", "112"]);
        let internal = s(&["20", "21"]);
        let input = DialNowInput {
            emergency: &emergency,
            fixed_codes: &["*97"],
            code_prefixes: PREFIXES,
            internal: &internal,
            local_dialing: true,
        };
        assert_eq!(rules(&input), ["11[0,2]", "*97"]);
    }

    #[test]
    fn at_most_twenty_rules_emergency_first() {
        let emergency = s(&["112"]);
        let internal: Vec<String> = (0..40).map(|i| format!("{}0", 20 + i)).collect();
        let input = DialNowInput {
            emergency: &emergency,
            internal: &internal,
            ..Default::default()
        };
        let r = rules(&input);
        assert_eq!(r.len(), MAX_RULES);
        assert_eq!(r[0], "112");
    }
}
