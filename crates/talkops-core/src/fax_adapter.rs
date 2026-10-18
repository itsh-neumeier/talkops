//! Isolated, bounded format and mailbox adapter process.
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn run(input: &serde_json::Value) -> Result<serde_json::Value, String> {
    let python = std::env::var("TALKOPS_FAX_PYTHON").unwrap_or_else(|_| "python3".into());
    let script = std::env::var("TALKOPS_FAX_HELPER")
        .unwrap_or_else(|_| "/usr/share/talkops/fax/helper.py".into());
    let mut child = tokio::process::Command::new(python)
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "Fax adapter is unavailable".to_owned())?;
    let bytes = serde_json::to_vec(input).map_err(|_| "Invalid fax adapter request")?;
    let mut stdin = child.stdin.take().ok_or("Fax adapter input unavailable")?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or("Fax adapter output unavailable")?;
    let operation = async {
        stdin
            .write_all(&bytes)
            .await
            .map_err(|_| "Fax adapter input failed")?;
        drop(stdin);
        let mut output = Vec::new();
        (&mut stdout)
            .take(45 * 1024 * 1024 + 1)
            .read_to_end(&mut output)
            .await
            .map_err(|_| "Fax adapter output failed")?;
        if output.len() > 45 * 1024 * 1024 {
            return Err("Fax adapter output exceeds limit".to_owned());
        }
        let status = child.wait().await.map_err(|_| "Fax adapter wait failed")?;
        let value: serde_json::Value =
            serde_json::from_slice(&output).map_err(|_| "Invalid fax adapter result")?;
        if !status.success() {
            return Err(
                "Fax adapter failed: check the mailbox connection or document format and limits"
                    .to_owned(),
            );
        }
        Ok(value)
    };
    tokio::time::timeout(std::time::Duration::from_secs(120), operation)
        .await
        .map_err(|_| "Fax adapter timed out".to_owned())?
}
