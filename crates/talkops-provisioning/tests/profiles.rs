use std::path::Path;
use talkops_provisioning::{
    PhoneCatalog, adapter,
    keys::{self, Key, Overrides},
    library,
    yealink::{self, KeyType},
};
fn catalog() -> PhoneCatalog {
    PhoneCatalog::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets/phones"))
        .unwrap()
}
fn setup(model: &talkops_provisioning::PhoneModel) -> yealink::PhoneSetup<'_> {
    yealink::PhoneSetup {
        name: "Desk".into(),
        mac: "805ec0123456".into(),
        model,
        accounts: vec![],
        keys: vec![],
        sip_host: "pbx.lan".into(),
        sip_port: 5060,
        firmware_url: None,
        voicemail_code: "*97".into(),
        ringtone: None,
        wallpaper: None,
        phonebooks: vec![],
        dial_now: vec![],
        settings: vec![],
        provisioning: Default::default(),
        asset_base: "http://provision:REDACTED@pbx.lan/provisioning".into(),
    }
}
#[test]
fn t41p_navigation_ids_codes_and_reset_are_explicit() {
    let catalog = catalog();
    let model = catalog.get("t41p").unwrap();
    let caps = keys::capabilities(model);
    assert_eq!(
        caps.programmable,
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 13, 17, 18]
    );
    assert_eq!(caps.expansion_modules, 0);
    let mut setup = setup(model);
    setup.provisioning.keys = vec![Key {
        group: "programmable".into(),
        key: 18,
        kind: KeyType::SpeedDial,
        value: "**101".into(),
        label: "Message override".into(),
        account: 1,
        history_type: 0,
        phonebook: 0,
    }];
    let rendered = adapter::render(&setup, &Default::default()).unwrap();
    assert!(rendered.contains("programablekey.18.type = 13\n"));
    assert!(rendered.contains("programablekey.18.value = **101\n"));
    assert!(rendered.contains("programablekey.7.type = 51\n"));
    assert!(!rendered.contains("expansion_module."));
    setup.provisioning.keys.clear();
    let reset = adapter::render(&setup, &Default::default()).unwrap();
    assert!(reset.contains("programablekey.18.type = 0\n"));
    assert!(!reset.contains("**101"));
    setup.provisioning.firmware_profile = "fusionpbx-legacy".into();
    assert!(
        adapter::render(&setup, &Default::default())
            .unwrap()
            .contains("programablekey.7.type = 48\n")
    );
}
#[test]
fn rejects_unsupported_slots_injection_and_vendor_fallback() {
    let catalog = catalog();
    let model = catalog.get("t41p").unwrap();
    let mut overrides = Overrides::default();
    let mut key = Key {
        group: "programmable".into(),
        key: 11,
        kind: KeyType::Menu,
        value: String::new(),
        label: String::new(),
        account: 0,
        history_type: 0,
        phonebook: 0,
    };
    overrides.keys.push(key.clone());
    assert!(keys::validate(model, &overrides).is_err());
    key.key = 18;
    overrides.keys = vec![key.clone()];
    assert!(keys::validate(model, &overrides).is_ok());
    key.label = "bad\naccount.1.password=x".into();
    overrides.keys = vec![key];
    assert!(keys::validate(model, &overrides).is_err());
    for url in [
        "tftp://pbx/provisioning",
        "https://user:secret@pbx/provisioning",
        "https://pbx/provisioning?password=secret",
        "https://pbx/provisioning#bad",
    ] {
        assert!(keys::validate_endpoint(url, true).is_err(), "{url}");
    }
    assert!(keys::validate_endpoint("http://192.168.1.2:8095/provisioning/", true).is_ok());
    let mut unknown = model.clone();
    unknown.vendor = "other".into();
    assert!(adapter::render(&setup(&unknown), &Default::default()).is_err());
    assert!(keys::capabilities(&unknown).programmable.is_empty());
}
#[test]
fn native_templates_validate_for_every_active_model_and_reject_bad_drafts() {
    for model in catalog().all() {
        library::validate_for_model(&library::builtin(), model).unwrap();
    }
    let model = catalog();
    let model = model.get("t41p").unwrap();
    let mut files = library::builtin();
    files
        .get_mut("mac.cfg")
        .unwrap()
        .push_str("\n{{ unknown_secret }}\n");
    assert!(library::validate_for_model(&files, model).is_err());
    files.insert(
        "mac.cfg".into(),
        format!(
            "#!version:1.0.0.1\n{{% for n in range(1000) %}}{}{{% endfor %}}",
            "x".repeat(1000)
        ),
    );
    assert!(
        library::validate_for_model(&files, model).is_err(),
        "output writer must stop before allocation grows without a bound"
    );
    files.insert(
        "mac.cfg".into(),
        "#!version:1.0.0.1\n{% for n in range(1000000000) %}x{% endfor %}".into(),
    );
    assert!(library::validate_for_model(&files, model).is_err());
    files.insert(
        "mac.cfg".into(),
        "#!version:1.0.0.1\n{% include '/etc/passwd' %}".into(),
    );
    assert!(library::validate_for_model(&files, model).is_err());
}
#[test]
fn source_library_contains_the_entire_pinned_tree() {
    assert_eq!(library::sources().len(), 433);
    assert!(
        library::source("t41p/{$mac}.cfg")
            .unwrap()
            .contains("linekey.")
    );
    assert!(
        library::source("t41p/y000000000036.cfg")
            .unwrap()
            .contains("programablekey.")
    );
    assert!(library::source("../licenses.php").is_none());
}
#[test]
fn call_state_layouts_have_separate_files_and_checked_actions() {
    let c = catalog();
    let model = c.get("t41p").unwrap();
    let mut setup = setup(model);
    let actions = vec![
        "Answer".into(),
        "Forward".into(),
        "Silence".into(),
        "Reject".into(),
    ];
    setup
        .provisioning
        .softkeys
        .insert("call_in".into(), actions.clone());
    keys::validate(model, &setup.provisioning).unwrap();
    let cfg = adapter::render(&setup, &Default::default()).unwrap();
    assert!(cfg.contains("custom_softkey_call_in.url = http://provision:REDACTED@pbx.lan/provisioning/softkeys/805ec0123456/call_in.xml"));
    let allowed = &keys::capabilities(model).softkey_states["call_in"];
    let xml = keys::softkey_xml("call_in", &actions, allowed);
    assert!(xml.contains("<CallIn>"));
    assert!(xml.contains("<Default><Key Type=\"Answer\"/>"));
    setup
        .provisioning
        .softkeys
        .insert("call_in".into(), vec!["<bad>".into(); 4]);
    assert!(keys::validate(model, &setup.provisioning).is_err());
}
