//! Voicemail flows against a scripted call and a real database.

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::path::Path;

use sqlx::PgPool;
use talkops_core::extensions::{self, ExtensionInput};
use talkops_core::tenant::TenantId;
use talkops_core::time_conditions;
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

    fn release(&mut self) -> impl Future<Output = ()> + Send {
        self.hung_up = true;
        std::future::ready(())
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
        secrets: talkops_core::crypto::SecretBox::from_hex(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        )
        .unwrap(),
        media: Arc::new(MediaPaths {
            voicemail: dir.join("voicemail"),
            sounds: dir.join("sounds"),
            recordings: dir.join("recordings"),
            snapshots: dir.join("snapshots"),
        }),
        telephony: Telephony::default(),
    };
    let ext = extensions::create(
        pool,
        T,
        &ExtensionInput {
            number: "200".into(),
            display_name: "Office".into(),
            user_id: None,
            outbound_number_id: None,
            hide_caller_id: false,
            ring_timeout_secs: 30,
            enabled: true,
            dnd: false,
            forward_all: None,
            record_calls: "inherit".into(),
            video_enabled: false,
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
            ring_timeout_secs: None,
            greeting_clip_id: None,
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

    // A clip as greeting, or none at all.
    let clip = Uuid::new_v4();
    let clip_path = ctx
        .media
        .sounds
        .join(talkops_core::audio::clip_file(T, clip));
    std::fs::create_dir_all(clip_path.parent().unwrap()).unwrap();
    write_wav(&clip_path, 1);
    talkops_core::audio::create_file(&pool, T, None, clip, "upload", 1000)
        .await
        .unwrap();
    for (greeting, clip_id, expect) in [
        ("clip", Some(clip), Some(clip.to_string())),
        ("none", None, None),
    ] {
        voicemail::update_box(
            &pool,
            T,
            ext,
            &VoicemailBoxInput {
                enabled: true,
                pin: None,
                email_notify: false,
                attach_audio: true,
                language: None,
                greeting: greeting.into(),
                greeting_text: String::new(),
                max_message_secs: 60,
                ring_timeout_secs: None,
                greeting_clip_id: clip_id,
            },
            "de",
        )
        .await
        .unwrap();
        let mut c = call("vm_deposit", Some(ext));
        c.record_secs.push_back(0);
        run(&mut c, &ctx, "vm_deposit").await.unwrap();
        let played: Vec<_> = c
            .executed
            .iter()
            .filter(|(a, d)| a == "playback" && !d.starts_with("silence") && !d.contains("tone"))
            .map(|(_, d)| d.clone())
            .collect();
        assert!(
            !played.iter().any(|d| d.contains("vm_greeting_default")),
            "{greeting}: {played:?}"
        );
        match expect {
            Some(id) => assert!(played.iter().any(|d| d.contains(&id)), "{played:?}"),
            None => assert!(!played.iter().any(|d| d.contains("clips/")), "{played:?}"),
        }
    }

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
            ring_timeout_secs: None,
            greeting_clip_id: None,
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
    c.digits.extend(["200", "0000", "200", "1234", "5", "*"]);
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
    c.digits.extend(["200", "1", "99", "1234", ""]);
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

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn queue_voicemail_for_recipients(pool: PgPool) {
    let (ctx, ext, dir) = setup(&pool).await;
    let input: talkops_core::queues::QueueInput = serde_json::from_value(serde_json::json!({
        "name": "Support", "voicemail_recipients": [ext]
    }))
    .unwrap();
    let q = talkops_core::queues::save(&pool, T, None, &input, &[])
        .await
        .unwrap();
    let mut c = call("queue_vm", None);
    c.vars.insert("talkops_queue_id".into(), q.id.to_string());
    c.record_secs.push_back(2);
    queue_voicemail(&mut c, &ctx).await.unwrap();
    assert!(
        c.executed
            .iter()
            .any(|(a, d)| a == "playback" && d.contains("vm_greeting_default"))
    );
    let msgs = voicemail::list_messages(&pool, T, ext).await.unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].duration_secs, 2);
    std::fs::remove_dir_all(dir).unwrap();
}

mod attendants {
    use super::*;
    use crate::attendant::{Outcome, run as run_attendant};
    use serde_json::json;
    use talkops_core::attendant::{self, AttendantInput};

    async fn save(pool: &PgPool, flow: serde_json::Value) -> Uuid {
        attendant::save(
            pool,
            T,
            None,
            &AttendantInput {
                number: None,
                name: "Main".into(),
                language: None,
                flow: serde_json::from_value(flow).unwrap(),
            },
            &[],
        )
        .await
        .unwrap()
        .id
    }

    async fn menu(pool: &PgPool, ext: Uuid, direct: bool, timeout: bool) -> Uuid {
        let timeout = timeout.then(|| {
            json!({"type": "transfer", "id": "t", "destination_type": "voicemail",
                   "destination_id": ext})
        });
        save(
            pool,
            json!({"type": "menu", "id": "start", "clip_id": null, "timeout_secs": 4,
                   "max_tries": 2, "direct_dial": direct,
                   "options": [{"digit": "1", "next": {"type": "transfer", "id": "one",
                       "destination_type": "extension", "destination_id": ext}}],
                   "timeout": timeout}),
        )
        .await
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
            run_attendant(&mut c, &ctx, T, id, None).await.unwrap(),
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

        // No input, nothing after the menu: goodbye, caller is hung up.
        let mut c = call("ivr", None);
        assert_eq!(
            run_attendant(&mut c, &ctx, T, id, None).await.unwrap(),
            Outcome::Done
        );
        assert!(transfers(&c).is_empty());

        // Direct dial of an existing number; unknown numbers time out.
        let id = menu(&pool, ext, true, true).await;
        let mut c = call("ivr", None);
        c.digits.push_back("200");
        assert_eq!(
            run_attendant(&mut c, &ctx, T, id, None).await.unwrap(),
            Outcome::Transferred
        );
        assert_eq!(transfers(&c), ["dial:200 XML talkops"]);
        let mut c = call("ivr", None);
        c.digits.push_back("99");
        run_attendant(&mut c, &ctx, T, id, None).await.unwrap();
        assert_eq!(transfers(&c), [format!("dest:voicemail:{ext} XML talkops")]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
    async fn schedule_ring_voicemail_park_and_loops(pool: PgPool) {
        let (ctx, ext, dir) = setup(&pool).await;
        let other = extensions::create(
            &pool,
            T,
            &ExtensionInput {
                number: "210".into(),
                display_name: "Shop".into(),
                user_id: None,
                outbound_number_id: None,
                hide_caller_id: false,
                ring_timeout_secs: 30,
                enabled: true,
                dnd: false,
                forward_all: None,
                record_calls: "inherit".into(),
                video_enabled: false,
            },
            &[],
        )
        .await
        .unwrap()
        .id;
        let schedule = |forced: &str| time_conditions::TimeConditionInput {
            number: None,
            name: format!("Hours {forced}"),
            schedule: Default::default(),
            holiday_region: None,
            closed_dates: vec![],
            r#override: forced.into(),
            open_type: talkops_core::trunks::NumberDestination::None,
            open_id: None,
            closed_type: talkops_core::trunks::NumberDestination::None,
            closed_id: None,
        };
        let open = time_conditions::create(&pool, T, &schedule("open"), &[])
            .await
            .unwrap()
            .id;
        let closed = time_conditions::create(&pool, T, &schedule("closed"), &[])
            .await
            .unwrap()
            .id;
        let flow = |tc: Uuid| {
            json!({"type": "schedule", "id": "start", "time_condition_id": tc,
                "open": {"type": "ring", "id": "ring", "extensions": [ext, other], "ring_secs": 20,
                    "next": {"type": "voicemail", "id": "vm", "recipients": [ext, other],
                             "clip_id": null}},
                "closed": {"type": "park", "id": "park"}})
        };

        // Open: ring the phones (the dialplan does the ringing).
        let id = save(&pool, flow(open)).await;
        let mut c = call("ivr", None);
        assert_eq!(
            run_attendant(&mut c, &ctx, T, id, None).await.unwrap(),
            Outcome::Transferred
        );
        assert_eq!(transfers(&c), [format!("attendant:{id}:ring XML talkops")]);

        // Nobody answered: back at the voicemail step, for both boxes.
        let mut c = call("ivr", None);
        c.record_secs.push_back(3);
        assert_eq!(
            run_attendant(&mut c, &ctx, T, id, Some("vm"))
                .await
                .unwrap(),
            Outcome::Done
        );
        assert!(
            c.executed
                .iter()
                .any(|(a, d)| a == "playback" && d.contains("vm_greeting_default"))
        );
        for box_ext in [ext, other] {
            let msgs = voicemail::list_messages(&pool, T, box_ext).await.unwrap();
            assert_eq!(msgs.len(), 1, "one copy per recipient");
            assert_eq!(msgs[0].duration_secs, 3);
            assert!(ctx.path(&msgs[0].file).is_file());
        }

        // Closed: park; without FreeSWITCH there is no free slot.
        let id = save(&pool, flow(closed)).await;
        let mut c = call("ivr", None);
        assert_eq!(
            run_attendant(&mut c, &ctx, T, id, None).await.unwrap(),
            Outcome::Done
        );
        assert!(transfers(&c).is_empty());

        // A flow that loops without input ends.
        let id = save(
            &pool,
            json!({"type": "play", "id": "start", "clip_id": null,
                   "next": {"type": "goto", "id": "again", "target": "start"}}),
        )
        .await;
        let mut c = call("ivr", None);
        assert_eq!(
            run_attendant(&mut c, &ctx, T, id, None).await.unwrap(),
            Outcome::Done
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn custom_keys_texts_and_caller_announcement(pool: PgPool) {
    let (ctx, ext, dir) = setup(&pool).await;
    talkops_core::phones::create_contact(
        &pool,
        T,
        &talkops_core::phones::ContactInput {
            name: "Anna Müller".into(),
            company: String::new(),
            phone_work: "030 123456".into(),
            phone_mobile: String::new(),
            phone_other: String::new(),
            section_id: None,
        },
    )
    .await
    .unwrap();
    let mut config = vmc::VoicemailConfig::default();
    config.keys.listen = "2".into();
    config.keys.delete = "3".into();
    config.texts.insert(
        "de".into(),
        std::collections::BTreeMap::from([("vm_goodbye".into(), "Tschüss und bis bald.".into())]),
    );
    vmc::set(&pool, T, &config).await.unwrap();
    // The reworded prompt, rendered with the default voice.
    let goodbye = talkops_core::prompts::path_for(
        &ctx.media.sounds,
        "vm_goodbye",
        "de",
        config.voice_model("de"),
        "Tschüss und bis bald.",
    );
    std::fs::create_dir_all(goodbye.parent().unwrap()).unwrap();
    write_wav(&goodbye, 1);

    // A message from the phone book: its announcement is queued.
    let mut c = call("vm_deposit", Some(ext));
    c.record_secs.push_back(3);
    run(&mut c, &ctx, "vm_deposit").await.unwrap();
    let msg = voicemail::list_messages(&pool, T, ext).await.unwrap()[0].clone();
    let job = jobs::claim(&pool, "w", &[vmc::JOB_TTS_MESSAGE_INFO])
        .await
        .unwrap()
        .unwrap();
    let info: vmc::MessageInfoJob = serde_json::from_value(job.payload).unwrap();
    assert!(
        info.text
            .starts_with("von Anna Müller, 0 3 0, 1 2 3, 4 5 6. Empfangen am "),
        "{}",
        info.text
    );
    assert_eq!(info.file, vmc::info_file(&msg.file));
    assert_eq!(info.voice, "de_DE-thorsten-medium");
    // The worker renders it; listening plays it before the message.
    write_wav(&ctx.path(&info.file), 1);

    // 2 = listen (moved from 1); 3 = delete (moved from 7); * = exit.
    let mut c = call("vm_check", Some(ext));
    c.digits.extend(["2", "3", "*"]);
    run(&mut c, &ctx, "vm_check").await.unwrap();
    let menus: Vec<&String> = c
        .executed
        .iter()
        .filter(|(a, _)| a == "play_and_get_digits")
        .map(|(_, d)| d)
        .collect();
    assert!(menus[0].contains("^[25*]$"), "{menus:?}");
    assert!(menus[1].contains("^[139#]$"), "{menus:?}");
    assert!(
        c.executed
            .iter()
            .any(|(a, d)| a == "playback" && d.contains(&format!("{}-info.wav", msg.id))),
        "{:?}",
        c.executed
    );
    assert_eq!(voicemail::counts(&pool, ext).await.unwrap(), (0, 0));
    assert!(
        !ctx.path(&info.file).exists(),
        "announcement deleted with the message"
    );
    // The reworded goodbye was played.
    assert!(
        c.executed
            .iter()
            .any(|(a, d)| a == "playback" && d.contains(&goodbye.to_string_lossy().into_owned()))
    );
    std::fs::remove_dir_all(dir).unwrap();
}
