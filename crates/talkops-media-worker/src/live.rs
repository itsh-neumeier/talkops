//! Live previews of active calls (ADR 0022, 0024). Reads bounded PCM from
//! the growing recording without relying on an unfinished WAV data length.
//!
//! A Coral or CUDA realtime preview (ADR 0046, 0054) that failed in the last 30 s runs
//! local Whisper `base` in parts instead; after the backoff the next job
//! tries the stream again.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, ensure};
use talkops_core::transcription_profiles::{self as profiles, Engine, Purpose};
use talkops_core::{jobs::Job, live_transcripts, settings};
use uuid::Uuid;

use crate::{Ctx, realtime, transcribe};

/// Complete PCM16 stereo frames `start..end` of a growing recording.
pub(crate) struct Pcm {
    pub start: u64,
    pub end: u64,
    pub rate: u32,
    /// Interleaved caller/called samples.
    pub samples: Vec<i16>,
}

/// FreeSWITCH writes PCM16 stereo. Read only complete frames already on
/// disk, at least `min` and at most `max` seconds from `cursor`. A negative
/// cursor starts eight seconds before the end.
pub(crate) fn read_pcm(
    source: &Path,
    cursor: i64,
    min: f32,
    max: f32,
) -> anyhow::Result<Option<Pcm>> {
    let mut file = std::fs::File::open(source)?;
    let size = file.metadata()?.len();
    let mut header = [0u8; 12];
    file.read_exact(&mut header)?;
    ensure!(
        &header[..4] == b"RIFF" && &header[8..] == b"WAVE",
        "unsupported WAV header"
    );
    let mut format = None;
    let data = loop {
        ensure!(file.stream_position()? <= 65536, "WAV header too large");
        let mut chunk = [0u8; 8];
        file.read_exact(&mut chunk)?;
        let len = u32::from_le_bytes(chunk[4..8].try_into()?);
        if &chunk[..4] == b"data" {
            break file.stream_position()?;
        }
        ensure!(len <= 65536, "WAV header chunk too large");
        if &chunk[..4] == b"fmt " {
            ensure!(len >= 16, "short WAV format");
            let mut fmt = [0u8; 16];
            file.read_exact(&mut fmt)?;
            ensure!(
                u16::from_le_bytes(fmt[0..2].try_into()?) == 1
                    && u16::from_le_bytes(fmt[14..16].try_into()?) == 16,
                "live preview requires PCM16"
            );
            let channels = u16::from_le_bytes(fmt[2..4].try_into()?);
            let rate = u32::from_le_bytes(fmt[4..8].try_into()?);
            ensure!(
                channels == 2 && (8000..=48000).contains(&rate),
                "live preview requires telephone stereo audio"
            );
            ensure!(
                u16::from_le_bytes(fmt[12..14].try_into()?) == channels * 2,
                "invalid WAV frame size"
            );
            format = Some((channels, rate));
            file.seek(SeekFrom::Current(i64::from(len - 16 + len % 2)))?;
        } else {
            file.seek(SeekFrom::Current(i64::from(len + len % 2)))?;
        }
    };
    let (channels, rate) = format.context("WAV without format")?;
    let frame_bytes = u64::from(channels * 2);
    let secs = |s: f32| (s * rate as f32) as u64;
    // Leave 500 ms behind the writer. Never read or allocate the whole recording.
    let frames = size.saturating_sub(data) / frame_bytes;
    let available = frames.saturating_sub(u64::from(rate) / 2);
    let start = if cursor < 0 {
        available.saturating_sub(u64::from(rate) * 8)
    } else {
        cursor as u64
    };
    ensure!(start <= frames, "recording was truncated");
    if available.saturating_sub(start) < secs(min).max(1) {
        return Ok(None);
    }
    let end = available.min(start + secs(max));
    file.seek(SeekFrom::Start(data + start * frame_bytes))?;
    let mut bytes = vec![0; ((end - start) * frame_bytes) as usize];
    file.read_exact(&mut bytes)?;
    let samples = bytes
        .chunks_exact(2)
        .map(|s| i16::from_le_bytes([s[0], s[1]]))
        .collect();
    Ok(Some(Pcm {
        start,
        end,
        rate,
        samples,
    }))
}

/// An eight-second WAV snapshot (two seconds at least) for one recognition.
fn snapshot(source: &Path, output: &Path, cursor: i64) -> anyhow::Result<Option<Pcm>> {
    let Some(pcm) = read_pcm(source, cursor, 2.0, 8.0)? else {
        return Ok(None);
    };
    let mut writer = hound::WavWriter::create(
        output,
        hound::WavSpec {
            channels: 2,
            sample_rate: pcm.rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )?;
    for sample in &pcm.samples {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    Ok(Some(pcm))
}

/// Runs one preview step: a snapshot with a local model or a cloud API, or
/// (Realtime API) starts a stream that keeps running in the background.
pub async fn run(ctx: &Ctx, job: &Job) -> anyhow::Result<()> {
    #[derive(serde::Deserialize)]
    struct Payload {
        preview_id: Uuid,
    }
    let payload: Payload = serde_json::from_value(job.payload.clone())?;
    let Some(work) = live_transcripts::work(&ctx.pool, job.tenant_id, payload.preview_id).await?
    else {
        return Ok(());
    };
    let settings = settings::get(&ctx.pool, job.tenant_id).await?;
    if !settings.transcription_enabled {
        return Ok(());
    }
    let root = ctx.recordings_dir.canonicalize()?;
    let source = root.join(&work.file).canonicalize()?;
    ensure!(source.starts_with(&root), "recording outside media volume");
    let language = talkops_core::prompts::language(&settings.default_language);
    let mut engine = profiles::engine(&ctx.pool, job.tenant_id, Purpose::Live)
        .await?
        .unwrap_or(Engine::Local(settings::TranscriptionQuality::Base));
    let mut fallback_from = None;
    if let Engine::Cloud {
        provider_id,
        model,
        realtime: true,
        coral_stable_preview,
        price,
        coral,
        ..
    } = &engine
    {
        let cfg = crate::cloud_config(ctx, job.tenant_id, *provider_id, model).await?;
        let failed = *coral
            && live_transcripts::stream_failed_recently(
                &ctx.pool,
                job.tenant_id,
                payload.preview_id,
            )
            .await?;
        if failed {
            fallback_from = Some(cfg.label());
            engine = Engine::Local(settings::TranscriptionQuality::Base);
            // Show the fallback at once, also before new audio arrives.
            live_transcripts::set_engine(
                &ctx.pool,
                job.tenant_id,
                payload.preview_id,
                "whisper:base",
                fallback_from.as_deref(),
            )
            .await?;
        } else {
            return realtime::start(realtime::Start {
                pool: ctx.pool.clone(),
                worker_id: ctx.worker_id.clone(),
                streams: ctx.streams.clone(),
                tenant: job.tenant_id,
                preview: payload.preview_id,
                source,
                cfg,
                language: language.to_owned(),
                vocabulary: settings.transcription_vocabulary,
                price: *price,
                coral_stable_preview: *coral_stable_preview,
            })
            .await;
        }
    }
    let temporary = transcribe::tempdir()?;
    let output = temporary.path.join("preview.wav");
    let (copy, path, cursor) = (source.clone(), output.clone(), work.cursor_frames);
    let Some(chunk) = tokio::task::spawn_blocking(move || snapshot(&copy, &path, cursor)).await??
    else {
        return Ok(());
    };
    let done = tokio::time::timeout(
        Duration::from_secs(60),
        crate::run_engine(
            ctx,
            job.tenant_id,
            Purpose::Live,
            engine,
            &output,
            language,
            &settings.transcription_vocabulary,
        ),
    )
    .await
    .context("live recognition timed out")??;
    let fallback_from = fallback_from.or(done.fallback_from);
    live_transcripts::set_engine(
        &ctx.pool,
        job.tenant_id,
        payload.preview_id,
        &done.engine,
        fallback_from.as_deref(),
    )
    .await?;
    let mut segments = done.segments;
    let offset = chunk.start as f32 / chunk.rate as f32;
    let end = chunk.end as f32 / chunk.rate as f32;
    segments.retain(|s| {
        !s.text.trim().is_empty()
            && s.start.is_finite()
            && s.end.is_finite()
            && s.start >= 0.0
            && s.end >= s.start
            && s.start < end - offset
    });
    for segment in &mut segments {
        segment.start += offset;
        segment.end = (segment.end + offset).min(end);
    }
    let mut previous = work.segments.0;
    previous.extend(segments);
    previous.sort_by(|a, b| a.start.total_cmp(&b.start));
    live_transcripts::save(
        &ctx.pool,
        job.tenant_id,
        payload.preview_id,
        work.cursor_frames,
        chunk.end as i64,
        end,
        previous,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_growing_stereo_and_bounds_the_window() {
        let dir = transcribe::tempdir().unwrap();
        let source = dir.path.join("growing.wav");
        let output = dir.path.join("chunk.wav");
        let mut writer = hound::WavWriter::create(
            &source,
            hound::WavSpec {
                channels: 2,
                sample_rate: 8000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..8000 * 30 {
            writer.write_sample(1000i16).unwrap();
            writer.write_sample(-1000i16).unwrap();
        }
        writer.flush().unwrap();
        // Simulate an active FreeSWITCH file with an unfinalized data length.
        let mut bytes = std::fs::read(&source).unwrap();
        bytes[40..44].copy_from_slice(&0u32.to_le_bytes());
        std::fs::write(&source, bytes).unwrap();
        let chunk = snapshot(&source, &output, -1).unwrap().unwrap();
        assert_eq!(chunk.end - chunk.start, 8000 * 8);
        assert_eq!(chunk.start, 8000 * 21 + 4000);
        let samples: Vec<i16> = hound::WavReader::open(&output)
            .unwrap()
            .samples()
            .map(Result::unwrap)
            .collect();
        assert_eq!(&samples[..4], &[1000, -1000, 1000, -1000]);
        assert!(
            snapshot(&source, &output, chunk.end as i64)
                .unwrap()
                .is_none()
        );
        assert_eq!(snapshot(&source, &output, 0).unwrap().unwrap().end, 64000);
        assert!(snapshot(&source, &output, 8000 * 40).is_err());
    }

    #[test]
    fn rejects_invalid_audio() {
        let dir = transcribe::tempdir().unwrap();
        let source = dir.path.join("invalid.wav");
        std::fs::write(&source, b"not audio").unwrap();
        assert!(snapshot(&source, &dir.path.join("chunk.wav"), -1).is_err());
    }
}
