//! Audio clips: generate with a computer voice (preview before saving),
//! upload or record in the browser (sent as WAV), play back.

use axum::Json;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde_json::json;
use talkops_core::audio::{self, Clip, TtsInput, Voice};
use talkops_core::audit;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

/// Largest upload (10 minutes of 16 kHz mono PCM is about 19 MB).
const UPLOAD_LIMIT: usize = 24 * 1024 * 1024;

pub fn router() -> OpenApiRouter<AppState> {
    let upload = OpenApiRouter::new()
        .routes(routes!(upload_clip))
        .layer(DefaultBodyLimit::max(UPLOAD_LIMIT));
    OpenApiRouter::new()
        .routes(routes!(list_voices))
        .routes(routes!(generate_clip))
        .routes(routes!(get_clip))
        .routes(routes!(clip_audio))
        .routes(routes!(list_music))
        .routes(routes!(music_audio))
        .merge(upload)
}

/// Computer voices (two per language).
#[utoipa::path(get, path = "/api/v1/audio/voices", tag = "audio", responses((status = 200, body = [Voice])))]
pub async fn list_voices(_auth: AuthUser) -> Json<&'static [Voice]> {
    Json(audio::VOICES)
}

/// Generates a clip from text. Rendering takes a moment: poll the clip until
/// its status is `ready`, then play it.
#[utoipa::path(post, path = "/api/v1/audio/clips/tts", tag = "audio", request_body = TtsInput, responses((status = 202, body = Clip)))]
pub async fn generate_clip(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<TtsInput>,
) -> ApiResult<(StatusCode, Json<Clip>)> {
    let clip = audio::create_tts(&state.db, auth.tenant, Some(auth.id), &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "generate",
        "audio_clip",
        Some(clip.id.to_string()),
        json!({"language": clip.language, "voice": clip.voice, "chars": clip.text.chars().count()}),
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(clip)))
}

/// Uploads a clip. Multipart fields: `file` (WAV, PCM 8–48 kHz, mono or
/// stereo; the web UI converts other formats and recordings in the browser)
/// and `source` (`upload` or `recording`).
#[utoipa::path(post, path = "/api/v1/audio/clips", tag = "audio", responses((status = 200, body = Clip)))]
pub async fn upload_clip(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> ApiResult<Json<Clip>> {
    let mut data = None;
    let mut source = String::from("upload");
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?
    {
        match field.name() {
            Some("file") => {
                data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| ApiError::BadRequest(e.to_string()))?,
                );
            }
            Some("source") => {
                source = field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(e.to_string()))?;
            }
            _ => {}
        }
    }
    let data = data.ok_or_else(|| ApiError::BadRequest("field `file` is required".into()))?;
    let duration_ms = wav_duration_ms(&data)?;
    let id = Uuid::new_v4();
    let path = state.media.sounds.join(audio::clip_file(auth.tenant, id));
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    let tmp = path.with_extension("upload.wav");
    tokio::fs::write(&tmp, &data)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    tokio::fs::rename(&tmp, &path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    match audio::create_file(
        &state.db,
        auth.tenant,
        Some(auth.id),
        id,
        &source,
        duration_ms,
    )
    .await
    {
        Ok(clip) => {
            audit::record(
                &state.db,
                &auth.actor(),
                "upload",
                "audio_clip",
                Some(clip.id.to_string()),
                json!({"source": clip.source, "duration_ms": clip.duration_ms}),
            )
            .await?;
            Ok(Json(clip))
        }
        Err(err) => {
            let _ = tokio::fs::remove_file(&path).await;
            Err(err.into())
        }
    }
}

/// Validates a WAV upload; returns its length.
fn wav_duration_ms(data: &[u8]) -> ApiResult<i32> {
    let reader = hound::WavReader::new(std::io::Cursor::new(data))
        .map_err(|_| ApiError::BadRequest("not a WAV file".into()))?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int
        || !(8000..=48000).contains(&spec.sample_rate)
        || spec.channels == 0
        || spec.channels > 2
    {
        return Err(ApiError::BadRequest(
            "unsupported WAV format (PCM, 8–48 kHz, mono or stereo)".into(),
        ));
    }
    let ms = i64::from(reader.duration()) * 1000 / i64::from(spec.sample_rate);
    if ms == 0 {
        return Err(ApiError::BadRequest("the recording is empty".into()));
    }
    if ms > audio::MAX_DURATION_MS {
        return Err(ApiError::BadRequest("at most 10 minutes".into()));
    }
    Ok(ms as i32)
}

/// A clip (poll `status` after generating).
#[utoipa::path(get, path = "/api/v1/audio/clips/{id}", tag = "audio", params(("id" = Uuid, Path)), responses((status = 200, body = Clip)))]
pub async fn get_clip(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Clip>> {
    Ok(Json(audio::get(&state.db, auth.tenant, id).await?))
}

/// The clip's audio (WAV).
#[utoipa::path(get, path = "/api/v1/audio/clips/{id}/audio", tag = "audio", params(("id" = Uuid, Path)), responses((status = 200, content_type = "audio/wav", body = Vec<u8>)))]
pub async fn clip_audio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Response> {
    let clip = audio::get(&state.db, auth.tenant, id).await?;
    if clip.status != "ready" {
        return Err(ApiError::NotFound);
    }
    super::voicemail::wav(state.media.sounds.join(audio::clip_file(auth.tenant, id))).await
}

#[derive(serde::Serialize, ToSchema)]
pub struct MusicTrack {
    pub id: &'static str,
    pub title: &'static str,
    /// False until FreeSWITCH has copied the piece into the sounds volume.
    pub available: bool,
}

/// Built-in music on hold.
#[utoipa::path(get, path = "/api/v1/audio/music", tag = "audio", responses((status = 200, body = [MusicTrack])))]
pub async fn list_music(State(state): State<AppState>, _auth: AuthUser) -> Json<Vec<MusicTrack>> {
    Json(
        audio::MUSIC
            .iter()
            .map(|(id, title)| MusicTrack {
                id,
                title,
                available: audio::music_file(id)
                    .is_some_and(|f| state.media.sounds.join(f).is_file()),
            })
            .collect(),
    )
}

/// A built-in piece of music on hold (WAV).
#[utoipa::path(get, path = "/api/v1/audio/music/{id}", tag = "audio", params(("id" = String, Path)), responses((status = 200, content_type = "audio/wav", body = Vec<u8>)))]
pub async fn music_audio(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let file = audio::music_file(&id).ok_or(ApiError::NotFound)?;
    super::voicemail::wav(state.media.sounds.join(file)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(rate: u32, samples: u32) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::new(&mut out, spec).unwrap();
        for _ in 0..samples {
            w.write_sample(0i16).unwrap();
        }
        w.finalize().unwrap();
        out.into_inner()
    }

    #[test]
    fn validates_uploads() {
        assert_eq!(wav_duration_ms(&wav(16000, 8000)).unwrap(), 500);
        assert!(wav_duration_ms(&wav(16000, 0)).is_err(), "empty");
        assert!(wav_duration_ms(&wav(4000, 4000)).is_err(), "rate");
        assert!(wav_duration_ms(b"ID3 not a wav").is_err());
    }
}
