//! Text to speech with Piper: system prompts and voicemail greetings.
//!
//! Piper runs as a child process per batch (`--json-input`: one JSON line per
//! sentence with its own output file), so the voice model is loaded once per
//! batch. Files are rendered under a temporary name and renamed when done, so
//! FreeSWITCH never plays a half-written file.

use std::collections::BTreeMap;
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
        // Parse every item before starting Piper. Invalid markup must never
        // become spoken text or leave a partially replaced batch.
        let parsed = items
            .iter()
            .map(|(text, file)| Ok((talkops_core::speech::segments(text, 1.0)?, file)))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let parent = items[0]
            .1
            .parent()
            .context("output file without directory")?;
        let work = parent.join(format!(".tts-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&work).await?;
        let result = async {
            let mut batches: BTreeMap<u64, Vec<(String, PathBuf)>> = BTreeMap::new();
            let mut outputs = Vec::new();
            for (i, (parts, file)) in parsed.into_iter().enumerate() {
                let mut paths = Vec::new();
                for (j, part) in parts.into_iter().enumerate() {
                    let path = work.join(format!("{i}-{j}.wav"));
                    batches
                        .entry(part.speed.to_bits())
                        .or_default()
                        .push((part.text, path.clone()));
                    paths.push(path);
                }
                outputs.push((paths, file));
            }
            // The pinned Piper accepts length_scale per process. Batch all
            // segments that use the same speed to avoid repeated model loads.
            for (bits, batch) in batches {
                self.render_plain(model, &batch, f64::from_bits(bits))
                    .await?;
            }
            for (i, (paths, file)) in outputs.into_iter().enumerate() {
                let staged = if paths.len() == 1 {
                    paths[0].clone()
                } else {
                    let joined = work.join(format!("{i}-joined.wav"));
                    join_wav(&paths, &joined)?;
                    joined
                };
                tokio::fs::create_dir_all(file.parent().context("output directory")?).await?;
                tokio::fs::rename(&staged, file).await?;
            }
            Ok(())
        }
        .await;
        let _ = tokio::fs::remove_dir_all(&work).await;
        result
    }

    async fn render_plain(
        &self,
        model: &str,
        items: &[(String, PathBuf)],
        speed: f64,
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

        let mut command = Command::new(&self.bin);
        if speed != 1.0 {
            // Scale the model's own default. A factor of 1.0 retains the
            // existing voice, including models whose default is not 1.0.
            let config: serde_json::Value =
                serde_json::from_slice(&tokio::fs::read(model.with_extension("onnx.json")).await?)?;
            let default = config["inference"]["length_scale"]
                .as_f64()
                .filter(|v| v.is_finite() && *v > 0.0)
                .context("voice config has no valid inference.length_scale")?;
            command
                .arg("--length_scale")
                .arg((default / speed).to_string());
        }
        let mut child = command
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

/// Concatenates PCM samples, not WAV headers. Reject incompatible segments.
fn join_wav(paths: &[PathBuf], output: &Path) -> anyhow::Result<()> {
    let first = hound::WavReader::open(&paths[0])?;
    let spec = first.spec();
    anyhow::ensure!(
        spec.sample_format == hound::SampleFormat::Int && spec.bits_per_sample == 16,
        "Piper segments must be 16-bit PCM"
    );
    let mut writer = hound::WavWriter::create(output, spec)?;
    for path in paths {
        let mut reader = hound::WavReader::open(path)?;
        anyhow::ensure!(reader.spec() == spec, "incompatible Piper WAV segments");
        for sample in reader.samples::<i16>() {
            writer.write_sample(sample?)?;
        }
    }
    writer.finalize()?;
    Ok(())
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

#[cfg(all(test, unix))]
pub(crate) mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Stand-in for Piper: writes a fake WAV to every `output_file` it reads.
    pub(crate) fn fake_piper(dir: &Path) -> Piper {
        let bin = dir.join("piper");
        std::fs::write(
            &bin,
            r#"#!/usr/bin/env python3
import json, pathlib, struct, sys, wave
args = sys.argv[1:]
scale = float(args[args.index('--length_scale') + 1]) if '--length_scale' in args else 0.9
log = pathlib.Path(__file__).with_suffix('.log')
for line in sys.stdin:
    item = json.loads(line)
    with log.open('a') as out:
        out.write(json.dumps({'text': item['text'], 'scale': scale}) + '\n')
    with wave.open(item['output_file'], 'wb') as out:
        out.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
        out.writeframes(struct.pack('<h', ord(item['text'][0])) * round(160 * scale / 0.9))
"#,
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let voices = dir.join("voices");
        std::fs::create_dir_all(&voices).unwrap();
        for voice in talkops_core::audio::VOICES {
            std::fs::write(voices.join(format!("{}.onnx", voice.model)), b"x").unwrap();
            std::fs::write(
                voices.join(format!("{}.onnx.json", voice.model)),
                br#"{"inference":{"length_scale":0.9}}"#,
            )
            .unwrap();
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

    #[tokio::test]
    async fn renders_speed_segments_in_order_and_preserves_existing_audio_on_error() {
        let dir = tempdir();
        let piper = fake_piper(&dir);
        let path = dir.join("joined.wav");
        piper
            .render(
                "de",
                &[(
                    "<speed:1.2>A</speed><speed:0.7>B</speed><speed:1.2>C</speed>".into(),
                    path.clone(),
                )],
            )
            .await
            .unwrap();
        let mut reader = hound::WavReader::open(&path).unwrap();
        let samples = reader
            .samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut expected = vec![65; 133];
        expected.extend(vec![66; 229]);
        expected.extend(vec![67; 133]);
        assert_eq!(samples, expected);
        let log = std::fs::read_to_string(piper.bin.with_extension("log")).unwrap();
        let lines: Vec<serde_json::Value> = log
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert!(
            lines
                .iter()
                .any(|l| (l["scale"].as_f64().unwrap() - 0.9 / 0.7).abs() < 1e-9)
        );
        assert!(
            lines
                .iter()
                .all(|l| !l["text"].as_str().unwrap().contains('<'))
        );
        let original = std::fs::read(&path).unwrap();
        assert!(
            piper
                .render("de", &[("<speed:0.7>bad".into(), path.clone())])
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        // Failure after segment synthesis must also leave the published file.
        std::fs::write(
            piper.voices_dir.join("de_DE-thorsten-medium.onnx.json"),
            "{}",
        )
        .unwrap();
        assert!(
            piper
                .render("de", &[("<speed:0.7>bad</speed>".into(), path.clone())])
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!std::fs::read_dir(&dir).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".tts-")
        }));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_incompatible_wave_segments() {
        let dir = tempdir();
        let mut paths = Vec::new();
        for rate in [8000, 16000] {
            let path = dir.join(format!("{rate}.wav"));
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut w = hound::WavWriter::create(&path, spec).unwrap();
            w.write_sample(1i16).unwrap();
            w.finalize().unwrap();
            paths.push(path);
        }
        assert!(join_wav(&paths, &dir.join("joined.wav")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
