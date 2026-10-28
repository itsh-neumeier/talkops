use std::{collections::BTreeMap, path::Path};
use talkops_provisioning::{
    PhoneCatalog, PhoneModel, keys,
    model::{Account, PhoneSetup, Provisioning},
    parameters, vendors,
};

fn catalog() -> PhoneCatalog {
    PhoneCatalog::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets/phones"))
        .unwrap()
}
fn setup(model: &PhoneModel) -> PhoneSetup<'_> {
    PhoneSetup {
        name: "Office & <Desk>".into(),
        mac: "805ec0010203".into(),
        model,
        accounts: vec![],
        keys: vec![],
        sip_host: "pbx.invalid".into(),
        sip_port: 5077,
        firmware_url: None,
        voicemail_code: "*97".into(),
        ringtone: None,
        wallpaper: None,
        phonebooks: vec![],
        dial_now: vec![],
        settings: vec![],
        provisioning: Default::default(),
        asset_base: "https://pbx.invalid/provisioning".into(),
    }
}
fn account(index: u16) -> Account {
    Account {
        index,
        label: format!("Line {index} & <label>"),
        display_name: format!("User {index} & <name>"),
        username: format!("user-{index}"),
        password: format!("secret-{index}&<unique>"),
        codecs: Default::default(),
    }
}
fn render(setup: &PhoneSetup<'_>, zone: &str, language: &str) -> Result<String, String> {
    let provisioning = Provisioning {
        base_url: setup.asset_base.clone(),
        username: "prov-user".into(),
        password: "prov-secret".into(),
    };
    vendors::render(
        setup,
        &vendors::Common {
            provisioning: &provisioning,
            admin_password: "admin&<secret>",
            timezone: zone,
            language,
            ntp_server: "time.invalid",
            contacts: &[],
        },
        None,
        &BTreeMap::new(),
    )
    .map(|r| r.0)
}
fn value<'a>(doc: &'a roxmltree::Document<'a>, name: &str) -> &'a str {
    doc.descendants()
        .find(|n| n.has_tag_name(name))
        .unwrap()
        .text()
        .unwrap_or("")
}

#[test]
fn mpp_models_bind_sparse_accounts_and_clear_all_hardware_slots() {
    let catalog = catalog();
    for (number, capacity) in [("7811", 1), ("7821", 2), ("7841", 4), ("7861", 16)] {
        let id = format!("cisco-{number}-mpp");
        let model = catalog.get(&id).unwrap();
        let profile = vendors::profile(&id).unwrap();
        assert_eq!(model.accounts, capacity);
        assert_eq!(profile.accounts, capacity);
        assert_eq!(profile.device_validation, "not_tested");
        assert_eq!(profile.artifacts.len(), 1);
        assert!(profile.key_groups.is_empty());
        let name = vendors::delivery_filename(profile, &profile.artifacts[0], "805ec0010203");
        assert_eq!(name, format!("CP-{number}-3PCC805ec0010203.cfg"));
        assert!(vendors::artifact(profile, &name, "805ec0010203").is_some());
        for bad in [
            name.replace("010203", "999999"),
            "SEP805EC0010203.cnf.xml".into(),
            "../config.xml".into(),
            "config.xml".into(),
        ] {
            assert!(vendors::artifact(profile, &bad, "805ec0010203").is_none());
        }
        let mut phone = setup(model);
        phone.accounts = vec![account(capacity)];
        let body = render(&phone, "Europe/Berlin", "de").unwrap();
        assert_eq!(body, render(&phone, "Europe/Berlin", "de").unwrap());
        let doc = roxmltree::Document::parse(&body).unwrap();
        assert_eq!(doc.root_element().tag_name().name(), "flat-profile");
        assert_eq!(value(&doc, "Admin_Password"), "admin&<secret>");
        assert_eq!(value(&doc, "Time_Zone"), "GMT+01:00");
        assert_eq!(value(&doc, "Language_Selection"), "German");
        assert_eq!(value(&doc, "Primary_NTP_Server"), "time.invalid");
        assert_eq!(value(&doc, "Voice_Mail_Number"), "*97");
        assert!(!body.contains("prov-secret"));
        assert!(!body.contains("https://")); // No undeclared file references.
        for n in 1..=capacity {
            assert_eq!(
                value(&doc, &format!("Line_Enable_{n}_")),
                if n == capacity { "Yes" } else { "No" }
            );
            assert_eq!(
                value(&doc, &format!("Register_{n}_")),
                if n == capacity { "Yes" } else { "No" }
            );
            assert_eq!(value(&doc, &format!("Proxy_{n}_")), "pbx.invalid:5077");
            assert_eq!(value(&doc, &format!("SIP_Transport_{n}_")), "UDP");
            assert_eq!(
                value(&doc, &format!("Password_{n}_")),
                if n == capacity {
                    phone.accounts[0].password.as_str()
                } else {
                    ""
                }
            );
            if capacity > 1 {
                assert_eq!(
                    value(&doc, &format!("Extension_{n}_")),
                    if n == capacity {
                        n.to_string()
                    } else {
                        "Disabled".into()
                    }
                );
            }
        }
        phone.accounts = (1..=capacity).map(account).collect();
        let full = render(&phone, "UTC", "en").unwrap();
        let doc = roxmltree::Document::parse(&full).unwrap();
        for a in &phone.accounts {
            assert_eq!(value(&doc, &format!("User_ID_{}_", a.index)), a.username);
            assert_eq!(value(&doc, &format!("Auth_ID_{}_", a.index)), a.username);
            assert_eq!(value(&doc, &format!("Password_{}_", a.index)), a.password);
            assert_eq!(
                value(&doc, &format!("Display_Name_{}_", a.index)),
                a.display_name
            );
        }
        let mut reduced = model.clone();
        reduced.accounts = 1;
        let empty = render(&setup(&reduced), "UTC", "en").unwrap();
        let doc = roxmltree::Document::parse(&empty).unwrap();
        assert_eq!(value(&doc, &format!("Line_Enable_{capacity}_")), "No");
        assert_eq!(value(&doc, "Daylight_Saving_Time_Enable"), "No");
        assert_eq!(value(&doc, "Language_Selection"), "English-US");
        phone.accounts = vec![account(capacity + 1)];
        assert!(render(&phone, "UTC", "en").is_err());
        phone.accounts = vec![account(1), account(1)];
        assert!(render(&phone, "UTC", "en").is_err());
        let overrides = keys::Overrides {
            account_slots: Some(capacity + 1),
            ..Default::default()
        };
        assert!(keys::effective_model(model, &overrides).is_err());
        assert!(catalog.get(&format!("cisco-{number}-enterprise")).is_none());
    }
}

#[test]
fn mpp_managed_fields_and_locale_limits_are_explicit() {
    let catalog = catalog();
    let model = catalog.get("cisco-7861-mpp").unwrap();
    let fields = parameters::schema(model).unwrap();
    for name in [
        "User_ID_16_",
        "Password_16_",
        "Auth_ID_16_",
        "Proxy_16_",
        "SIP_Transport_16_",
        "Extension_16_",
        "Short_Name_16_",
        "Admin_Password",
    ] {
        let field = fields
            .iter()
            .find(|f| f.parameter.contains(&format!("/{name}[")))
            .unwrap();
        assert!(field.managed, "{name}");
        assert!(!field.copyable);
        assert!(field.default.is_none());
        assert!(
            parameters::validate(
                model,
                &BTreeMap::from([(field.key.clone(), "wrong".into())])
            )
            .is_err()
        );
    }
    let dictionary = fields
        .iter()
        .find(|f| f.parameter.contains("/Dictionary_Server_Script["))
        .unwrap();
    assert!(!dictionary.managed);
    assert_eq!(dictionary.kind, "secret");
    assert!(!dictionary.copyable);
    let enterprise = keys::Overrides {
        firmware_profile: "enterprise".into(),
        ..Default::default()
    };
    assert!(keys::validate(model, &enterprise).is_err());
    let mut files = vendors::builtin(model).unwrap();
    let artifact = vendors::profile(&model.id)
        .unwrap()
        .primary
        .as_ref()
        .unwrap();
    files.get_mut(artifact).unwrap().push_str("<invalid/>");
    assert!(vendors::validate_for_model(&files, model).is_err());
    let mut files = vendors::builtin(model).unwrap();
    let source = files.get_mut(artifact).unwrap();
    *source = source.replace("</flat-profile>", "<Next>https://pbx.invalid/provisioning/devices/805ec0010203/other.xml</Next></flat-profile>");
    assert!(vendors::validate_for_model(&files, model).is_err());
    assert!(render(&setup(model), "America/New_York", "en").is_err());
    let uk = render(&setup(model), "Europe/London", "en").unwrap();
    assert!(uk.contains("start=3/-1/7/1;end=10/-1/7/2;save=1"));
}
