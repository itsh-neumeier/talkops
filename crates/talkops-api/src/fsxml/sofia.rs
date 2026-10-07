//! `sofia.conf`: the internal profile for devices and the external profile
//! carrying one gateway per enabled trunk account.

use talkops_core::crypto::SecretBox;
use talkops_core::presets::{AccountMode, FromUser, PresetCatalog, render_template};
use talkops_core::trunks::{GatewayRow, gateway_name};

use super::{CONTEXT_INTERNAL, CONTEXT_PUBLIC, SIP_DOMAIN, XmlWriter};

/// Network settings of the SIP profiles (from server configuration and tenant settings).
#[derive(Debug, Clone)]
pub struct ProfileSettings {
    /// IP the profiles bind to; empty = FreeSWITCH's detected local IPv4.
    pub sip_ip: String,
    pub internal_port: u16,
    pub external_port: u16,
    /// Public IP or `stun:host` for the external profile; empty = none.
    pub external_ip: String,
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            sip_ip: String::new(),
            internal_port: 5060,
            external_port: 5080,
            external_ip: String::new(),
        }
    }
}

/// Local WebSocket listener of the internal profile (see `TALKOPS_SIP_WS_URL`).
pub const WS_BINDING: &str = "127.0.0.1:5066";

const INTERNAL_CODECS: &str = "OPUS,G722,PCMA,PCMU,H264,VP8";
const EXTERNAL_CODECS: &str = "G722,PCMA,PCMU";

/// A gateway ready to render (credentials decrypted, preset resolved).
#[derive(Debug, Clone)]
pub struct GatewaySpec {
    pub name: String,
    pub params: Vec<(&'static str, String)>,
    pub vars: Vec<(&'static str, String)>,
}

/// Builds gateway specs; accounts with unknown presets or undecryptable
/// passwords are skipped with a warning (one broken trunk must not take down
/// all others).
pub fn gateway_specs(
    rows: &[GatewayRow],
    catalog: &PresetCatalog,
    secrets: &SecretBox,
) -> Vec<GatewaySpec> {
    rows.iter()
        .filter_map(|row| match gateway_spec(row, catalog, secrets) {
            Ok(spec) => Some(spec),
            Err(err) => {
                tracing::warn!(account = %row.account_id, error = %err, "skipping trunk account");
                None
            }
        })
        .collect()
}

fn host(addr: &str) -> &str {
    addr.split([':', ';']).next().unwrap_or(addr)
}

fn gateway_spec(
    row: &GatewayRow,
    catalog: &PresetCatalog,
    secrets: &SecretBox,
) -> Result<GatewaySpec, String> {
    let preset = catalog
        .get(&row.preset)
        .ok_or_else(|| format!("unknown preset {}", row.preset))?;
    let sip = preset.effective_sip(&row.overrides)?;
    let registrar = sip
        .registrar
        .clone()
        .filter(|r| !r.is_empty())
        .ok_or("no registrar")?;
    let password = secrets
        .decrypt(&row.password_enc)
        .map_err(|e| e.to_string())?;
    let realm = sip
        .realm
        .clone()
        .unwrap_or_else(|| host(&registrar).to_owned());
    let auth_username = if !row.auth_username.is_empty() {
        row.auth_username.clone()
    } else if !preset.credentials.auth_username_template.is_empty() {
        render_template(
            &preset.credentials.auth_username_template,
            row.first_number.as_deref(),
            &row.username,
        )
    } else {
        String::new()
    };

    let mut params: Vec<(&'static str, String)> = vec![
        ("username", row.username.clone()),
        ("password", password),
        ("realm", realm.clone()),
        (
            "proxy",
            sip.proxy.clone().unwrap_or_else(|| registrar.clone()),
        ),
        ("register-proxy", registrar.clone()),
        ("register", sip.register.to_string()),
        ("register-transport", sip.transport.as_str().to_owned()),
        ("expire-seconds", sip.expire_seconds.to_string()),
        ("retry-seconds", "30".to_owned()),
        ("from-user", row.username.clone()),
        ("from-domain", sip.from_domain.clone().unwrap_or(realm)),
        (
            "caller-id-in-from",
            (sip.from_user == FromUser::Number).to_string(),
        ),
    ];
    if !auth_username.is_empty() {
        params.push(("auth-username", auth_username));
    }
    if let Some(proxy) = &sip.outbound_proxy {
        params.push(("outbound-proxy", proxy.clone()));
    }
    if let Some(ping) = sip.ping_seconds {
        params.push(("ping", ping.to_string()));
    }
    // Per-number registrations: the provider sends calls for this number to
    // our contact, whose user part identifies the number.
    if preset.credentials.mode == AccountMode::PerNumber {
        if let Some(number) = &row.first_number {
            params.push(("extension", number.trim_start_matches('+').to_owned()));
            params.push(("extension-in-contact", "true".to_owned()));
        }
    }
    let vars = vec![
        ("talkops_tenant_id", row.tenant_id.to_string()),
        ("talkops_trunk_id", row.trunk_id.to_string()),
        ("talkops_account_id", row.account_id.to_string()),
    ];
    Ok(GatewaySpec {
        name: gateway_name(row.account_id),
        params,
        vars,
    })
}

/// Renders the complete `sofia.conf` response.
pub fn render(settings: &ProfileSettings, gateways: &[GatewaySpec]) -> String {
    let ip = if settings.sip_ip.is_empty() {
        "$${local_ip_v4}".to_owned()
    } else {
        settings.sip_ip.clone()
    };
    let mut w = XmlWriter::document();
    w.open("section", &[("name", "configuration")]);
    w.open(
        "configuration",
        &[
            ("name", "sofia.conf"),
            ("description", "Generated by TalkOps"),
        ],
    );

    w.open("global_settings", &[]);
    w.param("log-level", "0");
    w.param("debug-presence", "0");
    w.close("global_settings");

    w.open("profiles", &[]);

    // Devices (desk phones, DECT, softphones, door stations).
    w.open("profile", &[("name", "internal")]);
    w.open("settings", &[]);
    w.param("context", CONTEXT_INTERNAL);
    w.param("dialplan", "XML");
    w.raw_param("sip-ip", &ip);
    w.raw_param("rtp-ip", &ip);
    w.param("sip-port", &settings.internal_port.to_string());
    w.param("auth-calls", "true");
    w.param("challenge-realm", "auto_from");
    w.param("force-register-domain", SIP_DOMAIN);
    w.param("force-register-db-domain", SIP_DOMAIN);
    w.param("force-subscription-domain", SIP_DOMAIN);
    w.param("inbound-reg-force-matching-username", "true");
    w.param("apply-nat-acl", "nat.auto");
    w.param("local-network-acl", "localnet.auto");
    w.param("manage-presence", "true");
    w.param("inbound-codec-prefs", INTERNAL_CODECS);
    w.param("outbound-codec-prefs", INTERNAL_CODECS);
    // Negotiate the caller's codec only after the callee answered, so a
    // browser (OPUS/VP8) can call a phone without OPUS and both sides end
    // up on a common codec without transcoding where possible.
    w.param("inbound-late-negotiation", "true");
    w.param("dtmf-type", "rfc2833");
    w.param("rfc2833-pt", "101");
    w.param("nonce-ttl", "60");
    w.param("rtp-timeout-sec", "300");
    w.param("rtp-hold-timeout-sec", "1800");
    w.param("user-agent-string", "TalkOps");
    // SIP over WebSocket for the browser softphone; only TalkOps (same host)
    // connects here and relays it to logged-in users.
    w.param("ws-binding", WS_BINDING);
    w.close("settings");
    w.close("profile");

    // Trunks.
    w.open("profile", &[("name", "external")]);
    w.open("gateways", &[]);
    for gw in gateways {
        w.open("gateway", &[("name", &gw.name)]);
        for (k, v) in &gw.params {
            w.param(k, v);
        }
        w.open("variables", &[]);
        for (k, v) in &gw.vars {
            w.empty(
                "variable",
                &[("name", k), ("value", v), ("direction", "inbound")],
            );
        }
        w.close("variables");
        w.close("gateway");
    }
    w.close("gateways");
    w.open("settings", &[]);
    w.param("context", CONTEXT_PUBLIC);
    w.param("dialplan", "XML");
    w.raw_param("sip-ip", &ip);
    w.raw_param("rtp-ip", &ip);
    w.param("sip-port", &settings.external_port.to_string());
    w.param("auth-calls", "false");
    if !settings.external_ip.is_empty() {
        w.param("ext-sip-ip", &settings.external_ip);
        w.param("ext-rtp-ip", &settings.external_ip);
    }
    w.param("inbound-codec-prefs", EXTERNAL_CODECS);
    w.param("outbound-codec-prefs", EXTERNAL_CODECS);
    w.param("dtmf-type", "rfc2833");
    w.param("rfc2833-pt", "101");
    w.param("rtp-timeout-sec", "300");
    w.param("rtp-hold-timeout-sec", "1800");
    w.param("user-agent-string", "TalkOps");
    w.close("settings");
    w.close("profile");

    w.close("profiles");
    w.close("configuration");
    w.close("section");
    w.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use talkops_core::tenant::TenantId;
    use uuid::Uuid;

    const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    fn catalog() -> PresetCatalog {
        PresetCatalog::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets/trunks"))
            .unwrap()
    }

    #[test]
    fn renders_leonet_gateway() {
        let sb = SecretBox::from_hex(KEY).unwrap();
        let row = GatewayRow {
            account_id: Uuid::from_u128(1),
            tenant_id: TenantId::DEFAULT,
            trunk_id: Uuid::from_u128(2),
            preset: "leonet".into(),
            overrides: serde_json::json!({}),
            username: "leo49891234567".into(),
            auth_username: String::new(),
            password_enc: sb.encrypt("s3cr\"et").unwrap(),
            first_number: Some("+49891234567".into()),
        };
        let broken = GatewayRow {
            preset: "nope".into(),
            ..row.clone()
        };
        let specs = gateway_specs(&[row, broken], &catalog(), &sb);
        assert_eq!(specs.len(), 1);
        let xml = render(&ProfileSettings::default(), &specs);
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let gw = doc
            .descendants()
            .find(|n| n.has_tag_name("gateway"))
            .unwrap();
        assert_eq!(
            gw.attribute("name"),
            Some("gw-00000000000000000000000000000001")
        );
        let param = |name: &str| {
            gw.descendants()
                .find(|n| n.has_tag_name("param") && n.attribute("name") == Some(name))
                .and_then(|n| n.attribute("value"))
                .map(str::to_owned)
        };
        assert_eq!(param("realm").as_deref(), Some("sip.leovoice.online"));
        assert_eq!(param("password").as_deref(), Some("s3cr\"et"));
        assert_eq!(param("extension").as_deref(), Some("49891234567"));
        assert_eq!(param("ping").as_deref(), Some("30"));
        assert!(xml.contains("value=\"$${local_ip_v4}\""));
        assert!(!xml.contains("ext-rtp-ip"));
    }
}
