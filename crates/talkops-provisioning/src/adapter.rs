//! Dispatch only to implemented vendor adapters. Unknown vendors cannot emit Yealink files.
use crate::{model::PhoneSetup, yealink};
use std::collections::BTreeMap;

pub fn render(
    setup: &PhoneSetup<'_>,
    templates: &BTreeMap<String, String>,
) -> Result<String, String> {
    match setup.model.vendor.as_str() {
        "yealink" => yealink::render_phone_with(setup, templates).map_err(|e| e.to_string()),
        _ => Err(format!(
            "unsupported provisioning vendor {}",
            setup.model.vendor
        )),
    }
}
pub fn phonebook(
    vendor: &str,
    title: &str,
    entries: &[crate::phonebook::Entry],
) -> Result<String, String> {
    match vendor {
        "yealink" => Ok(crate::phonebook::render(title, entries)),
        _ => Err("unsupported phonebook vendor".into()),
    }
}
