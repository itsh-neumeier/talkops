//! Immutable upstream sources and bounded native template rendering.
use minijinja::{Environment, UndefinedBehavior};
use std::collections::BTreeMap;

include!(concat!(env!("OUT_DIR"), "/yealink_reference.rs"));
pub fn sources() -> &'static [(&'static str, &'static str)] {
    REFERENCE
}
pub fn source(path: &str) -> Option<&'static str> {
    REFERENCE.iter().find(|(p, _)| *p == path).map(|(_, s)| *s)
}
pub fn builtin() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("common.cfg".into(), crate::yealink::COMMON.into()),
        ("mac.cfg".into(), crate::yealink::MAC.into()),
    ])
}
pub fn template_env(
    files: &BTreeMap<String, String>,
) -> Result<Environment<'static>, minijinja::Error> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_trim_blocks(true);
    env.set_lstrip_blocks(true);
    env.set_fuel(Some(100_000));
    env.set_recursion_limit(20);
    for (name, source) in files {
        env.add_template_owned(name.clone(), source.clone())?;
    }
    Ok(env)
}
pub fn validate(files: &BTreeMap<String, String>) -> Result<(), String> {
    if files.len() != 2
        || !files.contains_key("common.cfg")
        || !files.contains_key("mac.cfg")
        || files
            .values()
            .any(|s| s.len() > 65536 || !s.starts_with("#!version:1.0.0.1\n"))
    {
        return Err(
            "provide common.cfg and mac.cfg, each at most 64 KiB with the Yealink header".into(),
        );
    }
    template_env(files).map_err(|e| e.to_string())?;
    Ok(())
}
/// Render both file kinds against representative, redacted model data before activation.
pub fn validate_for_model(
    files: &BTreeMap<String, String>,
    model: &crate::PhoneModel,
) -> Result<(), String> {
    validate(files)?;
    let environment = template_env(files).map_err(|e| e.to_string())?;
    let variables = [
        "prov",
        "admin_password",
        "locale",
        "phonebook",
        "phone",
        "model",
        "accounts",
        "unused_accounts",
        "keys",
        "server",
        "firmware_url",
        "voicemail_code",
        "settings",
        "profile_codecs",
        "ringtone",
        "wallpaper",
        "phonebooks",
        "unused_phonebooks",
        "dial_now",
        "extra_parameters",
        "range",
        "dict",
        "namespace",
    ];
    for name in files.keys() {
        let template = environment.get_template(name).map_err(|e| e.to_string())?;
        if template
            .undeclared_variables(false)
            .iter()
            .any(|name| !variables.contains(&name.as_str()))
        {
            return Err("template uses an unsupported variable".into());
        }
    }
    if model.vendor != "yealink" {
        return Err("unsupported template vendor".into());
    }
    let prov = crate::yealink::Provisioning {
        base_url: "https://pbx.invalid/provisioning".into(),
        username: "REDACTED".into(),
        password: "REDACTED".into(),
    };
    let common = crate::yealink::render_common_with(
        &prov,
        "REDACTED",
        &crate::yealink::Locale::new("Europe/Berlin", "en"),
        "en",
        files,
    )
    .map_err(|e| e.to_string())?;
    for account_count in [0, 1, model.accounts] {
        let setup = crate::yealink::PhoneSetup {
            name: "Preview".into(),
            mac: "805ec0000000".into(),
            model,
            accounts: (1..=account_count)
                .map(|index| crate::yealink::Account {
                    index,
                    label: "Account".into(),
                    display_name: "Preview".into(),
                    username: "REDACTED".into(),
                    password: "REDACTED".into(),
                    codecs: crate::yealink::AccountCodecs::Profiles,
                })
                .collect(),
            keys: vec![],
            sip_host: "pbx.invalid".into(),
            sip_port: 5060,
            firmware_url: None,
            voicemail_code: "*97".into(),
            ringtone: None,
            wallpaper: None,
            phonebooks: vec![],
            dial_now: vec![],
            settings: vec![],
            provisioning: Default::default(),
            asset_base: prov.authenticated_base(),
        };
        let device = crate::adapter::render(&setup, files)?;
        if device.len() > 262144 || common.len() > 262144 {
            return Err("template output exceeds 256 KiB".into());
        }
    }
    Ok(())
}
