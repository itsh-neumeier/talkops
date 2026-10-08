//! Text to speech with Piper: system prompts and voicemail greetings.
//!
//! Piper runs as a child process per batch (`--json-input`: one JSON line per
//! sentence with its own output file), so the voice model is loaded once per
//! batch. Files are rendered under a temporary name and renamed when done, so
//! FreeSWITCH never plays a half-written file.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, bail};
use talkops_core::prompts;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct Piper {
    pub bin: PathBuf,
    pub voices_dir: PathBuf,
}

impl Piper {
    fn model_path(&self, model: &str) -> PathBuf {
        self.voices_dir.join(format!("{model}.onnx"))
    }

    /// True if the binary and the system voice for `lang` are installed.
    pub fn available(&self, lang: &str) -> bool {
        self.bin.is_file()
            && self
                .model_path(prompts::voice(prompts::language(lang)))
                .is_file()
    }

    /// Renders `(text, file)` pairs in one Piper run with the system voice.
    pub async fn render(&self, lang: &str, items: &[(String, PathBuf)]) -> anyhow::Result<()> {
        self.render_with(prompts::voice(prompts::language(lang)), items)
            .await
    }

    /// Renders `(text, file)` pairs in one Piper run with voice `model`.
    pub async fn render_with(
        &self,
        model: &str,
        items: &[(String, PathBuf)],
    ) -> anyhow::Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let model = self.model_path(model);
        anyhow::ensure!(
            self.bin.is_file() && model.is_file(),
            "Piper or voice {} not installed",
            model.display()
        );
        let mut input = String::new();
        let mut renames = Vec::new();
        for (text, file) in items {
            let dir = file.parent().context("output file without directory")?;
            tokio::fs::create_dir_all(dir)
                .await
                .with_context(|| format!("create {}", dir.display()))?;
            let tmp = file.with_extension("tmp.wav");
            let line = serde_json::json!({ "text": clean(text), "output_file": tmp });
            input.push_str(&line.to_string());
            input.push('\n');
            renames.push((tmp, file.clone()));
        }

        let mut child = Command::new(&self.bin)
            .arg("--model")
            .arg(&model)
            .arg("--json-input")
            .arg("--quiet")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("start {}", self.bin.display()))?;
        let mut stdin = child.stdin.take().context("piper stdin")?;
        stdin.write_all(input.as_bytes()).await?;
        drop(stdin);
        let out = child.wait_with_output().await?;
        if !out.status.success() {
            bail!(
                "piper failed ({}): {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        for (tmp, file) in renames {
            let size = tokio::fs::metadata(&tmp)
                .await
                .map(|m| m.len())
                .unwrap_or(0);
            if size <= 44 {
                let _ = tokio::fs::remove_file(&tmp).await;
                bail!("piper produced no audio for {}", file.display());
            }
            tokio::fs::rename(&tmp, &file).await?;
        }
        Ok(())
    }
}

/// One line of plain text: Piper reads one sentence batch per JSON line.
fn clean(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Renders all missing system prompts for every language with a voice.
pub async fn ensure_prompts(piper: &Piper, sounds_dir: &Path) -> anyhow::Result<usize> {
    let mut rendered = 0;
    for lang in prompts::LANGUAGES {
        let missing: Vec<(String, PathBuf)> = prompts::keys()
            .filter_map(|key| {
                let path = prompts::path(sounds_dir, &key, lang)?;
                let text = prompts::text(&key, lang)?;
                (!path.is_file()).then(|| (text.into_owned(), path))
            })
            .collect();
        if missing.is_empty() {
            continue;
        }
        if !piper.available(lang) {
            tracing::warn!(lang, "no Piper voice installed, system prompts stay silent");
            continue;
        }
        tracing::info!(lang, count = missing.len(), "rendering system prompts");
        piper.render(lang, &missing).await?;
        rendered += missing.len();
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Stand-in for Piper: writes a fake WAV to every `output_file` it reads.
    fn fake_piper(dir: &Path) -> Piper {
        let bin = dir.join("piper");
        std::fs::write(
            &bin,
            "#!/bin/sh\nwhile read -r line; do\n  f=$(echo \"$line\" | sed 's/.*\"output_file\":\"\\([^\"]*\\)\".*/\\1/')\n  \
             head -c 2000 /dev/zero > \"$f\"\ndone\n",
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let voices = dir.join("voices");
        std::fs::create_dir_all(&voices).unwrap();
        for lang in prompts::LANGUAGES {
            std::fs::write(voices.join(format!("{}.onnx", prompts::voice(lang))), b"x").unwrap();
        }
        Piper {
            bin,
            voices_dir: voices,
        }
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("talkops-tts-{}", std::process::id()));
        let dir = dir.join(format!("{:?}", std::thread::current().id()).replace(['(', ')'], ""));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn renders_missing_prompts_once() {
        let dir = tempdir();
        let piper = fake_piper(&dir);
        let sounds = dir.join("sounds");
        let total = prompts::keys().count() * prompts::LANGUAGES.len();
        assert_eq!(ensure_prompts(&piper, &sounds).await.unwrap(), total);
        assert_eq!(ensure_prompts(&piper, &sounds).await.unwrap(), 0);
        let p = prompts::path(&sounds, "vm_goodbye", "en").unwrap();
        assert!(p.is_file());
        assert!(!p.with_extension("tmp.wav").exists());

        // A missing voice is reported, not silently skipped, for jobs.
        let none = Piper {
            bin: dir.join("nope"),
            voices_dir: dir.clone(),
        };
        assert!(
            none.render("de", &[("x".into(), dir.join("x.wav"))])
                .await
                .is_err()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
