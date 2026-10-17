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
