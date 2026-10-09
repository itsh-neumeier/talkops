//! Editable voicemail control: menu keys, prompt texts and voices per
//! language, and what is announced about a message's caller.
//!
//! The menu structure is fixed (main menu, message menu); every key can be
//! moved and every voicemail prompt reworded. Menu prompts are templates
//! whose `{placeholders}` name the keys, so moving a key also changes what
//! the prompt says. Stored per tenant in `tenant_settings.voicemail_config`.

use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;

use crate::error::{CoreError, CoreResult};
use crate::prompts;
use crate::tenant::TenantId;

/// Job: render the prompts a tenant changed (texts, voice); payload `{}`.
pub const JOB_TTS_PROMPTS: &str = "tts_prompts";
/// Job: render a message's caller announcement; payload [MessageInfoJob].
pub const JOB_TTS_MESSAGE_INFO: &str = "tts_message_info";

/// Keys of the two menus (`0`-`9`, `*`, `#`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(default)]
pub struct MenuKeys {
    /// Main menu: listen to messages.
    pub listen: String,
    /// Main menu: record the greeting.
    pub greeting: String,
    /// Main menu: hang up.
    pub exit: String,
    /// Message menu: play again.
    pub repeat: String,
    pub delete: String,
    pub save: String,
    /// Message menu: next message (keeps it as heard).
    pub next: String,
}

impl Default for MenuKeys {
    fn default() -> Self {
        Self {
            listen: "1".into(),
            greeting: "5".into(),
            exit: "*".into(),
            repeat: "1".into(),
            delete: "7".into(),
            save: "9".into(),
            next: "#".into(),
        }
    }
}

impl MenuKeys {
    pub fn main(&self) -> [(&'static str, &str); 3] {
        [
            ("listen", &self.listen),
            ("greeting", &self.greeting),
            ("exit", &self.exit),
        ]
    }

    pub fn message(&self) -> [(&'static str, &str); 4] {
        [
            ("repeat", &self.repeat),
            ("delete", &self.delete),
            ("save", &self.save),
            ("next", &self.next),
        ]
    }
}

/// What is said about the caller before a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(default)]
pub struct Announce {
    /// Name from the phone book or the extension list.
    pub name: bool,
    /// The number, digit by digit in groups.
    pub number: bool,
    /// When the message came in.
    pub date: bool,
    /// The name the provider sent, if the phone book knows none.
    pub cnam: bool,
}

impl Default for Announce {
    fn default() -> Self {
        Self {
            name: true,
            number: true,
            date: true,
            cnam: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(default)]
pub struct VoicemailConfig {
    pub keys: MenuKeys,
    /// Reworded prompts: language → prompt key → text.
    pub texts: BTreeMap<String, BTreeMap<String, String>>,
    /// Voice per language (see [crate::audio::VOICES]); default voice 1.
    pub voices: BTreeMap<String, i16>,
    pub announce: Announce,
}

/// Prompts that can be reworded, with the placeholders they understand.
pub const EDITABLE: &[(&str, &[&str])] = &[
    ("vm_greeting_default", &[]),
    ("vm_saved", &[]),
    ("vm_enter_box", &[]),
    ("vm_enter_pin", &[]),
    ("vm_login_failed", &[]),
    ("vm_you_have", &[]),
    ("vm_no_new_messages", &[]),
    ("vm_one_new_message", &[]),
    ("vm_new_messages", &[]),
    ("vm_and", &[]),
    ("vm_one_saved_message", &[]),
    ("vm_saved_messages", &[]),
    ("vm_main_menu", &["listen", "greeting", "exit"]),
    ("vm_message", &[]),
    ("vm_message_menu", &["repeat", "delete", "save", "next"]),
    ("vm_info_caller", &["caller"]),
    ("vm_info_date", &["date"]),
    ("vm_deleted", &[]),
    ("vm_message_saved", &[]),
    ("vm_no_more_messages", &[]),
    ("vm_record_greeting", &[]),
    ("vm_greeting_saved", &[]),
    ("vm_invalid", &[]),
    ("vm_goodbye", &[]),
];

const MAX_TEXT: usize = 500;

fn valid_key(k: &str) -> bool {
    k.len() == 1
        && k.chars()
            .all(|c| c.is_ascii_digit() || c == '*' || c == '#')
}

/// `{name}` placeholders in a text.
fn placeholders(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        out.push(&rest[start + 1..start + end]);
        rest = &rest[start + end + 1..];
    }
    out
}

impl VoicemailConfig {
    /// Checks keys (single key, unique per menu), texts and voices.
    pub fn validate(&self) -> CoreResult<()> {
        let bad = |m: &str| Err(CoreError::Validation(m.into()));
        for menu in [&self.keys.main()[..], &self.keys.message()[..]] {
            let mut seen = Vec::new();
            for (_, key) in menu {
                if !valid_key(key) {
                    return bad("menu keys must be 0-9, * or #");
                }
                if seen.contains(key) {
                    return bad("a key is used twice in the same menu");
                }
                seen.push(*key);
            }
        }
        for (lang, texts) in &self.texts {
            if !prompts::LANGUAGES.contains(&lang.as_str()) {
                return bad("unknown language");
            }
            for (key, text) in texts {
                let Some((_, allowed)) = EDITABLE.iter().find(|(k, _)| k == key) else {
                    return bad("this prompt cannot be changed");
                };
                if text.trim().is_empty()
                    || text.chars().count() > MAX_TEXT
                    || text.chars().any(char::is_control)
                {
                    return bad("prompt texts: 1 to 500 characters on one line");
                }
                if placeholders(text).iter().any(|p| !allowed.contains(p)) {
                    return bad("unknown placeholder in a prompt text");
                }
            }
        }
        for (lang, voice) in &self.voices {
            if !crate::audio::VOICES
                .iter()
                .any(|v| v.language == lang && v.voice == *voice)
            {
                return bad("unknown voice");
            }
        }
        Ok(())
    }

    /// Drops texts equal to the default (keeps the stored config small).
    pub fn normalized(mut self) -> Self {
        for (lang, texts) in &mut self.texts {
            texts.retain(|key, text| {
                *text = text.trim().to_owned();
                prompts::template(key, lang).as_deref() != Some(text.as_str())
            });
        }
        self.texts.retain(|_, t| !t.is_empty());
        self.voices.retain(|_, v| *v != 1);
        self
    }

    /// The Piper voice model for a language.
    pub fn voice_model(&self, lang: &str) -> &'static str {
        let lang = prompts::language(lang);
        crate::audio::voice(lang, self.voices.get(lang).copied().unwrap_or(1)).model
    }

    /// Text of a prompt in `lang` (reworded or default), keys filled in.
    pub fn text(&self, key: &str, lang: &str) -> Option<String> {
        let lang = prompts::language(lang);
        let template = match self.texts.get(lang).and_then(|t| t.get(key)) {
            Some(t) => t.clone(),
            None => prompts::template(key, lang)?.into_owned(),
        };
        Some(fill_keys(&template, &self.keys, lang))
    }
}

/// How a key is spoken: German "die 1", "die Stern-Taste"; English "1", "star".
pub fn spoken_key(key: &str, lang: &str) -> String {
    match (prompts::language(lang), key) {
        ("en", "*") => "star".into(),
        ("en", "#") => "pound".into(),
        ("en", k) => k.into(),
        (_, "*") => "die Stern-Taste".into(),
        (_, "#") => "die Raute-Taste".into(),
        (_, k) => format!("die {k}"),
    }
}

/// Replaces `{listen}`, `{delete}`, … with the spoken keys.
pub fn fill_keys(template: &str, keys: &MenuKeys, lang: &str) -> String {
    let mut text = template.to_owned();
    for (name, key) in keys.main().into_iter().chain(keys.message()) {
        text = text.replace(&format!("{{{name}}}"), &spoken_key(key, lang));
    }
    text
}

pub async fn get<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<VoicemailConfig> {
    let value: serde_json::Value =
        sqlx::query_scalar("SELECT voicemail_config FROM tenant_settings WHERE tenant_id = $1")
            .bind(tenant)
            .fetch_one(db)
            .await?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

pub async fn set<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    config: &VoicemailConfig,
) -> CoreResult<()> {
    config.validate()?;
    sqlx::query(
        "UPDATE tenant_settings SET voicemail_config = $2, updated_at = now() WHERE tenant_id = $1",
    )
    .bind(tenant)
    .bind(sqlx::types::Json(config))
    .execute(db)
    .await?;
    Ok(())
}

/// Payload of [JOB_TTS_MESSAGE_INFO]: text rendered into the voicemail
/// volume next to the message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInfoJob {
    pub text: String,
    /// Piper voice model.
    pub voice: String,
    /// Relative to the voicemail volume.
    pub file: String,
}

/// Announcement file of a message (`…/<id>.wav` → `…/<id>-info.wav`).
pub fn info_file(message_file: &str) -> String {
    match message_file.strip_suffix(".wav") {
        Some(stem) => format!("{stem}-info.wav"),
        None => format!("{message_file}-info.wav"),
    }
}

/// A number spoken digit by digit in groups: "030123456" → "0 3 0, 1 2 3,
/// 4 5 6". Parts separated by spaces stay apart; parts longer than four
/// digits are split into threes, a lonely last digit joins its neighbours.
pub fn grouped_digits(display: &str) -> String {
    let mut groups: Vec<Vec<char>> = Vec::new();
    for part in display.split_whitespace() {
        let digits: Vec<char> = part.chars().filter(char::is_ascii_digit).collect();
        if digits.len() <= 4 {
            if !digits.is_empty() {
                groups.push(digits);
            }
            continue;
        }
        let mut chunks: Vec<Vec<char>> = digits.chunks(3).map(<[char]>::to_vec).collect();
        if chunks.last().is_some_and(|c| c.len() == 1) {
            let last = chunks.pop().unwrap_or_default();
            if let Some(prev) = chunks.last_mut() {
                prev.extend(last);
            }
        }
        groups.extend(chunks);
    }
    groups
        .iter()
        .map(|g| g.iter().map(char::to_string).collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Who left a message, as far as known.
#[derive(Debug, Clone, Default)]
pub struct Caller {
    /// From the phone book or the extension list.
    pub name: Option<String>,
    /// Display name sent by the provider.
    pub cnam: Option<String>,
    /// Number as shown to users (national format or extension).
    pub number: Option<String>,
}

const WEEKDAYS_DE: [&str; 7] = [
    "Montag",
    "Dienstag",
    "Mittwoch",
    "Donnerstag",
    "Freitag",
    "Samstag",
    "Sonntag",
];
const MONTHS_DE: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];
const WEEKDAYS_EN: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// "Donnerstag, 8. Oktober, um 14 Uhr 32" / "Thursday, October 8, at 2:32 PM".
pub fn spoken_date(at: DateTime<Utc>, tz: chrono_tz::Tz, lang: &str) -> String {
    let t = at.with_timezone(&tz);
    let wd = t.weekday().num_days_from_monday() as usize;
    let month = t.month0() as usize;
    match prompts::language(lang) {
        "en" => {
            let (pm, h12) = t.hour12();
            format!(
                "{}, {} {}, at {}:{:02} {}",
                WEEKDAYS_EN[wd],
                MONTHS_EN[month],
                t.day(),
                h12,
                t.minute(),
                if pm { "PM" } else { "AM" }
            )
        }
        _ => {
            let time = if t.minute() == 0 {
                format!("{} Uhr", t.hour())
            } else {
                format!("{} Uhr {}", t.hour(), t.minute())
            };
            format!(
                "{}, {}. {}, um {time}",
                WEEKDAYS_DE[wd],
                t.day(),
                MONTHS_DE[month]
            )
        }
    }
}

/// Text announced before a message ("von Anna Müller, 0 1 5 2 …. Empfangen
/// am …"), or `None` if nothing is to be said.
pub fn message_info(
    config: &VoicemailConfig,
    lang: &str,
    caller: &Caller,
    at: DateTime<Utc>,
    tz: chrono_tz::Tz,
) -> Option<String> {
    let lang = prompts::language(lang);
    let a = config.announce;
    let name = caller
        .name
        .clone()
        .filter(|_| a.name)
        .or_else(|| caller.cnam.clone().filter(|_| a.cnam))
        .map(|n| clean(&n))
        .filter(|n| !n.is_empty());
    let number = caller
        .number
        .as_deref()
        .filter(|_| a.number)
        .map(grouped_digits)
        .filter(|n| !n.is_empty());
    let who = match (name, number) {
        (Some(n), Some(d)) => Some(format!("{n}, {d}")),
        (Some(n), None) => Some(n),
        (None, Some(d)) => Some(d),
        (None, None) if a.name || a.number || a.cnam => Some(
            if lang == "en" {
                "an unknown caller"
            } else {
                "einem unbekannten Anrufer"
            }
            .to_owned(),
        ),
        (None, None) => None,
    };
    let mut parts = Vec::new();
    if let Some(who) = who {
        parts.push(
            config
                .text("vm_info_caller", lang)?
                .replace("{caller}", &who),
        );
    }
    if a.date {
        parts.push(
            config
                .text("vm_info_date", lang)?
                .replace("{date}", &spoken_date(at, tz, lang)),
        );
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// Names are spoken: keep letters, digits and simple punctuation.
fn clean(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || " .-'&,".contains(*c))
        .take(80)
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn default_menu_texts_are_unchanged() {
        let c = VoicemailConfig::default();
        assert_eq!(
            c.text("vm_main_menu", "de").unwrap(),
            "Um Ihre Nachrichten abzuhören, drücken Sie die 1. Um Ihre Ansage aufzunehmen, drücken Sie die 5. Zum Beenden drücken Sie die Stern-Taste."
        );
        assert_eq!(
            c.text("vm_message_menu", "en").unwrap(),
            "To repeat this message, press 1. To delete it, press 7. To save it, press 9. For the next message, press pound."
        );
        // The same text as before, so existing prompt files stay valid.
        assert_eq!(
            prompts::text("vm_main_menu", "de").unwrap(),
            c.text("vm_main_menu", "de").unwrap()
        );
    }

    #[test]
    fn moved_keys_change_the_prompt() {
        let mut c = VoicemailConfig::default();
        c.keys.delete = "3".into();
        c.keys.next = "6".into();
        let t = c.text("vm_message_menu", "de").unwrap();
        assert!(t.contains("zum Löschen die 3"), "{t}");
        assert!(t.contains("drücken Sie die 6"), "{t}");
        c.validate().unwrap();
        c.keys.save = "3".into();
        assert!(c.validate().is_err());
        c.keys.save = "x".into();
        assert!(c.validate().is_err());
    }

    #[test]
    fn texts_are_validated_and_normalized() {
        let mut c = VoicemailConfig::default();
        c.texts.insert(
            "de".into(),
            BTreeMap::from([
                ("vm_goodbye".into(), " Tschüss! ".into()),
                ("vm_and".into(), "und".into()),
            ]),
        );
        c.validate().unwrap();
        let n = c.clone().normalized();
        // Equal to the default: dropped; others trimmed.
        assert_eq!(
            n.texts["de"],
            BTreeMap::from([("vm_goodbye".into(), "Tschüss!".into())])
        );
        c.texts
            .get_mut("de")
            .unwrap()
            .insert("vm_main_menu".into(), "Drück {foo}".into());
        assert!(c.validate().is_err());
        c.texts
            .get_mut("de")
            .unwrap()
            .insert("vm_main_menu".into(), "Drück {listen}".into());
        c.validate().unwrap();
        c.texts
            .get_mut("de")
            .unwrap()
            .insert("rec_announcement".into(), "x".into());
        assert!(c.validate().is_err());
        let mut c = VoicemailConfig::default();
        c.voices.insert("de".into(), 2);
        c.validate().unwrap();
        assert_eq!(c.voice_model("de"), "de_DE-kerstin-low");
        c.voices.insert("de".into(), 9);
        assert!(c.validate().is_err());
    }

    #[test]
    fn numbers_are_grouped() {
        assert_eq!(grouped_digits("015237566022"), "0 1 5, 2 3 7, 5 6 6, 0 2 2");
        assert_eq!(grouped_digits("030123456"), "0 3 0, 1 2 3, 4 5 6");
        assert_eq!(
            grouped_digits("0152 37566022"),
            "0 1 5 2, 3 7 5, 6 6 0, 2 2"
        );
        assert_eq!(grouped_digits("1234567"), "1 2 3, 4 5 6 7");
        assert_eq!(grouped_digits("21"), "2 1");
        assert_eq!(grouped_digits(""), "");
    }

    #[test]
    fn caller_announcement() {
        let c = VoicemailConfig::default();
        let tz: chrono_tz::Tz = "Europe/Berlin".parse().unwrap();
        let at = Utc.with_ymd_and_hms(2026, 10, 8, 12, 32, 0).unwrap();
        let caller = Caller {
            name: Some("Anna Müller".into()),
            cnam: Some("ANNA M".into()),
            number: Some("030123456".into()),
        };
        assert_eq!(
            message_info(&c, "de", &caller, at, tz).unwrap(),
            "von Anna Müller, 0 3 0, 1 2 3, 4 5 6. Empfangen am Donnerstag, 8. Oktober, um 14 Uhr 32."
        );
        assert_eq!(
            message_info(&c, "en", &caller, at, tz).unwrap(),
            "from Anna Müller, 0 3 0, 1 2 3, 4 5 6. Received on Thursday, October 8, at 2:32 PM."
        );
        // Not in the phone book: the provider's name; nothing known: unknown.
        let cnam = Caller {
            name: None,
            ..caller.clone()
        };
        assert!(
            message_info(&c, "de", &cnam, at, tz)
                .unwrap()
                .starts_with("von ANNA M, 0 3 0")
        );
        let mut only_date = c.clone();
        only_date.announce = Announce {
            name: false,
            number: false,
            cnam: false,
            date: true,
        };
        assert_eq!(
            message_info(&only_date, "de", &caller, at, tz).unwrap(),
            "Empfangen am Donnerstag, 8. Oktober, um 14 Uhr 32."
        );
        assert_eq!(
            message_info(&c, "de", &Caller::default(), at, tz).unwrap(),
            "von einem unbekannten Anrufer. Empfangen am Donnerstag, 8. Oktober, um 14 Uhr 32."
        );
        only_date.announce.date = false;
        assert!(message_info(&only_date, "de", &caller, at, tz).is_none());
    }
}
