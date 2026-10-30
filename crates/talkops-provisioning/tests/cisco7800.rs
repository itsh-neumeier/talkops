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
    render_with(setup, zone, language, "")
}
fn render_with(
    setup: &PhoneSetup<'_>,
    zone: &str,
    language: &str,
    directory_url: &str,
) -> Result<String, String> {
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
            directory_url,
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
        assert_eq!(
            catalog
                .get(&format!("cisco-{number}-enterprise"))
                .unwrap()
                .accounts,
            1
        );
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

#[test]
fn enterprise_profiles_are_separate_single_identity_tcp_bundles() {
    let catalog = catalog();
    for number in ["7811", "7821", "7841", "7861"] {
        let model = catalog.get(&format!("cisco-{number}-enterprise")).unwrap();
        assert_eq!(model.accounts, 1);
        let profile = vendors::profile(&model.id).unwrap();
        assert_eq!(profile.device_validation, "not_tested");
        assert_eq!(profile.artifacts.len(), 2);
        assert_eq!(
            vendors::delivery_filename(profile, &profile.artifacts[0], "805ec0010203"),
            "SEP805EC0010203.cnf.xml"
        );
        let mut phone = setup(model);
        phone.accounts = vec![account(1)];
        let body = render(&phone, "Europe/Berlin", "en").unwrap();
        let doc = roxmltree::Document::parse(&body).unwrap();
        assert_eq!(doc.root_element().tag_name().name(), "device");
        for (name, expected) in [
            ("name", "time.invalid"),
            ("deviceProtocol", "SIP"),
            ("processNodeName", "pbx.invalid"),
            ("sipPort", "5077"),
            ("transportLayerProtocol", "1"),
            ("authName", "user-1"),
            ("authPassword", "secret-1&<unique>"),
            ("messagesNumber", "*97"),
            ("dialTemplate", "dialplan.xml"),
            // Without these the 14.2(1) firmware reports "Error Verifying Config Info".
            ("ethernetPhonePort", "2000"),
            ("securedSipPort", "5061"),
            ("remoteCcEnable", "true"),
            ("remotePartyID", "true"),
            ("webAccess", "0"),
        ] {
            assert_eq!(value(&doc, name), expected);
        }
        assert!(body.starts_with("<?xml"));
        // The phone ignores the call manager when these tags touch (device test 2026-10-30).
        for pair in [
            "<callManagerGroup><",
            "<members><",
            "\"0\"><",
            "</callManager><",
        ] {
            assert!(!body.contains(pair), "{pair} needs whitespace");
        }
        assert!(value(&doc, "versionStamp").starts_with("talkops-"));
        assert!(
            !body.contains("directoryURL"),
            "no address without endpoint"
        );
        assert!(doc.descendants().any(|n| n.has_tag_name("commonProfile")));
        assert!(!body.contains("loadInformation"));
        assert!(!body.contains("prov-secret"));
        assert!(vendors::artifact(profile, "SEP805EC0090909.cnf.xml", "805ec0010203").is_none());
        assert!(
            vendors::artifact(profile, "SEP805EC0010203.cnf.xml.sgn", "805ec0010203").is_none()
        );
        assert!(vendors::artifact(profile, "ITLFile.tlv", "805ec0010203").is_none());
        let mut fields = parameters::schema(model).unwrap();
        assert!(fields.iter_mut().all(|f| f.managed && !f.copyable));
        let mut overrides = keys::Overrides {
            account_slots: Some(2),
            ..Default::default()
        };
        assert!(keys::effective_model(model, &overrides).is_err());
        for bad in ["0.0.0.0", "224.0.0.1", "255.255.255.255", "::1", "../other"] {
            overrides.account_slots = None;
            overrides
                .vendor_settings
                .insert("cisco_enterprise_tftp_ip".into(), bad.into());
            assert!(keys::validate(model, &overrides).is_err());
        }
        phone.accounts.push(account(2));
        assert!(render(&phone, "UTC", "en").is_err());
        let mut files = vendors::builtin(model).unwrap();
        let primary = profile.primary.as_ref().unwrap();
        let source = files.get_mut(primary).unwrap();
        *source = source.replace("dialplan.xml", "other.xml");
        assert!(vendors::validate_for_model(&files, model).is_err());
    }
}

#[test]
fn enterprise_version_stamp_follows_content_and_directory_url_is_escaped() {
    let catalog = catalog();
    let model = catalog.get("cisco-7861-enterprise").unwrap();
    let mut phone = setup(model);
    phone.accounts = vec![account(1)];
    let stamp = |body: &str| {
        let doc = roxmltree::Document::parse(body).unwrap();
        value(&doc, "versionStamp").to_owned()
    };
    let first = render(&phone, "Europe/Berlin", "en").unwrap();
    assert_eq!(
        render(&phone, "Europe/Berlin", "en").unwrap(),
        first,
        "stable"
    );
    assert_eq!(stamp(&first).len(), "talkops-".len() + 32);
    // Hashing a stamped file again keeps the stamp (the API hashes after overrides).
    assert_eq!(vendors::cisco_version_stamp(&first), first);
    phone.accounts[0].password = "changed".into();
    let changed = render(&phone, "Europe/Berlin", "en").unwrap();
    assert_ne!(stamp(&changed), stamp(&first), "content change, new stamp");

    let url = "http://pbx.invalid/provisioning/cisco/directory/id/token?a=1&b=<2>";
    let body = render_with(&phone, "Europe/Berlin", "en", url).unwrap();
    let doc = roxmltree::Document::parse(&body).unwrap();
    let directory = doc
        .descendants()
        .find(|n| n.has_tag_name("directoryURL"))
        .unwrap();
    assert_eq!(directory.parent().unwrap().tag_name().name(), "device");
    assert_eq!(directory.text(), Some(url));
    assert!(body.contains(
        "<directoryURL>http://pbx.invalid/provisioning/cisco/directory/id/token?a=1&amp;b=&lt;2&gt;</directoryURL>"
    ));
    assert_ne!(stamp(&body), stamp(&changed));
}

#[test]
fn cisco_ring_list_limits_names_and_entries() {
    assert_eq!(
        vendors::cisco_ring_filename("0123abcd-ef01-2345-6789-abcdef012345"),
        "talkops-0123abcd.rwb"
    );
    assert!(vendors::cisco_ring_filename("0123ABCD").len() <= 25);
    let rings: Vec<(String, String)> = (0..60)
        .map(|i| {
            (
                format!("Ring & <{i}> with a very long display name"),
                format!("talkops-{i:08x}.rwb"),
            )
        })
        .collect();
    let xml = vendors::cisco_ringlist(&rings);
    let doc = roxmltree::Document::parse(&xml).unwrap();
    assert_eq!(doc.root_element().tag_name().name(), "CiscoIPPhoneRingList");
    let names: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("DisplayName"))
        .map(|n| n.text().unwrap().to_owned())
        .collect();
    assert_eq!(names.len(), vendors::CISCO_RING_LIMIT);
    assert_eq!(names[0], "Ring & <0> with a very lo");
    assert!(names.iter().all(|n| n.chars().count() <= 25));
}
