//! Directory: SIP users (one per device) for registration, call
//! authentication and `user/` bridging.

use talkops_core::extensions::DeviceAuth;

use super::{CONTEXT_INTERNAL, SIP_DOMAIN, XmlWriter, sanitize_value};

/// Renders one user. `password` is the decrypted SIP password.
pub fn render_user(device: &DeviceAuth, password: &str) -> String {
    let mut w = XmlWriter::document();
    w.open("section", &[("name", "directory")]);
    w.open("domain", &[("name", SIP_DOMAIN)]);
    w.open("params", &[]);
    // Bridge to every registered contact of the user (multiple registrations
    // of the same device, e.g. after IP changes, all ring).
    w.raw_param(
        "dial-string",
        "{^^:sip_invite_domain=${dialed_domain}:presence_id=${dialed_user}@${dialed_domain}}${sofia_contact(*/${dialed_user}@${dialed_domain})}",
    );
    w.close("params");
    w.open("user", &[("id", &device.sip_username)]);
    w.open("params", &[]);
    w.param("password", password);
    // Presence (BLF) is reported per extension number, not per device: busy
    // lamps watch "20", whichever of the extension's devices is in a call.
    let presence = format!("{}@{SIP_DOMAIN}", sanitize_value(&device.extension_number));
    w.raw_param(
        "dial-string",
        &format!("{{^^:sip_invite_domain=${{dialed_domain}}:presence_id={presence}}}${{sofia_contact(*/${{dialed_user}}@${{dialed_domain}})}}"),
    );
    w.close("params");
    w.open("variables", &[]);
    let vars = [
        ("user_context", CONTEXT_INTERNAL.to_owned()),
        (
            "effective_caller_id_name",
            sanitize_value(&device.display_name),
        ),
        (
            "effective_caller_id_number",
            device.extension_number.clone(),
        ),
        ("talkops_tenant_id", device.tenant_id.to_string()),
        ("talkops_extension_id", device.extension_id.to_string()),
        ("talkops_device_id", device.device_id.to_string()),
        (
            "presence_id",
            format!("{}@{SIP_DOMAIN}", sanitize_value(&device.extension_number)),
        ),
    ];
    for (name, value) in &vars {
        w.empty("variable", &[("name", name), ("value", value)]);
    }
    w.close("variables");
    w.close("user");
    w.close("domain");
    w.close("section");
    w.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkops_core::tenant::TenantId;
    use uuid::Uuid;

    #[test]
    fn renders_user_with_sanitized_name() {
        let device = DeviceAuth {
            device_id: Uuid::from_u128(1),
            tenant_id: TenantId::DEFAULT,
            sip_username: "20-1".into(),
            sip_password_enc: String::new(),
            extension_id: Uuid::from_u128(2),
            extension_number: "20".into(),
            display_name: "Büro ${evil}".into(),
        };
        let xml = render_user(&device, "pw&1");
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let user = doc.descendants().find(|n| n.has_tag_name("user")).unwrap();
        assert_eq!(user.attribute("id"), Some("20-1"));
        let var = |name: &str| {
            doc.descendants()
                .find(|n| n.has_tag_name("variable") && n.attribute("name") == Some(name))
                .and_then(|n| n.attribute("value"))
        };
        assert_eq!(var("effective_caller_id_name"), Some("Büro evil"));
        assert_eq!(var("user_context"), Some("internal"));
        let pw = doc
            .descendants()
            .find(|n| n.attribute("name") == Some("password"))
            .unwrap();
        assert_eq!(pw.attribute("value"), Some("pw&1"));
        assert!(xml.contains("${sofia_contact("));
        assert_eq!(var("presence_id"), Some("20@talkops.local"));
        assert!(
            xml.contains("presence_id=20@talkops.local}"),
            "user dial-string reports the extension"
        );
    }
}
