//! Shared TTS speed validation and inline speed tags.

use crate::error::{CoreError, CoreResult};

pub fn default_speed() -> f64 {
    1.0
}

pub fn validate_speed(speed: f64) -> CoreResult<()> {
    if !speed.is_finite() || !(0.5..=2.0).contains(&speed) {
        return Err(CoreError::Validation(
            "speech speed must be 0.5 to 2.0".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
pub struct Segment {
    pub text: String,
    pub speed: f64,
}

/// Parses absolute speed factors. Untagged text uses `speed`.
/// Nested tags and empty tagged sections are invalid.
pub fn segments(text: &str, speed: f64) -> CoreResult<Vec<Segment>> {
    validate_speed(speed)?;
    let bad = || {
        CoreError::Validation(
            "use <speed:0.7>text</speed>; tags must not be empty or nested".into(),
        )
    };
    let mut out: Vec<Segment> = Vec::new();
    let mut rest = text;
    let mut push = |text: &str, speed| {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() {
            if let Some(last) = out.last_mut().filter(|s| s.speed == speed) {
                last.text.push(' ');
                last.text.push_str(&text);
            } else {
                out.push(Segment { text, speed });
            }
        }
    };
    while let Some(start) = rest.find('<') {
        if rest[..start].contains('>') {
            return Err(bad());
        }
        push(&rest[..start], speed);
        rest = rest[start..].strip_prefix("<speed:").ok_or_else(bad)?;
        let end = rest.find('>').ok_or_else(bad)?;
        let value = &rest[..end];
        if value.is_empty() || !value.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return Err(bad());
        }
        let factor: f64 = value.parse().map_err(|_| bad())?;
        validate_speed(factor)?;
        rest = &rest[end + 1..];
        let close = rest.find("</speed>").ok_or_else(bad)?;
        let inner = &rest[..close];
        if inner.trim().is_empty() || inner.contains(['<', '>']) {
            return Err(bad());
        }
        push(inner, factor);
        rest = &rest[close + "</speed>".len()..];
    }
    if rest.contains('>') {
        return Err(bad());
    }
    push(rest, speed);
    if out.is_empty() {
        return Err(CoreError::Validation("the spoken text is empty".into()));
    }
    if out.len() > 64 {
        return Err(CoreError::Validation("at most 64 speech segments".into()));
    }
    Ok(out)
}

/// Encodes the effective speed without nesting existing tags. This text is
/// also the cache identity, so old untagged text at 1.0 keeps its old file.
pub fn with_speed(text: &str, speed: f64) -> CoreResult<String> {
    let parts = segments(text, speed)?;
    if speed == 1.0 && !text.contains('<') {
        return Ok(text.to_owned());
    }
    Ok(parts
        .into_iter()
        .map(|s| format!("<speed:{}>{}</speed>", s.speed, s.text))
        .collect::<Vec<_>>()
        .join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_tags_restore_the_default() {
        let parts = segments("Hello. <speed:0.7>Änderung</speed> Goodbye.", 1.2).unwrap();
        assert_eq!(
            parts.iter().map(|s| s.speed).collect::<Vec<_>>(),
            [1.2, 0.7, 1.2]
        );
        assert_eq!(parts[1].text, "Änderung");
        let encoded = with_speed("A <speed:0.7>B</speed> C", 0.8).unwrap();
        assert_eq!(
            segments(&encoded, 1.0).unwrap(),
            segments("A <speed:0.7>B</speed> C", 0.8).unwrap()
        );
        assert_eq!(with_speed("Hello.", 1.0).unwrap(), "Hello.");
    }

    #[test]
    fn rejects_invalid_input() {
        for text in [
            "",
            "</speed>",
            "<speed:0.7>x",
            "<speed:NaN>x</speed>",
            "<speed:0.4>x</speed>",
            "<speed:2.1>x</speed>",
            "<speed:1> </speed>",
            "<speed:1><speed:1>x</speed></speed>",
            "<speed 0.7>x</speed>",
        ] {
            assert!(segments(text, 1.0).is_err(), "{text}");
        }
        for speed in [f64::NAN, f64::INFINITY, 0.0, 2.1] {
            assert!(validate_speed(speed).is_err());
        }
        assert!(segments(&"a <speed:0.7>b</speed> ".repeat(33), 1.0).is_err());
        for speed in [0.5, 1.0, 2.0] {
            validate_speed(speed).unwrap();
        }
    }
}
