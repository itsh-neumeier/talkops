//! The write path (ADR 0042): read-modify-write of one Wi-Fi broadcast.
//!
//! The only write request of the crate is the `PUT` in [`Writer::send_put`] to
//! `/proxy/network/integration/v1/sites/{site}/wifi/broadcasts/{id}`. The
//! request body is the object of a new `GET` without `id` and `metadata`,
//! with only allowlist fields changed. The passphrase and other secrets of
//! the object stay in memory for this one request.

use serde_json::Value;

use crate::allowlist::{self, Field, Slot, Snapshot};
use crate::{Broadcast, Client, INTEGRATION, UnifiError, check_id};

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error(transparent)]
    Unifi(#[from] UnifiError),
    /// The API key may read but not write (HTTP 403 on the PUT).
    #[error("the UniFi API key has no write permission")]
    Forbidden,
    /// UniFi refused the body (HTTP 400 or 422). `code` is the error code
    /// of the answer, without the message text.
    #[error("UniFi refused the change ({code})")]
    Rejected { code: String },
    /// The SSID changed since the proposal or since the stored change.
    #[error("the SSID changed in UniFi")]
    Conflict { current: Snapshot },
    #[error("TalkOps cannot write this SSID ({0})")]
    NotWritable(&'static str),
    #[error("{0}")]
    Invalid(String),
    /// UniFi accepted the PUT, but the read-back differs.
    #[error("UniFi did not apply all values")]
    Verify { before: Snapshot, after: Snapshot },
}

impl WriteError {
    /// Short class for stored rows and the audit log (no details).
    pub fn class(&self) -> &'static str {
        match self {
            WriteError::Unifi(e) => e.class(),
            WriteError::Forbidden => "forbidden",
            WriteError::Rejected { .. } => "rejected",
            WriteError::Conflict { .. } => "ssid_changed",
            WriteError::NotWritable(r) => r,
            WriteError::Invalid(_) => "invalid",
            WriteError::Verify { .. } => "verify_mismatch",
        }
    }
}

/// What TalkOps expects in UniFi before the write.
#[derive(Debug, Clone)]
pub enum Guard {
    /// [`allowlist::hash`] of the object at proposal time.
    Hash(String),
    /// These values (rollback: the stored "after" values).
    Values(Snapshot),
    /// No check (rollback after the user accepted foreign changes).
    None,
}

/// Result of a write.
#[derive(Debug, Clone)]
pub struct Applied {
    pub name: String,
    /// Values of the changed fields before the write.
    pub before: Snapshot,
    /// Values of the changed fields after the write (read back).
    pub after: Snapshot,
}

pub struct Writer<'a> {
    client: &'a Client,
}

impl Client {
    /// The write path. Callers check the ADR 0042 conditions first.
    pub fn writer(&self) -> Writer<'_> {
        Writer { client: self }
    }
}

impl Writer<'_> {
    /// Applies `changes` to the SSID `id`: GET, check, merge, PUT, GET.
    pub async fn apply(
        &self,
        site_id: &str,
        id: &str,
        changes: &[(Field, Slot)],
        guard: &Guard,
    ) -> Result<Applied, WriteError> {
        if changes.is_empty() {
            return Err(WriteError::Invalid("no field selected".into()));
        }
        let fields: Vec<Field> = changes.iter().map(|(f, _)| *f).collect();
        if (1..fields.len()).any(|i| fields[..i].contains(&fields[i])) {
            return Err(WriteError::Invalid("a field occurs twice".into()));
        }
        let current = self.client.wifi_broadcast(site_id, id).await?;
        let obj = current.object();
        let conflict = match guard {
            Guard::Hash(h) => allowlist::hash(obj) != *h,
            Guard::Values(expected) => expected.iter().any(|(k, v)| match Field::from_key(k) {
                Some(f) => !allowlist::read(obj, f).same(v),
                None => true,
            }),
            Guard::None => false,
        };
        if conflict {
            return Err(WriteError::Conflict {
                current: current.snapshot(Field::ALL),
            });
        }
        if let Some(reason) = allowlist::not_writable(obj) {
            return Err(WriteError::NotWritable(reason));
        }
        let available = allowlist::available(obj);
        for (field, slot) in changes {
            // A rollback may remove an optional field again.
            if slot.value().is_some() && !available.contains(field) {
                return Err(WriteError::Invalid(format!(
                    "{} is not available in this UniFi version",
                    field.key()
                )));
            }
            allowlist::check(*field, slot, obj).map_err(WriteError::Invalid)?;
        }
        let before = current.snapshot(&fields);
        let body = body(obj, changes)?;
        self.send_put(site_id, id, &body).await?;
        drop(body);
        let after = self.client.wifi_broadcast(site_id, id).await?;
        let applied = Applied {
            name: after.name().to_owned(),
            before,
            after: after.snapshot(&fields),
        };
        let mismatch = changes
            .iter()
            .any(|(f, slot)| !applied.after[f.key()].same(slot));
        if mismatch {
            return Err(WriteError::Verify {
                before: applied.before,
                after: applied.after,
            });
        }
        Ok(applied)
    }

    /// The only write request of the crate.
    async fn send_put(&self, site_id: &str, id: &str, body: &Value) -> Result<(), WriteError> {
        let (site, id) = (check_id(site_id)?, check_id(id)?);
        let c = self.client;
        let res = c
            .http
            .put(format!(
                "{}{INTEGRATION}/sites/{site}/wifi/broadcasts/{id}",
                c.cfg.url
            ))
            .header("X-API-KEY", &c.cfg.api_key)
            .header("Accept", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| c.send_error(e))?;
        match res.status().as_u16() {
            200..=204 => Ok(()),
            401 => Err(UnifiError::Unauthorized.into()),
            403 => Err(WriteError::Forbidden),
            404 => Err(UnifiError::NotFound.into()),
            code @ (400 | 409 | 422) => {
                let v = crate::read_json(res).await.unwrap_or_default();
                let code = v
                    .get("code")
                    .and_then(Value::as_str)
                    .filter(|c| {
                        c.len() <= 80
                            && c.bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
                    })
                    .map_or_else(|| format!("HTTP {code}"), str::to_owned);
                Err(WriteError::Rejected { code })
            }
            code => Err(UnifiError::Http(format!("HTTP status {code}")).into()),
        }
    }
}

/// The PUT body for `obj` with `changes`. Fails if anything other than an
/// allowlist field would change.
pub fn body(obj: &Value, changes: &[(Field, Slot)]) -> Result<Value, WriteError> {
    let base = allowlist::update_body(obj);
    let mut body = base.clone();
    allowlist::merge(&mut body, changes);
    if allowlist::differences(&base, &body)
        .iter()
        .any(|p| !allowlist::allowed_path(p))
    {
        return Err(WriteError::Invalid("change outside the allowlist".into()));
    }
    Ok(body)
}

/// Rollback: the stored "before" values of the changed fields.
pub fn rollback_changes(before: &Snapshot) -> Vec<(Field, Slot)> {
    before
        .iter()
        .filter_map(|(k, v)| Some((Field::from_key(k)?, v.clone())))
        .collect()
}

/// Fields whose current value differs from the stored "after" value.
pub fn foreign_changes(current: &Broadcast, after: &Snapshot) -> Vec<String> {
    after
        .iter()
        .filter(|(k, v)| {
            Field::from_key(k).is_none_or(|f| !allowlist::read(current.object(), f).same(v))
        })
        .map(|(k, _)| k.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;
    use axum::Router;
    use axum::extract::State;
    use axum::http::{HeaderMap, Method, StatusCode, Uri};
    use axum::routing::any;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    const SITE: &str = "88f7af54-98f8-306a-a1c7-c9349722b1f6";
    const WIFI: &str = "6f1c2a3b-0000-4000-8000-000000000001";
    const PASS: &str = "correct-horse-battery";

    #[test]
    fn write_path_only_puts_wifi_broadcasts() {
        // ADR 0042: one PUT to wifi/broadcasts/{id}, no other method.
        let src = include_str!("write.rs");
        let code = &src[..src.find("#[cfg(test)]").unwrap()];
        for forbidden in [
            ".post(",
            ".patch(",
            ".delete(",
            ".request(",
            "Method::",
            "/api/s/",
        ] {
            assert!(!code.contains(forbidden), "{forbidden}");
        }
        assert_eq!(code.matches(".put(").count(), 1);
        let compact: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(compact.contains(
            ".put(format!(\"{}{INTEGRATION}/sites/{site}/wifi/broadcasts/{id}\",c.cfg.url))"
        ));
        assert_eq!(code.matches(".get(").count(), 1, "only `v.get(\"code\")`");
        assert_eq!(INTEGRATION, "/proxy/network/integration/v1");
        // The read path stays GET only (see `this_crate_only_reads`).
        let lib = include_str!("lib.rs");
        assert!(!lib[..lib.find("#[cfg(test)]").unwrap()].contains(".put("));
    }

    /// Wi-Fi broadcast detail objects as Network 10.6.106 and 11.0.89 return
    /// them (from the published OpenAPI files). Unknown fields must survive.
    fn detail(version: &str) -> Value {
        let mut v = json!({
            "type": "STANDARD", "id": WIFI, "name": "Voice", "enabled": true,
            "metadata": {"origin": "USER_DEFINED"},
            "network": {"type": "SPECIFIC", "networkId": "0b5e6d3c-0000-4000-8000-000000000002"},
            "securityConfiguration": {"type": "WPA2_WPA3_PERSONAL", "passphrase": PASS,
                "pmfMode": "OPTIONAL", "fastRoamingEnabled": true, "wpa3FastRoamingEnabled": false,
                "groupRekeyIntervalSeconds": 3600,
                "saeConfiguration": {"anticloggingThresholdSeconds": 5, "syncTimeSeconds": 5}},
            "broadcastingFrequenciesGHz": [2.4, 5],
            "multicastToUnicastConversionEnabled": false, "clientIsolationEnabled": false,
            "hideName": false, "uapsdEnabled": false, "channel2gLockedTo6": true,
            "dtimPeriod2gLockedTo3": false, "advertiseDeviceName": false,
            "arpProxyEnabled": false, "bssTransitionEnabled": false, "bandSteeringEnabled": true,
            "mloEnabled": false,
            "basicDataRateKbpsByFrequencyGHz": {"2.4": 12000, "5": 12000},
            "blackoutScheduleConfiguration": {"type": "PER_DAY", "days": []},
            "mdnsProxyConfiguration": {"mode": "AUTO"}
        });
        if version == "11.0.89" {
            // A field that TalkOps does not know.
            v["futureFieldOf11"] = json!({"x": [1, 2]});
        }
        v
    }

    #[derive(Clone, Default)]
    struct Mock {
        obj: Arc<Mutex<Value>>,
        requests: Arc<Mutex<Vec<(String, String)>>>,
        puts: Arc<Mutex<Vec<Value>>>,
        put_status: Arc<Mutex<u16>>,
        /// Values that UniFi ignores on PUT (read-back mismatch).
        ignore: Arc<Mutex<Option<&'static str>>>,
    }

    async fn console(
        State(m): State<Mock>,
        method: Method,
        uri: Uri,
        h: HeaderMap,
        body: axum::body::Bytes,
    ) -> Result<axum::Json<Value>, (StatusCode, axum::Json<Value>)> {
        m.requests
            .lock()
            .unwrap()
            .push((method.to_string(), uri.path().to_owned()));
        if h.get("x-api-key").and_then(|v| v.to_str().ok()) != Some("k1") {
            return Err((StatusCode::UNAUTHORIZED, axum::Json(json!({}))));
        }
        let path = format!("/proxy/network/integration/v1/sites/{SITE}/wifi/broadcasts/{WIFI}");
        if uri.path() != path {
            return Err((StatusCode::NOT_FOUND, axum::Json(json!({}))));
        }
        match method {
            Method::GET => Ok(axum::Json(m.obj.lock().unwrap().clone())),
            Method::PUT => {
                let status = *m.put_status.lock().unwrap();
                if status != 200 {
                    return Err((
                        StatusCode::from_u16(status).unwrap(),
                        axum::Json(
                            json!({"code": "api.wifi.invalid", "message": format!("bad {PASS}")}),
                        ),
                    ));
                }
                let body: Value = serde_json::from_slice(&body).unwrap();
                m.puts.lock().unwrap().push(body.clone());
                let mut obj = body;
                obj["id"] = json!(WIFI);
                obj["metadata"] = json!({"origin": "USER_DEFINED"});
                if let Some(k) = *m.ignore.lock().unwrap() {
                    obj[k] = m.obj.lock().unwrap()[k].clone();
                }
                *m.obj.lock().unwrap() = obj.clone();
                Ok(axum::Json(obj))
            }
            _ => Err((StatusCode::METHOD_NOT_ALLOWED, axum::Json(json!({})))),
        }
    }

    async fn start(obj: Value) -> (Client, Mock) {
        let mock = Mock {
            obj: Arc::new(Mutex::new(obj)),
            put_status: Arc::new(Mutex::new(200)),
            ..Mock::default()
        };
        let app = Router::new()
            .route("/{*path}", any(console))
            .with_state(mock.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = Client::new(Config {
            url: format!("http://{addr}"),
            site: "default".into(),
            api_key: "k1".into(),
            cert_sha256: String::new(),
        })
        .unwrap();
        (client, mock)
    }

    fn set(field: Field, v: Value) -> (Field, Slot) {
        (field, Slot::Present { value: v })
    }

    #[tokio::test]
    async fn full_replace_keeps_all_other_fields() {
        for version in ["10.6.106", "11.0.89"] {
            let original = detail(version);
            let (client, mock) = start(original.clone()).await;
            let guard = Guard::Hash(allowlist::hash(&original));
            let applied = client
                .writer()
                .apply(
                    SITE,
                    WIFI,
                    &[
                        set(Field::BssTransition, json!(true)),
                        set(Field::Wpa3FastRoaming, json!(true)),
                        set(
                            Field::HandoffSuggestions,
                            json!({"band5GHzRssiThreshold": -75, "band6GHzRssiThreshold": null}),
                        ),
                        set(Field::DtimOverride, json!({"2.4": 1, "5": 1, "6": 1})),
                    ],
                    &guard,
                )
                .await
                .unwrap();
            assert_eq!(applied.name, "Voice");
            assert_eq!(
                applied.before["bssTransitionEnabled"],
                Slot::Present {
                    value: json!(false)
                }
            );
            assert_eq!(
                applied.before["handoffSuggestionsConfiguration"],
                Slot::absent()
            );
            assert_eq!(
                applied.after["bssTransitionEnabled"],
                Slot::Present { value: json!(true) }
            );

            // Only GET and one PUT, only on the SSID path.
            let reqs = mock.requests.lock().unwrap().clone();
            let methods: Vec<&str> = reqs.iter().map(|(m, _)| m.as_str()).collect();
            assert_eq!(methods, ["GET", "PUT", "GET"], "{version}");
            assert!(
                reqs.iter()
                    .all(|(_, p)| p.ends_with(&format!("/wifi/broadcasts/{WIFI}")))
            );

            // The body is the GET object without `id`/`metadata`, plus the
            // allowlist changes; all other fields are unchanged.
            let put = mock.puts.lock().unwrap()[0].clone();
            assert!(put.get("id").is_none() && put.get("metadata").is_none());
            let diff = allowlist::differences(&allowlist::update_body(&original), &put);
            assert!(diff.iter().all(|p| allowlist::allowed_path(p)), "{diff:?}");
            assert_eq!(diff.len(), 4, "{diff:?}");
            assert_eq!(put["securityConfiguration"]["passphrase"], PASS);
            assert_eq!(put["channel2gLockedTo6"], true);
            assert_eq!(put["dtimPeriod2gLockedTo3"], false);
            assert_eq!(
                put["basicDataRateKbpsByFrequencyGHz"],
                original["basicDataRateKbpsByFrequencyGHz"]
            );
            if version == "11.0.89" {
                assert_eq!(put["futureFieldOf11"], json!({"x": [1, 2]}));
            }
            // Snapshots hold no secret.
            let snaps = format!("{:?}{:?}", applied.before, applied.after);
            assert!(!snaps.contains(PASS));
        }
    }

    #[tokio::test]
    async fn rollback_restores_only_the_changed_fields() {
        let original = detail("11.0.89");
        let (client, mock) = start(original.clone()).await;
        let w = client.writer();
        let applied = w
            .apply(
                SITE,
                WIFI,
                &[
                    set(Field::Uapsd, json!(true)),
                    set(
                        Field::HandoffSuggestions,
                        json!({"band5GHzRssiThreshold": -70}),
                    ),
                ],
                &Guard::Hash(allowlist::hash(&original)),
            )
            .await
            .unwrap();
        // Another person changes a field that TalkOps did not touch.
        mock.obj.lock().unwrap()["arpProxyEnabled"] = json!(true);
        let b = client.wifi_broadcast(SITE, WIFI).await.unwrap();
        assert!(foreign_changes(&b, &applied.after).is_empty());
        w.apply(
            SITE,
            WIFI,
            &rollback_changes(&applied.before),
            &Guard::Values(applied.after.clone()),
        )
        .await
        .unwrap();
        let now = mock.obj.lock().unwrap().clone();
        assert_eq!(now["uapsdEnabled"], false);
        assert!(now.get("handoffSuggestionsConfiguration").is_none());
        assert_eq!(now["arpProxyEnabled"], true, "the foreign change stays");
    }

    #[tokio::test]
    async fn foreign_change_stops_rollback() {
        let original = detail("11.0.89");
        let (client, mock) = start(original.clone()).await;
        let w = client.writer();
        let applied = w
            .apply(
                SITE,
                WIFI,
                &[set(Field::Uapsd, json!(true))],
                &Guard::Hash(allowlist::hash(&original)),
            )
            .await
            .unwrap();
        mock.obj.lock().unwrap()["uapsdEnabled"] = json!(false);
        let b = client.wifi_broadcast(SITE, WIFI).await.unwrap();
        assert_eq!(foreign_changes(&b, &applied.after), ["uapsdEnabled"]);
        let err = w
            .apply(
                SITE,
                WIFI,
                &rollback_changes(&applied.before),
                &Guard::Values(applied.after.clone()),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, WriteError::Conflict { .. }));
        // The user accepts the foreign change: rollback without a guard.
        w.apply(SITE, WIFI, &rollback_changes(&applied.before), &Guard::None)
            .await
            .unwrap();
        // A changed SSID stops an apply from an old proposal.
        let err = w
            .apply(
                SITE,
                WIFI,
                &[set(Field::Uapsd, json!(true))],
                &Guard::Hash(allowlist::hash(&original) + "x"),
            )
            .await
            .unwrap_err();
        assert_eq!(err.class(), "ssid_changed");
        assert_eq!(mock.puts.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn put_errors() {
        let original = detail("10.6.106");
        let (client, mock) = start(original.clone()).await;
        let guard = Guard::Hash(allowlist::hash(&original));
        let change = [set(Field::Uapsd, json!(true))];
        *mock.put_status.lock().unwrap() = 403;
        let err = client
            .writer()
            .apply(SITE, WIFI, &change, &guard)
            .await
            .unwrap_err();
        assert!(matches!(err, WriteError::Forbidden));
        *mock.put_status.lock().unwrap() = 401;
        let err = client
            .writer()
            .apply(SITE, WIFI, &change, &guard)
            .await
            .unwrap_err();
        assert_eq!(err.class(), "unauthorized");
        *mock.put_status.lock().unwrap() = 400;
        let err = client
            .writer()
            .apply(SITE, WIFI, &change, &guard)
            .await
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "UniFi refused the change (api.wifi.invalid)"
        );
        assert!(!format!("{err:?}").contains(PASS));
        // Wrong key on the GET: no PUT at all.
        let bad = Client::new(Config {
            api_key: "nope".into(),
            ..client.cfg.clone()
        })
        .unwrap();
        let err = bad
            .writer()
            .apply(SITE, WIFI, &change, &guard)
            .await
            .unwrap_err();
        assert_eq!(err.class(), "unauthorized");
        assert!(mock.puts.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn refuses_unsafe_writes() {
        // Masked passphrase: a PUT would overwrite it.
        let mut masked = detail("11.0.89");
        masked["securityConfiguration"]["passphrase"] = json!("************");
        let (client, mock) = start(masked.clone()).await;
        let err = client
            .writer()
            .apply(
                SITE,
                WIFI,
                &[set(Field::Uapsd, json!(true))],
                &Guard::Hash(allowlist::hash(&masked)),
            )
            .await
            .unwrap_err();
        assert_eq!(err.class(), "secret_not_readable");

        // Missing field of an older version, invalid value, empty and double changes.
        let mut old = detail("10.6.106");
        old.as_object_mut().unwrap().remove("channel2gLockedTo6");
        old.as_object_mut().unwrap().remove("bssTransitionEnabled");
        *mock.obj.lock().unwrap() = old.clone();
        let g = Guard::Hash(allowlist::hash(&old));
        let w = client.writer();
        for changes in [
            vec![set(Field::BssTransition, json!(true))],
            vec![set(Field::DtimOverride, json!({"2.4": 1, "5": 1, "6": 1}))],
            vec![set(Field::Uapsd, json!("on"))],
            vec![],
            vec![
                set(Field::Uapsd, json!(true)),
                set(Field::Uapsd, json!(false)),
            ],
        ] {
            let err = w.apply(SITE, WIFI, &changes, &g).await.unwrap_err();
            assert_eq!(err.class(), "invalid", "{changes:?}");
        }
        // Path injection through IDs.
        let err = w
            .apply("../x", WIFI, &[set(Field::Uapsd, json!(true))], &g)
            .await
            .unwrap_err();
        assert_eq!(err.class(), "invalid");
        assert!(mock.puts.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn read_back_mismatch() {
        let original = detail("11.0.89");
        let (client, mock) = start(original.clone()).await;
        *mock.ignore.lock().unwrap() = Some("uapsdEnabled");
        let err = client
            .writer()
            .apply(
                SITE,
                WIFI,
                &[set(Field::Uapsd, json!(true))],
                &Guard::Hash(allowlist::hash(&original)),
            )
            .await
            .unwrap_err();
        let WriteError::Verify { before, after } = err else {
            panic!()
        };
        assert_eq!(before, after, "UniFi kept the old value");
        assert_eq!(
            after["uapsdEnabled"],
            Slot::Present {
                value: json!(false)
            }
        );
    }

    #[test]
    fn body_refuses_other_fields() {
        let d = detail("11.0.89");
        assert!(body(&d, &[set(Field::Uapsd, json!(true))]).is_ok());
        // `merge` only knows allowlist pointers; `body` checks the result again.
        let diff = allowlist::differences(&allowlist::update_body(&d), &body(&d, &[]).unwrap());
        assert!(diff.is_empty());
    }
}
