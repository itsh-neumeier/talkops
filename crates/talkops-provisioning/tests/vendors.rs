use talkops_provisioning::{PhoneModel, vendors};

#[test]
fn every_source_profile_renders_its_complete_artifact_manifest() {
    let mut failures = Vec::new();
    let mut coverage = Vec::new();
    for profile in vendors::profiles() {
        if profile.adaptation != "adapted" {
            assert_eq!(profile.reason.as_deref(), Some("no_source_template"));
            continue;
        }
        let model = PhoneModel {
            id: profile.id.clone(),
            name: profile.source_model.clone(),
            vendor: profile.vendor.clone(),
            family: serde_json::from_value(serde_json::json!(profile.family)).unwrap(),
            accounts: profile.accounts,
            line_keys: 0,
            layout: None,
            video: false,
            ringtone_max_kb: 0,
            wallpaper: false,
        };
        match vendors::builtin(&model)
            .and_then(|files| vendors::validate_for_model(&files, &model))
            .and_then(|_| {
                talkops_provisioning::parameters::schema(&model).map(|fields| {
                    assert!(fields.iter().all(|f| !f.managed || f.default.is_none()));
                    coverage.push(serde_json::json!({"profile":profile.id,"vendor":profile.vendor,"source_model":profile.source_model,"accounts":model.accounts,"artifacts":profile.artifacts.len(),"fields":fields.len(),"editable_fields":fields.iter().filter(|f|!f.managed).count(),"fixture_validation":"passed","device_validation":"not_tested"}));
                })
            }) {
            Ok(()) => {}
            Err(error) => failures.push(error),
        }
    }
    if let Ok(path) = std::env::var("TALKOPS_PROVISIONING_FAILURE_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&failures).unwrap()).unwrap();
    }
    if failures.is_empty() {
        if let Ok(path) = std::env::var("TALKOPS_PROVISIONING_COVERAGE_REPORT") {
            std::fs::write(path,serde_json::to_vec_pretty(&serde_json::json!({"source_revision":vendors::profiles()[0].revision,"profiles":coverage})).unwrap()).unwrap();
        }
    }
    assert!(
        failures.is_empty(),
        "{} profile failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn artifact_resolution_requires_a_declared_file_and_rejects_path_traversal() {
    for profile in vendors::profiles()
        .iter()
        .filter(|p| p.adaptation == "adapted")
    {
        for file in &profile.artifacts {
            let name = vendors::delivery_filename(profile, file, "805ec0010203");
            assert!(vendors::valid_path(&name), "{}: {name}", profile.id);
            assert_eq!(
                vendors::artifact(profile, &name, "805ec0010203").map(|a| &a.name),
                Some(&file.name),
                "{}: {name}",
                profile.id
            );
        }
    }
    let profile = vendors::profile("cisco-7940").unwrap();
    assert!(vendors::artifact(profile, "SIP805ec0010203.cnf", "805ec0010203").is_some());
    for path in [
        "../SIPDefault.cnf",
        "/SIPDefault.cnf",
        "SIPDefault.cnf?password=x",
        "SIP805ec0090909.cnf",
        "firmware.bin",
        "dir//file",
    ] {
        assert!(
            vendors::artifact(profile, path, "805ec0010203").is_none(),
            "{path}"
        );
    }
}

#[test]
fn source_contact_operations_bound_unicode_and_preserve_multiple_numbers() {
    let environment = vendors::environment(&Default::default(), false).unwrap();
    assert_eq!(
        environment
            .render_str(
                "{{ name | truncate(6) }}",
                serde_json::json!({"name":"ÄÖÜßlang"})
            )
            .unwrap(),
        "ÄÖÜ..."
    );
    assert_eq!(environment.render_str("{{ array_merge(left,right) | json }}",serde_json::json!({"left":[{"phone_number":"101"}],"right":[{"phone_number":"102"}]})).unwrap(),"[{\"phone_number\":\"101\"},{\"phone_number\":\"102\"}]");
    assert!(
        environment
            .render_str(
                "{{ array_merge(left,right) }}",
                serde_json::json!({"left":vec![0;1024],"right":[1]})
            )
            .is_err()
    );
}

#[test]
fn device_lock_pins_are_secrets_and_do_not_copy() {
    let profile = vendors::profile("yealink-t4x").unwrap();
    let field = profile
        .settings
        .iter()
        .find(|field| field.key == "yealink_lock_pin")
        .unwrap();
    assert_eq!(field.kind, "secret");
    assert!(!field.copyable);
}
