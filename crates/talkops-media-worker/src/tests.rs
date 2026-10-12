//! Worker tests with real database state and a deterministic Piper process.

use super::*;
use talkops_core::tenant::TenantId;
use talkops_core::{extensions, voicemail_config};

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn renders_box_speeds_and_refreshes_existing_message_information(pool: PgPool) {
    let dir = std::env::temp_dir().join(format!("talkops-worker-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let ctx = Ctx {
        pool: pool.clone(),
        piper: tts::tests::fake_piper(&dir),
        whisper: Whisper {
            bin: dir.join("unused"),
            models_dir: dir.clone(),
            model: "base".into(),
            threads: 1,
            vad: false,
        },
        secrets: None,
        voicemail_dir: dir.join("voicemail"),
        sounds_dir: dir.join("sounds"),
        recordings_dir: dir.join("recordings"),
    };
    let tenant = TenantId::DEFAULT;
    let input: extensions::ExtensionInput = serde_json::from_value(serde_json::json!({
        "number":"205", "display_name":"Test"
    }))
    .unwrap();
    let ext = extensions::create(&pool, tenant, &input, &[])
        .await
        .unwrap();
    let mut box_input: voicemail::VoicemailBoxInput = serde_json::from_value(serde_json::json!({
        "enabled":true, "speech_speed":0.7
    }))
    .unwrap();
    voicemail::update_box(&pool, tenant, ext.id, &box_input, "de")
        .await
        .unwrap();
    let msg = voicemail::create_message(
        &pool,
        tenant,
        &voicemail::NewMessage {
            id: uuid::Uuid::new_v4(),
            extension_id: ext.id,
            caller_number: "+4930123456".into(),
            caller_name: "Test Caller".into(),
            duration_secs: 3,
            call_uuid: None,
        },
    )
    .await
    .unwrap();
    let file = ctx.voicemail_dir.join(&msg.file);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, b"recording").unwrap();
    assert!(tenant_prompts(&ctx, Some(tenant)).await.unwrap() > 0);
    let config = voicemail_config::VoicemailConfig::default();
    let text = config.rendered_text("vm_main_menu", "de", 0.7).unwrap();
    let slow = talkops_core::prompts::path_for(
        &ctx.sounds_dir,
        "vm_main_menu",
        "de",
        config.voice_model("de"),
        &text,
    );
    assert!(slow.is_file());
    let old = voicemail_config::message_info_job(&pool, tenant, &msg)
        .await
        .unwrap()
        .unwrap();
    assert!(ctx.voicemail_dir.join(&old.file).is_file());
    assert_eq!(tenant_prompts(&ctx, Some(tenant)).await.unwrap(), 0);
    box_input.speech_speed = Some(1.0);
    voicemail::update_box(&pool, tenant, ext.id, &box_input, "de")
        .await
        .unwrap();
    assert!(
        tenant_prompts(&ctx, Some(tenant)).await.unwrap() > 0,
        "explicit jobs also refresh a default configuration"
    );
    let new = voicemail_config::message_info_job(&pool, tenant, &msg)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(new.file, old.file);
    assert!(ctx.voicemail_dir.join(new.file).is_file());
    assert_eq!(
        tenant_prompts(&ctx, Some(TenantId(uuid::Uuid::new_v4())))
            .await
            .unwrap(),
        0
    );
    std::fs::remove_dir_all(dir).unwrap();
}
