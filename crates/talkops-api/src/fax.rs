//! Fax control plane. Dialing is separate from retryable conversion and mail jobs.
use crate::{
    AppState,
    fsxml::{self, dialplan},
    routes::fax::{Upload, UploadFile, store_upload},
};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use std::{path::PathBuf, time::Duration};
use talkops_core::{
    error::CoreResult,
    fax::{self, Fax},
    jobs, mail, settings,
    tenant::TenantId,
};
use talkops_esl::{Headers, outbound::OutboundSession};
use uuid::Uuid;

pub fn directory(s: &AppState, tenant: TenantId, id: Uuid) -> PathBuf {
    s.media.fax.join(tenant.to_string()).join(id.to_string())
}

/// No user-controlled path or channel variable may create ESL syntax.
fn safe(value: &str) -> Result<&str, String> {
    if value.contains(['|', '{', '}', '[', ']', '\r', '\n', '\'', '"', '\\', '$']) {
        Err("Unsafe fax route or path".into())
    } else {
        Ok(value)
    }
}
pub fn spawn(state: AppState) {
    let mail_state = state.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = mail_cycle(&mail_state).await {
                tracing::warn!(error=%e,"fax mailbox processing failed");
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
    tokio::spawn(async move {
        loop {
            if let Err(e) = dispatch(&state).await {
                tracing::warn!(error=%e,"fax dispatch failed");
            }
            if let Err(e) = deliver_mail(&state).await {
                tracing::warn!(error=%e,"fax report delivery failed");
            }
            if let Err(e) = reconcile(&state).await {
                tracing::warn!(error=%e,"fax reconciliation failed");
            }
            if let Err(e) = purge(&state).await {
                tracing::warn!(error=%e,"fax retention failed");
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

async fn tenants(db: &PgPool) -> CoreResult<Vec<TenantId>> {
    Ok(sqlx::query_scalar("SELECT tenant_id FROM fax_settings")
        .fetch_all(db)
        .await?)
}

async fn dispatch(s: &AppState) -> anyhow::Result<()> {
    let Some(esl) = s.telephony.esl.get().await else {
        return Ok(());
    };
    // One short database lock serializes dispatch decisions across server instances.
    let mut tx = s.db.begin().await?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(681948251)")
        .fetch_one(&mut *tx)
        .await?;
    if !locked {
        return Ok(());
    }
    let f:Option<Fax>=sqlx::query_as("SELECT f.* FROM faxes f WHERE direction='outbound' AND state='queued' AND next_attempt_at<=now() AND NOT EXISTS(SELECT 1 FROM faxes a WHERE a.trunk_id=f.trunk_id AND a.state IN ('dialing','sending')) ORDER BY next_attempt_at LIMIT 1 FOR UPDATE SKIP LOCKED").fetch_optional(&mut *tx).await?;
    let Some(f) = f else {
        return Ok(());
    };
    let route = fax::resolve_number(&s.db, f.tenant_id, f.owner_id, f.number_id).await;
    let allowed = if f.sender_email.is_empty() {
        true
    } else {
        fax::mail_user(&s.db, f.tenant_id, &f.sender_email)
            .await
            .is_ok_and(|u| u == f.owner_id)
    };
    let route = match route {
        Ok(route)
            if allowed
                && Some(route.number_id) == f.number_id
                && Some(route.trunk_id) == f.trunk_id
                && route.e164 == f.source =>
        {
            route
        }
        _ => {
            sqlx::query("UPDATE faxes SET state='failed',result='Fax permission or sender route is no longer available',updated_at=now() WHERE id=$1").bind(f.id).execute(&mut *tx).await?;
            queue_report(
                &mut tx,
                &f,
                "fax_failed",
                "Fax permission or sender route is no longer available",
            )
            .await?;
            tx.commit().await?;
            return Ok(());
        }
    };
    let call = Uuid::new_v4();
    let cfg = fax::settings(&s.db, f.tenant_id).await?;
    let settings = settings::get(&s.db, f.tenant_id).await?;
    let actions = dialplan::outbound_actions(
        &s.catalog,
        &settings.dial_plan(),
        &route,
        &route.e164,
        Some(&f.destination),
        &f.destination,
        false,
    )?;
    let dial = actions
        .iter()
        .find(|(app, _)| *app == "bridge")
        .map(|(_, v)| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing fax route"))?;
    let mut vars = vec![
        format!("origination_uuid={call}"),
        format!("talkops_fax_id={}", f.id),
        format!("talkops_tenant_id={}", f.tenant_id),
        "talkops_app=fax_out".into(),
        "talkops_direction=outbound".into(),
        "originate_timeout=60".into(),
        "ignore_early_media=true".into(),
    ];
    for (app, value) in &actions {
        if *app == "set" && !value.starts_with("absolute_codec_string=") {
            safe(value).map_err(anyhow::Error::msg)?;
            vars.push(value.clone());
            if let Some(number) = value.strip_prefix("effective_caller_id_number=") {
                vars.push(format!("origination_caller_id_number={number}"));
            }
            if let Some(name) = value.strip_prefix("effective_caller_id_name=") {
                vars.push(format!("origination_caller_id_name={name}"));
            }
        }
    }
    vars.extend([
        "absolute_codec_string=PCMA,PCMU".into(),
        "fax_ident=".to_owned() + &f.source,
        "fax_header=".into(),
        "fax_use_ecm=true".into(),
        "fax_enable_t38=".to_owned()
            + if cfg
                .trunk_modes
                .get(&route.trunk_id)
                .is_some_and(|v| v == "audio")
            {
                "false"
            } else {
                "true"
            },
        "fax_enable_t38_request=".to_owned()
            + if cfg
                .trunk_modes
                .get(&route.trunk_id)
                .is_some_and(|v| v == "audio")
            {
                "false"
            } else {
                "true"
            },
        "fax_enable_t38_insist=".to_owned()
            + if cfg
                .trunk_modes
                .get(&route.trunk_id)
                .is_some_and(|v| v == "required")
            {
                "true"
            } else {
                "false"
            },
    ]);
    safe(dial).map_err(anyhow::Error::msg)?;
    safe(&s.outbound_socket).map_err(anyhow::Error::msg)?;
    let command = format!(
        "originate {{^^|{}}}{dial} &socket({} async full)",
        vars.join("|"),
        s.outbound_socket
    );
    sqlx::query("UPDATE faxes SET state='dialing',attempts=attempts+1,call_uuid=$2,updated_at=now() WHERE id=$1").bind(f.id).bind(call).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO fax_attempts(tenant_id,fax_id,call_uuid) VALUES($1,$2,$3)")
        .bind(f.tenant_id)
        .bind(f.id)
        .bind(call)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let still_dialing: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM faxes WHERE tenant_id=$1 AND id=$2 AND call_uuid=$3 AND state='dialing')")
        .bind(f.tenant_id).bind(f.id).bind(call).fetch_one(&s.db).await?;
    if !still_dialing {
        return Ok(());
    }
    // An uncertain command outcome must never cause another originate automatically.
    if esl.bgapi(&command).await.is_err() {
        mark_unknown(&s.db, &f, "Originate outcome unknown; verify before resend").await?;
    }
    Ok(())
}

pub async fn handle(mut session: OutboundSession, s: AppState) {
    let result = call(&mut session, &s).await;
    if let Err(e) = result {
        tracing::warn!(error=%e,call=session.uuid(),"fax session failed; awaiting CDR reconciliation");
    }
    session.hangup("NORMAL_CLEARING").await;
}
async fn call(session: &mut OutboundSession, s: &AppState) -> anyhow::Result<()> {
    let tenant = TenantId(
        session
            .var("talkops_tenant_id")
            .ok_or_else(|| anyhow::anyhow!("missing tenant"))?
            .parse()?,
    );
    let cfg = fax::settings(&s.db, tenant).await?;
    if !cfg.enabled {
        anyhow::bail!("fax disabled");
    }
    let call_uuid: Uuid = session.uuid().parse()?;
    let outbound = session.var("talkops_app") == Some("fax_out");
    let f = if outbound {
        let id: Uuid = session
            .var("talkops_fax_id")
            .ok_or_else(|| anyhow::anyhow!("missing fax"))?
            .parse()?;
        let f = fax::get(&s.db, tenant, id).await?;
        if f.call_uuid != Some(call_uuid) || f.state != "dialing" {
            anyhow::bail!("stale fax attempt");
        }
        fax::resolve_number(&s.db, tenant, f.owner_id, f.number_id).await?;
        if !f.sender_email.is_empty() {
            fax::mail_user(&s.db, tenant, &f.sender_email).await?;
        }
        sqlx::query("UPDATE faxes SET state='sending',updated_at=now() WHERE tenant_id=$1 AND id=$2 AND state='dialing'").bind(tenant).bind(id).execute(&s.db).await?;
        f
    } else {
        let endpoint: Uuid = session
            .var("talkops_fax_endpoint")
            .ok_or_else(|| anyhow::anyhow!("missing endpoint"))?
            .parse()?;
        let e = fax::endpoints(&s.db, tenant)
            .await?
            .into_iter()
            .find(|e| e.id == endpoint && e.enabled)
            .ok_or_else(|| anyhow::anyhow!("disabled endpoint"))?;
        let id = call_uuid;
        let source =
            fsxml::sanitize_value(session.var("talkops_caller_number").unwrap_or("anonymous"));
        let destination = fsxml::sanitize_value(
            session
                .channel()
                .get("Caller-Destination-Number")
                .unwrap_or_default(),
        );
        sqlx::query("INSERT INTO faxes(id,tenant_id,owner_id,endpoint_id,direction,state,source,destination,call_uuid) VALUES($1,$2,$3,$4,'inbound','sending',$5,$6,$1) ON CONFLICT(id) DO NOTHING").bind(id).bind(tenant).bind(e.owner_id).bind(e.id).bind(source).bind(destination).execute(&s.db).await?;
        fax::get(&s.db, tenant, id).await?
    };
    let dir = directory(s, tenant, f.id);
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(if outbound {
        "document.tif"
    } else {
        "received.tif"
    });
    let path = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("invalid fax path"))?;
    safe(path).map_err(anyhow::Error::msg)?;
    session.execute("set", "fax_header=").await?;
    let ident = if outbound {
        f.source.clone()
    } else if let Some(id) = session
        .var("talkops_number_id")
        .and_then(|v| v.parse().ok())
    {
        talkops_core::trunks::get_number(&s.db, tenant, id)
            .await?
            .e164
    } else {
        "TalkOps".into()
    };
    session
        .execute(
            "set",
            &format!("fax_ident={}", fsxml::sanitize_value(&ident)),
        )
        .await?;
    session.execute("set", "fax_use_ecm=true").await?;
    session
        .execute("set", "absolute_codec_string=PCMA,PCMU")
        .await?;
    if !outbound {
        let mode = session
            .var("talkops_trunk_id")
            .and_then(|v| v.parse::<Uuid>().ok())
            .and_then(|id| cfg.trunk_modes.get(&id))
            .map(String::as_str)
            .unwrap_or("preferred");
        session
            .execute(
                "set",
                if mode == "audio" {
                    "fax_enable_t38=false"
                } else {
                    "fax_enable_t38=true"
                },
            )
            .await?;
        session
            .execute(
                "set",
                if mode == "required" {
                    "fax_enable_t38_insist=true"
                } else {
                    "fax_enable_t38_insist=false"
                },
            )
            .await?;
        session.execute("answer", "").await?;
    }
    let event = tokio::time::timeout(
        Duration::from_secs(1800),
        session.execute(if outbound { "txfax" } else { "rxfax" }, path),
    )
    .await??;
    finish(&s.db, call_uuid, &event.headers).await?;
    Ok(())
}

async fn queue_report(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    f: &Fax,
    kind: &str,
    reason: &str,
) -> Result<(), sqlx::Error> {
    let recipient = if f.direction == "inbound" {
        sqlx::query_scalar::<_, String>(
            "SELECT reception_email FROM fax_endpoints WHERE tenant_id=$1 AND id=$2",
        )
        .bind(f.tenant_id)
        .bind(f.endpoint_id)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or_default()
    } else {
        f.sender_email.clone()
    };
    if !recipient.is_empty() {
        sqlx::query("INSERT INTO fax_mail_outbox(tenant_id,fax_id,recipient,kind,reason) VALUES($1,$2,$3,$4,$5)").bind(f.tenant_id).bind(f.id).bind(recipient).bind(kind).bind(reason).execute(&mut **tx).await?;
    }
    Ok(())
}
pub async fn finish(db: &PgPool, call: Uuid, headers: &Headers) -> CoreResult<()> {
    let get = |key: &str| headers.get(&format!("variable_{key}")).unwrap_or_default();
    let pages = get("fax_document_transferred_pages")
        .parse::<i32>()
        .unwrap_or(0)
        .clamp(0, 1000);
    let success = ["1", "true"].contains(&get("fax_success")) && get("fax_result_code") == "0";
    let result = get("fax_result_text");
    let cause = headers
        .get("Hangup-Cause")
        .unwrap_or_else(|| get("hangup_cause"));
    let mut tx = db.begin().await?;
    let f: Option<Fax> = sqlx::query_as("SELECT * FROM faxes WHERE call_uuid=$1 FOR UPDATE")
        .bind(call)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(f) = f else {
        return Ok(());
    };
    if !["dialing", "sending", "unknown"].contains(&f.state.as_str()) {
        return Ok(());
    }
    let busy = !success
        && pages == 0
        && ["USER_BUSY", "NO_ANSWER", "NO_USER_RESPONSE"].contains(&cause)
        && f.attempts < 3
        && f.direction == "outbound";
    let complete = success && pages > 0 && (f.direction == "inbound" || pages == f.pages);
    let state = if busy {
        "queued"
    } else if f.direction == "inbound" && pages > 0 {
        "received"
    } else if complete {
        "succeeded"
    } else {
        "failed"
    };
    let reason = if complete {
        format!(
            "Fax transfer completed: {pages} pages; transport {}",
            fsxml::sanitize_value(get("fax_t38_status"))
        )
    } else {
        format!(
            "{}{} ({cause})",
            if pages > 0 { "Partial fax: " } else { "" },
            result
        )
    };
    sqlx::query("UPDATE faxes SET state=$2,result=$3,updated_at=now(),next_attempt_at=now()+interval '60 seconds' WHERE id=$1").bind(f.id).bind(state).bind(&reason).execute(&mut *tx).await?;
    sqlx::query("UPDATE fax_attempts SET result=$2,pages=$3,finished_at=now() WHERE call_uuid=$1")
        .bind(call)
        .bind(&reason)
        .bind(pages)
        .execute(&mut *tx)
        .await?;
    if state == "received" {
        let mut job = jobs::NewJob::new(fax::RECEIVE, json!({"fax_id":f.id}));
        job.tenant_id = f.tenant_id;
        job.max_attempts = 1;
        jobs::enqueue(&mut *tx, job).await?;
    } else if !busy {
        queue_report(
            &mut tx,
            &f,
            if complete { "fax_sent" } else { "fax_failed" },
            &reason,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
/// CDR replay is independent of the in-memory ESL connection.
pub async fn ingest_cdr(db: &PgPool, xml: &str) -> CoreResult<()> {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return Ok(());
    };
    let Some(vars) = doc.descendants().find(|n| n.has_tag_name("variables")) else {
        return Ok(());
    };
    let mut headers = Headers::default();
    for node in vars.children().filter(|n| n.is_element()) {
        let value = percent_encoding::percent_decode_str(node.text().unwrap_or_default())
            .decode_utf8_lossy()
            .into_owned();
        headers.push(format!("variable_{}", node.tag_name().name()), value);
    }
    if let Some(call) = headers.get("variable_uuid").and_then(|v| v.parse().ok()) {
        finish(db, call, &headers).await?;
    }
    Ok(())
}
async fn reconcile(s: &AppState) -> anyhow::Result<()> {
    let Some(esl) = s.telephony.esl.get().await else {
        return Ok(());
    };
    let faxes:Vec<Fax>=sqlx::query_as("SELECT * FROM faxes WHERE state IN ('dialing','sending') AND updated_at<now()-interval '2 minutes'").fetch_all(&s.db).await?;
    for f in faxes {
        if let Some(call) = f.call_uuid {
            if esl.api(&format!("uuid_exists {call}")).await?.trim() == "false" {
                mark_unknown(
                    &s.db,
                    &f,
                    "No final fax result. Verify delivery before resend.",
                )
                .await?;
            }
        }
    }
    Ok(())
}
async fn mark_unknown(db: &PgPool, f: &Fax, reason: &str) -> CoreResult<()> {
    let mut tx = db.begin().await?;
    let changed=sqlx::query("UPDATE faxes SET state='unknown',result=$2,updated_at=now() WHERE id=$1 AND state IN ('dialing','sending')").bind(f.id).bind(reason).execute(&mut *tx).await?.rows_affected();
    if changed > 0 {
        queue_report(&mut tx, f, "fax_failed", reason).await?;
    }
    tx.commit().await?;
    Ok(())
}
#[derive(Deserialize)]
pub struct MailFile {
    name: String,
    data: String,
}
#[derive(Deserialize)]
pub struct MailInput {
    uid: String,
    validity: String,
    sender: String,
    subject: String,
    automatic: bool,
    files: Vec<MailFile>,
    error: String,
    fingerprint: String,
}

async fn mail_cycle(s: &AppState) -> anyhow::Result<()> {
    for tenant in tenants(&s.db).await? {
        let cfg = fax::settings(&s.db, tenant).await?;
        if !cfg.enabled || !cfg.imap_enabled || !cfg.trusted_mailbox {
            continue;
        }
        let Some(password) = fax::password(&s.db, tenant, &s.secrets).await? else {
            continue;
        };
        let mut request = json!({"mode":"mail","host":cfg.imap_host,"port":cfg.imap_port,"username":cfg.imap_username,"password":password,"folder":cfg.imap_folder});
        let value = talkops_core::fax_adapter::run(&request)
            .await
            .map_err(anyhow::Error::msg)?;
        if value.is_null() {
            continue;
        }
        let input: MailInput = serde_json::from_value(value)?;
        let mailbox = format!(
            "{}:{}:{}:{}",
            cfg.imap_host, cfg.imap_port, cfg.imap_username, cfg.imap_folder
        );
        let ack = input.uid.clone();
        let validity = input.validity.clone();
        admit_mail(s, tenant, input, &mailbox).await?;
        request["ack"] = json!(ack);
        request["validity"] = json!(validity);
        talkops_core::fax_adapter::run(&request)
            .await
            .map_err(anyhow::Error::msg)?;
    }
    Ok(())
}

/// Apply mail admission and persist a receipt before mailbox acknowledgement.
pub async fn admit_mail(
    s: &AppState,
    tenant: TenantId,
    input: MailInput,
    mailbox: &str,
) -> anyhow::Result<()> {
    let receipt = format!("{mailbox}:{}:{}", input.validity, input.uid);
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM fax_mail_receipts WHERE tenant_id=$1 AND (receipt=$2 OR fingerprint=$3))").bind(tenant).bind(&receipt).bind(&input.fingerprint).fetch_one(&s.db).await?;
    if !exists {
        let sender = fax::email(&input.sender).ok();
        let user = if let Some(sender) = &sender {
            match fax::mail_user(&s.db, tenant, sender).await {
                Ok(user) => Some(user),
                Err(
                    talkops_core::error::CoreError::Validation(_)
                    | talkops_core::error::CoreError::NotFound,
                ) => None,
                Err(error) => return Err(error.into()),
            }
        } else {
            None
        };
        let reason = if user.is_none() {
            "Sender is not permitted to submit faxes."
        } else if !fax::number(&input.subject) {
            "Subject must contain only an international number, e.g. +49123456."
        } else {
            input.error.as_str()
        };
        let result = if reason.is_empty() && !input.automatic {
            let upload = Upload {
                destination: input.subject,
                number_id: None,
                files: input
                    .files
                    .into_iter()
                    .map(|f| UploadFile {
                        name: f.name,
                        data: f.data,
                    })
                    .collect(),
                receipt: Some(fax::Receipt {
                    key: receipt.clone(),
                    fingerprint: input.fingerprint.clone(),
                }),
            };
            match store_upload(
                    s,
                    tenant,
                    user.ok_or_else(|| anyhow::anyhow!("missing sender"))?,
                    upload,
                    sender.as_deref().unwrap_or_default(),
                    true,
                )
                .await {
                    Ok(_)=>Ok(()),
                    Err(crate::error::ApiError::Internal(_))=>anyhow::bail!("Temporary fax storage or database error; mailbox message remains unread"),
                    Err(_)=>Err("Fax submission failed. Check attachments, permissions and the outgoing number.".to_owned()),
                }
        } else {
            Err(if input.automatic {
                "Automatic messages cannot submit faxes.".into()
            } else {
                reason.to_owned()
            })
        };
        if let Err(reason) = result {
            let mut tx = s.db.begin().await?;
            let inserted=sqlx::query("INSERT INTO fax_mail_receipts(tenant_id,receipt,fingerprint,sender,result) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING").bind(tenant).bind(&receipt).bind(&input.fingerprint).bind(sender.as_deref().unwrap_or_default()).bind(&reason).execute(&mut *tx).await?.rows_affected();
            if inserted > 0 && !input.automatic {
                if let Some(sender) = sender {
                    sqlx::query("INSERT INTO fax_mail_outbox(tenant_id,recipient,kind,reason) VALUES($1,$2,'fax_rejected',$3)").bind(tenant).bind(sender).bind(reason).execute(&mut *tx).await?;
                }
            }
            tx.commit().await?;
        }
    }

    Ok(())
}

async fn deliver_mail(s: &AppState) -> anyhow::Result<()> {
    let row:Option<(Uuid,TenantId,Option<Uuid>,String,String,String)>=sqlx::query_as("UPDATE fax_mail_outbox SET attempts=attempts+1,run_at=now()+interval '5 minutes' WHERE id=(SELECT id FROM fax_mail_outbox WHERE NOT sent AND attempts<5 AND run_at<=now() ORDER BY run_at LIMIT 1 FOR UPDATE SKIP LOCKED) RETURNING id,tenant_id,fax_id,recipient,kind,reason").fetch_optional(&s.db).await?;
    let Some((id, tenant, fax_id, recipient, kind, reason)) = row else {
        return Ok(());
    };
    let Some(cfg) = mail::config(&s.db, tenant, &s.secrets).await? else {
        anyhow::bail!("SMTP is not configured for fax reports");
    };
    let mut values = crate::mailer::base_values(&s.db, tenant, &recipient).await?;
    let mut attachments = Vec::new();
    values.insert("fax.reason".into(), reason);
    values.insert(
        "fax.id".into(),
        fax_id.map(|v| v.to_string()).unwrap_or_default(),
    );
    values.insert("fax.source".into(), String::new());
    values.insert("fax.destination".into(), String::new());
    values.insert("fax.pages".into(), String::new());
    if let Some(fax_id) = fax_id {
        let f = fax::get(&s.db, tenant, fax_id).await?;
        values.insert("fax.source".into(), f.source);
        values.insert("fax.destination".into(), f.destination);
        values.insert("fax.pages".into(), f.pages.to_string());
        if kind == "fax_received" {
            let bytes = tokio::fs::read(directory(s, tenant, fax_id).join("document.pdf")).await?;
            attachments.push(crate::mailer::MailAttachment {
                filename: "fax.pdf".into(),
                content_type: "application/pdf".into(),
                data: bytes,
            });
        }
    }
    let lang = settings::get(&s.db, tenant).await?.default_language;
    let kind = talkops_core::mail_render::kind(&kind)
        .ok_or_else(|| anyhow::anyhow!("unknown fax mail type"))?;
    let (design, logo, rendered) =
        crate::mailer::render_for(&s.db, tenant, kind, &lang, &values).await?;
    let from = crate::mailer::sender(&cfg.from, &design.sender_name)?;
    let mut message =
        crate::mailer::build_message(from, &recipient, rendered, logo.as_ref(), attachments)?;
    message
        .headers_mut()
        .insert_raw(lettre::message::header::HeaderValue::new(
            lettre::message::header::HeaderName::new_from_ascii_str("Auto-Submitted"),
            "auto-replied".into(),
        ));
    crate::mailer::send(&cfg, message).await?;
    sqlx::query("UPDATE fax_mail_outbox SET sent=true WHERE id=$1")
        .bind(id)
        .execute(&s.db)
        .await?;
    Ok(())
}
async fn purge(s: &AppState) -> anyhow::Result<()> {
    for tenant in tenants(&s.db).await? {
        let days = fax::settings(&s.db, tenant).await?.retention_days;
        let ids:Vec<Uuid>=sqlx::query_scalar("DELETE FROM faxes WHERE tenant_id=$1 AND state IN ('draft','succeeded','failed','cancelled','unknown') AND updated_at<now()-make_interval(days=>$2) RETURNING id").bind(tenant).bind(days).fetch_all(&s.db).await?;
        for id in ids {
            let _ = tokio::fs::remove_dir_all(directory(s, tenant, id)).await;
        }
    }
    Ok(())
}
