//! Conversion jobs never dial: retries can only recreate document derivatives.
use anyhow::Context;
use serde_json::json;
use sqlx::PgPool;
use talkops_core::{fax, jobs::Job};
use uuid::Uuid;

pub async fn run(pool: &PgPool, job: &Job) -> anyhow::Result<()> {
    let id: Uuid = serde_json::from_value(job.payload["fax_id"].clone())?;
    let f = fax::get(pool, job.tenant_id, id).await?;
    let receive = job.kind == fax::RECEIVE;
    if (!receive && f.state != "preparing") || (receive && f.state != "received") {
        return Ok(());
    }
    let root = std::env::var("TALKOPS_FAX_DIR").unwrap_or_else(|_| "/var/lib/talkops/fax".into());
    let directory = std::path::Path::new(&root)
        .join(job.tenant_id.to_string())
        .join(id.to_string());
    let inputs: Vec<fax::InputFile> = serde_json::from_value(f.inputs.clone())?;
    let files = if receive {
        vec!["received.tif".to_owned()]
    } else {
        inputs.into_iter().map(|i| i.file).collect()
    };
    let result=talkops_core::fax_adapter::run(&json!({"mode":"convert","directory":directory,"files":files,
        "receive":receive,"header":f.header,"footer":f.footer,"sender":f.source,"recipient":f.destination,
        "date":f.created_at.to_rfc3339(),"literal_header":!f.sender_email.is_empty()})).await;
    let (pages, state, reason) = match result {
        Ok(v) => (
            v["pages"].as_i64().context("adapter omitted page count")? as i32,
            if receive {
                if f.result.starts_with("Partial fax:") {
                    "failed"
                } else {
                    "succeeded"
                }
            } else if job.payload["automatic"] == true {
                "queued"
            } else {
                "draft"
            },
            if receive {
                f.result.clone()
            } else {
                String::new()
            },
        ),
        Err(e) => (0, "failed", e),
    };
    let mut tx = pool.begin().await?;
    let changed=sqlx::query("UPDATE faxes SET state=$3,pages=$4,result=$5,updated_at=now() WHERE tenant_id=$1 AND id=$2 AND state=$6")
        .bind(job.tenant_id).bind(id).bind(state).bind(pages).bind(&reason).bind(if receive {"received"} else {"preparing"}).execute(&mut *tx).await?.rows_affected();
    if changed > 0 && (receive || state == "failed") {
        let recipient = if receive {
            sqlx::query_scalar::<_, String>(
                "SELECT reception_email FROM fax_endpoints WHERE tenant_id=$1 AND id=$2",
            )
            .bind(job.tenant_id)
            .bind(f.endpoint_id)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or_default()
        } else {
            f.sender_email
        };
        if !recipient.is_empty() {
            sqlx::query("INSERT INTO fax_mail_outbox(tenant_id,fax_id,recipient,kind,reason) VALUES($1,$2,$3,$4,$5)")
                .bind(job.tenant_id).bind(id).bind(recipient).bind(if receive && state == "succeeded" {"fax_received"} else {"fax_failed"}).bind(&reason).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}
