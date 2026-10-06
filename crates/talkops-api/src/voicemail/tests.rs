//! Voicemail flows against a scripted call and a real database.

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::path::Path;

use sqlx::PgPool;
use talkops_core::extensions::{self, ExtensionInput};
use talkops_core::tenant::TenantId;
use talkops_core::voicemail::{self, VoicemailBoxInput};
use talkops_esl::{EslError, Event, Headers};

use super::ivr::Call;
use super::*;

const T: TenantId = TenantId::DEFAULT;

/// Scripted call: answers DTMF prompts from a queue and "records" silence.
#[derive(Default)]
struct FakeCall {
    vars: HashMap<String, String>,
    digits: VecDeque<&'static str>,
    record_secs: VecDeque<u32>,
    executed: Vec<(String, String)>,
    hung_up: bool,
}

fn write_wav(path: &Path, secs: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..secs * 8000 {
        w.write_sample(0i16).unwrap();
    }
    w.finalize().unwrap();
}

impl Call for FakeCall {
    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).cloned()
    }

    fn execute(
        &mut self,
        app: &str,
        arg: &str,
    ) -> impl Future<Output = Result<Event, EslError>> + Send {
        self.executed.push((app.to_owned(), arg.to_owned()));
        let mut headers = Headers::default();
        let result = if self.hung_up {
            Err(EslError::Hangup)
        } else {
            match app {
                "play_and_get_digits" => {
                    let var = arg.split(' ').nth(7).unwrap().to_owned();
                    let digits = self.digits.pop_front().unwrap_or_default();
                    headers.push(format!("variable_{var}"), digits.to_owned());
                }
                "record" => {
                    let path = arg.split(' ').next().unwrap();
                    write_wav(Path::new(path), self.record_secs.pop_front().unwrap_or(0));
                }
                _ => {}
            }
            Ok(Event {
                headers,
                body: None,
            })
        };
        std::future::ready(result)
    }

    fn is_hung_up(&self) -> bool {
        self.hung_up
    }
}

fn call(app: &str, ext: Option<Uuid>) -> FakeCall {
    let mut vars = HashMap::from([
        ("talkops_tenant_id".to_owned(), T.to_string()),
        ("talkops_app".to_owned(), app.to_owned()),
        ("talkops_caller_number".to_owned(), "+4930123456".to_owned()),
        ("talkops_caller_name".to_owned(), "Anna".to_owned()),
    ]);
    if let Some(ext) = ext {
        vars.insert("talkops_vm_extension_id".into(), ext.to_string());
    }
    FakeCall {
        vars,
        ..Default::default()
    }
}

async fn setup(pool: &PgPool) -> (VmContext, Uuid, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("talkops-vm-{}", Uuid::new_v4()));
    let ctx = VmContext {
        db: pool.clone(),
        media: Arc::new(MediaPaths {
            voicemail: dir.join("voicemail"),
            sounds: dir.join("sounds"),
        }),
        telephony: Telephony::default(),
    };
    let ext = extensions::create(
        pool,
        T,
        &ExtensionInput {
            number: "20".into(),
            display_name: "Office".into(),
            user_id: None,
            outbound_number_id: None,
            hide_caller_id: false,
            ring_timeout_secs: 30,
            enabled: true,
            dnd: false,
            forward_all: None,
        },
        &[],
    )
    .await
    .unwrap();
    voicemail::update_box(
        pool,
        T,
        ext.id,
        &VoicemailBoxInput {
            enabled: true,
            pin: Some("1234".into()),
            email_notify: false,
            attach_audio: true,
            language: None,
            greeting: "default".into(),
            greeting_text: String::new(),
            max_message_secs: 60,
        },
        "de",
    )
    .await
    .unwrap();
    // One rendered prompt, to check prompts are found on disk.
    let prompt =
        talkops_core::prompts::path(&ctx.media.sounds, "vm_greeting_default", "de").unwrap();
    std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
    write_wav(&prompt, 1);
    (ctx, ext.id, dir)
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn deposit_stores_messages(pool: PgPool) {
    let (ctx, ext, dir) = setup(&pool).await;

    let mut c = call("vm_deposit", Some(ext));
    c.record_secs.push_back(4);
    run(&mut c, &ctx, "vm_deposit").await.unwrap();
    assert!(
        c.executed
            .iter()
            .any(|(a, d)| a == "playback" && d.contains("vm_greeting_default")),
        "{:?}",
        c.executed
    );
    let msgs = voicemail::list_messages(&pool, T, ext).await.unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].duration_secs, 4);
    assert_eq!(msgs[0].caller_number, "+4930123456");
    assert_eq!(msgs[0].caller_name, "Anna");
    assert!(ctx.path(&msgs[0].file).is_file());

    // Hang-up during the greeting: nothing is stored.
    let mut c = call("vm_deposit", Some(ext));
    c.record_secs.push_back(0);
    run(&mut c, &ctx, "vm_deposit").await.unwrap();
    assert_eq!(voicemail::counts(&pool, ext).await.unwrap(), (1, 0));

    // Disabled box: the call is not answered.
    let vbox = voicemail::get_box(&pool, T, ext).await.unwrap();
    voicemail::update_box(
        &pool,
        T,
        ext,
        &VoicemailBoxInput {
            enabled: false,
            pin: None,
            email_notify: false,
            attach_audio: true,
            language: None,
            greeting: vbox.greeting,
            greeting_text: vbox.greeting_text,
            max_message_secs: 60,
        },
        "de",
    )
    .await
    .unwrap();
    let mut c = call("vm_deposit", Some(ext));
    run(&mut c, &ctx, "vm_deposit").await.unwrap();
    assert!(c.executed.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn listen_delete_and_save(pool: PgPool) {
    let (ctx, ext, dir) = setup(&pool).await;
    for secs in [3, 5] {
        let mut c = call("vm_deposit", Some(ext));
        c.record_secs.push_back(secs);
        run(&mut c, &ctx, "vm_deposit").await.unwrap();
    }
    let first = voicemail::list_messages(&pool, T, ext).await.unwrap()[0].clone();

    // 1 = listen; first message: 7 = delete; second: 9 = save; * = exit.
    let mut c = call("vm_check", Some(ext));
    c.digits.extend(["1", "7", "9", "*"]);
    run(&mut c, &ctx, "vm_check").await.unwrap();
    assert_eq!(voicemail::counts(&pool, ext).await.unwrap(), (0, 1));
    assert!(!ctx.path(&first.file).exists());
    // The second message was played.
    let second = voicemail::list_messages(&pool, T, ext).await.unwrap()[0].clone();
    assert!(
        c.executed
            .iter()
            .any(|(a, d)| a == "playback" && d.contains(&second.id.to_string()))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn login_with_pin_and_record_greeting(pool: PgPool) {
    let (ctx, ext, dir) = setup(&pool).await;

    // Wrong PIN, then the right one; 5 = record greeting; * = exit.
    let mut c = call("vm_login", None);
    c.digits.extend(["20", "0000", "20", "1234", "5", "*"]);
    c.record_secs.push_back(2);
    run(&mut c, &ctx, "vm_login").await.unwrap();
    let vbox = voicemail::get_box(&pool, T, ext).await.unwrap();
    assert_eq!(vbox.greeting, "recorded");
    assert!(vbox.uses_custom_greeting());
    assert!(ctx.path(&voicemail::greeting_file(T, ext)).is_file());

    // The recorded greeting is played to callers.
    let mut c = call("vm_deposit", Some(ext));
    c.record_secs.push_back(2);
    run(&mut c, &ctx, "vm_deposit").await.unwrap();
    assert!(
        c.executed
            .iter()
            .any(|(a, d)| a == "playback" && d.ends_with("greeting.wav"))
    );

    // Three failed logins end the call.
    let mut c = call("vm_login", None);
    c.digits.extend(["20", "1", "99", "1234", ""]);
    run(&mut c, &ctx, "vm_login").await.unwrap();
    assert_eq!(
        c.executed
            .iter()
            .filter(|(a, _)| a == "play_and_get_digits")
            .count(),
        5
    );
    std::fs::remove_dir_all(dir).unwrap();
}

mod menus {
    use super::*;
    use crate::menu::{Outcome, run as run_menu};
    use talkops_core::ivr::{self, IvrMenuInput, MenuOption};
    use talkops_core::trunks::NumberDestination;

    async fn menu(pool: &PgPool, ext: Uuid, direct: bool, timeout: bool) -> Uuid {
        ivr::save(
            pool,
            T,
            None,
            &IvrMenuInput {
                number: None,
                name: "Main".into(),
                language: None,
                greeting: "tts".into(),
                greeting_text: "Willkommen".into(),
                timeout_secs: 4,
                max_tries: 2,
                direct_dial: direct,
                options: vec![MenuOption {
                    digit: "1".into(),
                    kind: NumberDestination::Extension,
                    id: Some(ext),
                }],
                timeout_type: if timeout {
                    NumberDestination::Voicemail
                } else {
                    NumberDestination::None
                },
                timeout_id: timeout.then_some(ext),
            },
            &[],
            "de",
        )
        .await
        .unwrap()
        .id
    }

    fn transfers(c: &FakeCall) -> Vec<String> {
        c.executed
            .iter()
            .filter(|(a, _)| a == "transfer")
            .map(|(_, d)| d.clone())
            .collect()
    }

    #[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
    async fn choices_direct_dial_and_timeout(pool: PgPool) {
        let (ctx, ext, dir) = setup(&pool).await;

        // Option 1 → extension.
        let id = menu(&pool, ext, false, false).await;
        let mut c = call("ivr", None);
        c.digits.push_back("1");
        assert_eq!(
            run_menu(&mut c, &ctx, T, id).await.unwrap(),
            Outcome::Transferred
        );
        assert_eq!(transfers(&c), [format!("dest:extension:{ext} XML talkops")]);
        let pgd = c
            .executed
            .iter()
            .find(|(a, _)| a == "play_and_get_digits")
            .unwrap();
        assert!(pgd.1.starts_with("1 1 2 4000 none "), "{}", pgd.1);
        assert!(pgd.1.contains("^[1]$"));

        // No input, no timeout destination: goodbye, caller is hung up.
        let mut c = call("ivr", None);
        assert_eq!(run_menu(&mut c, &ctx, T, id).await.unwrap(), Outcome::Done);
        assert!(transfers(&c).is_empty());

        // Direct dial of an existing number; unknown numbers time out.
        let id = menu(&pool, ext, true, true).await;
        let mut c = call("ivr", None);
        c.digits.push_back("20");
        assert_eq!(
            run_menu(&mut c, &ctx, T, id).await.unwrap(),
            Outcome::Transferred
        );
        assert_eq!(transfers(&c), ["dial:20 XML talkops"]);
        let mut c = call("ivr", None);
        c.digits.push_back("99");
        run_menu(&mut c, &ctx, T, id).await.unwrap();
        assert_eq!(transfers(&c), [format!("dest:voicemail:{ext} XML talkops")]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
