//! System prompts (voicemail menus, numbers) in every supported language.
//!
//! The media worker renders them with Piper into the shared sounds volume;
//! call logic plays them by key. The file name contains a hash of voice and
//! text, so changing a text or voice produces a new file automatically.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Languages with prompts and a bundled Piper voice.
pub const LANGUAGES: &[&str] = &["de", "en"];

/// Highest number with its own prompt (`n0` … `n99`).
pub const MAX_NUMBER: u32 = 99;

/// Piper voice used for a language (model file `<voice>.onnx`).
pub fn voice(lang: &str) -> &'static str {
    match lang {
        "en" => "en_US-ljspeech-medium",
        _ => "de_DE-thorsten-medium",
    }
}

/// Normalizes a language code to a supported one (default German).
pub fn language(lang: &str) -> &'static str {
    LANGUAGES
        .iter()
        .copied()
        .find(|l| lang.eq_ignore_ascii_case(l))
        .unwrap_or("de")
}

struct Prompt {
    key: &'static str,
    de: &'static str,
    en: &'static str,
}

const PROMPTS: &[Prompt] = &[
    Prompt {
        key: "vm_greeting_default",
        de: "Ihr Gesprächspartner ist gerade nicht erreichbar. Bitte hinterlassen Sie eine Nachricht nach dem Signalton.",
        en: "The person you are calling is not available. Please leave a message after the tone.",
    },
    Prompt {
        key: "vm_saved",
        de: "Vielen Dank, Ihre Nachricht wurde gespeichert. Auf Wiederhören.",
        en: "Thank you, your message has been saved. Goodbye.",
    },
    Prompt {
        key: "vm_enter_box",
        de: "Bitte geben Sie Ihre Nebenstelle ein, gefolgt von der Raute-Taste.",
        en: "Please enter your extension, followed by the pound key.",
    },
    Prompt {
        key: "vm_enter_pin",
        de: "Bitte geben Sie Ihre PIN ein, gefolgt von der Raute-Taste.",
        en: "Please enter your PIN, followed by the pound key.",
    },
    Prompt {
        key: "vm_login_failed",
        de: "Die Anmeldung ist fehlgeschlagen.",
        en: "Login incorrect.",
    },
    Prompt {
        key: "vm_you_have",
        de: "Sie haben",
        en: "You have",
    },
    Prompt {
        key: "vm_no_new_messages",
        de: "Sie haben keine neuen Nachrichten.",
        en: "You have no new messages.",
    },
    Prompt {
        key: "vm_one_new_message",
        de: "eine neue Nachricht.",
        en: "one new message.",
    },
    Prompt {
        key: "vm_new_messages",
        de: "neue Nachrichten.",
        en: "new messages.",
    },
    Prompt {
        key: "vm_and",
        de: "und",
        en: "and",
    },
    Prompt {
        key: "vm_one_saved_message",
        de: "eine gespeicherte Nachricht.",
        en: "one saved message.",
    },
    Prompt {
        key: "vm_saved_messages",
        de: "gespeicherte Nachrichten.",
        en: "saved messages.",
    },
    Prompt {
        key: "vm_main_menu",
        de: "Um Ihre Nachrichten abzuhören, drücken Sie die 1. Um Ihre Ansage aufzunehmen, drücken Sie die 5. Zum Beenden drücken Sie die Stern-Taste.",
        en: "To listen to your messages, press 1. To record your greeting, press 5. To exit, press star.",
    },
    Prompt {
        key: "vm_message",
        de: "Nachricht",
        en: "Message",
    },
    Prompt {
        key: "vm_from",
        de: "von",
        en: "from",
    },
    Prompt {
        key: "vm_message_menu",
        de: "Zum Wiederholen drücken Sie die 1, zum Löschen die 7, zum Speichern die 9. Für die nächste Nachricht drücken Sie die Raute-Taste.",
        en: "To repeat this message, press 1. To delete it, press 7. To save it, press 9. For the next message, press pound.",
    },
    Prompt {
        key: "vm_deleted",
        de: "Nachricht gelöscht.",
        en: "Message deleted.",
    },
    Prompt {
        key: "vm_message_saved",
        de: "Nachricht gespeichert.",
        en: "Message saved.",
    },
    Prompt {
        key: "vm_no_more_messages",
        de: "Keine weiteren Nachrichten.",
        en: "No more messages.",
    },
    Prompt {
        key: "vm_record_greeting",
        de: "Bitte sprechen Sie Ihre Ansage nach dem Signalton und drücken Sie anschließend die Raute-Taste.",
        en: "Please record your greeting after the tone, then press pound.",
    },
    Prompt {
        key: "vm_greeting_saved",
        de: "Ihre Ansage wurde gespeichert.",
        en: "Your greeting has been saved.",
    },
    Prompt {
        key: "vm_invalid",
        de: "Ungültige Eingabe.",
        en: "Invalid entry.",
    },
    Prompt {
        key: "vm_goodbye",
        de: "Auf Wiederhören.",
        en: "Goodbye.",
    },
];

/// Text of a prompt (`n<number>` for numbers), or `None` for unknown keys.
pub fn text(key: &str, lang: &str) -> Option<Cow<'static, str>> {
    if let Some(n) = key.strip_prefix('n').and_then(|n| n.parse::<u32>().ok()) {
        return (n <= MAX_NUMBER).then(|| Cow::Owned(n.to_string()));
    }
    PROMPTS.iter().find(|p| p.key == key).map(|p| {
        Cow::Borrowed(match language(lang) {
            "en" => p.en,
            _ => p.de,
        })
    })
}

/// Every prompt key, including the numbers.
pub fn keys() -> impl Iterator<Item = Cow<'static, str>> {
    PROMPTS
        .iter()
        .map(|p| Cow::Borrowed(p.key))
        .chain((0..=MAX_NUMBER).map(|n| Cow::Owned(format!("n{n}"))))
}

/// File the prompt is rendered to: `<sounds>/system/<lang>/<key>-<hash>.wav`.
pub fn path(sounds_dir: &Path, key: &str, lang: &str) -> Option<PathBuf> {
    let lang = language(lang);
    let text = text(key, lang)?;
    let digest = Sha256::digest(format!("{}\n{text}", voice(lang)).as_bytes());
    let hash = hex::encode(&digest[..4]);
    Some(
        sounds_dir
            .join("system")
            .join(lang)
            .join(format!("{key}-{hash}.wav")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_and_paths() {
        assert_eq!(text("vm_goodbye", "en").unwrap(), "Goodbye.");
        assert_eq!(text("vm_goodbye", "fr").unwrap(), "Auf Wiederhören.");
        assert_eq!(text("n42", "de").unwrap(), "42");
        assert!(text("n100", "de").is_none());
        assert!(text("nope", "de").is_none());
        assert_eq!(keys().count(), PROMPTS.len() + 100);

        let p = path(Path::new("/s"), "vm_goodbye", "de").unwrap();
        assert!(p.starts_with("/s/system/de"));
        let name = p.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("vm_goodbye-") && name.ends_with(".wav"));
        assert_ne!(p, path(Path::new("/s"), "vm_goodbye", "en").unwrap());
        // Every prompt has a German and an English text.
        for p in PROMPTS {
            assert!(!p.de.is_empty() && !p.en.is_empty(), "{}", p.key);
        }
    }
}
