//! The GPU, the job queue and the load limit.
//!
//! One process uses one GPU (ADR 0052). A device thread loads the selected
//! models and then takes jobs from one queue. Live jobs (realtime sessions)
//! come before file jobs. Waiting jobs for the same model go to the GPU in
//! one batch. The queue has a fixed capacity: a full queue rejects new jobs
//! at once with [EngineError::Overloaded], so callers can fall back instead
//! of waiting. A supervisor starts the device thread again after a failure.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use talkops_stt_service::audio::MODEL_RATE;
use talkops_stt_service::diagnostics::Diagnostics;
use talkops_stt_service::{EngineError, Priority, Word};
use tokio::sync::oneshot;

use crate::gpu::Gpu;
use crate::models::Catalog;

/// What the GPU computes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskKind {
    /// Speech recognition with the model of a language slot.
    Decode { language: String },
    /// Spoken language identification with the `langid` model.
    LangId,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub kind: TaskKind,
    /// Audio at 16 kHz.
    pub samples: Vec<f32>,
}

/// The text of one decode.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Hypothesis {
    pub text: String,
    /// Word times from the start of the audio (empty if the model has none).
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    Text(Hypothesis),
    /// ISO 639-1 code.
    Language(String),
}

/// A model as the device loaded it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LoadedModel {
    pub slot: String,
    pub id: String,
    pub name: String,
    pub family: crate::models::Family,
    /// `ready` or `error`.
    pub state: &'static str,
    /// VRAM in use after the load minus before (NVML), MiB.
    pub vram_mib: Option<u64>,
    pub load_secs: f32,
    pub error: Option<String>,
}

/// A device implementation. `serve` runs on the device thread: it loads the
/// models, calls [Port::ready], and then works through [Port::next_batch]
/// until it returns `None` or the device fails.
pub trait Backend: Send + Sync + 'static {
    /// `cuda`, `cpu` or `mock`.
    fn name(&self) -> &'static str;
    fn serve(&self, catalog: &Catalog, gpu: Option<&Gpu>, port: &mut Port) -> Result<(), String>;
}

struct Job {
    task: Task,
    reply: oneshot::Sender<Result<Output, EngineError>>,
}

#[derive(Default)]
struct Jobs {
    live: VecDeque<Job>,
    file: VecDeque<Job>,
}

impl Jobs {
    fn len(&self) -> usize {
        self.live.len() + self.file.len()
    }

    /// The first job and up to `max - 1` further jobs of the same kind and
    /// priority. Jobs whose caller gave up are dropped.
    fn pop_batch(&mut self, max: usize) -> Vec<Job> {
        for queue in [&mut self.live, &mut self.file] {
            queue.retain(|j| !j.reply.is_closed());
            let Some(first) = queue.pop_front() else {
                continue;
            };
            let kind = first.task.kind.clone();
            let mut batch = vec![first];
            let mut i = 0;
            while batch.len() < max && i < queue.len() {
                if queue[i].task.kind == kind {
                    batch.extend(queue.remove(i));
                } else {
                    i += 1;
                }
            }
            return batch;
        }
        Vec::new()
    }
}

struct Queue {
    jobs: Mutex<Jobs>,
    wake: Condvar,
    capacity: usize,
}

/// The state of the device as `/health` shows it.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceStatus {
    pub id: String,
    /// `starting`, `ready`, `error` or `stopped`.
    pub state: String,
    pub busy: bool,
    /// Share of time busy over the last seconds (0..1).
    pub load: f32,
    pub inferences: u64,
    pub batches: u64,
    pub models: Vec<LoadedModel>,
    pub error: Option<String>,
    #[serde(skip)]
    busy_secs: f64,
    #[serde(skip)]
    last_busy_secs: f64,
    #[serde(skip)]
    running: bool,
}

/// The connection of the device thread to the queue.
pub struct Port {
    queue: Arc<Queue>,
    status: Arc<Mutex<DeviceStatus>>,
    stop: Arc<AtomicBool>,
    batch: usize,
    current: Vec<oneshot::Sender<Result<Output, EngineError>>>,
    started: Option<Instant>,
}

impl Port {
    fn update(&self, f: impl FnOnce(&mut DeviceStatus)) {
        if let Ok(mut status) = self.status.lock() {
            f(&mut status);
        }
    }

    /// The models are loaded (or failed); the device takes jobs from now on.
    pub fn ready(&mut self, models: Vec<LoadedModel>) {
        tracing::info!(models = ?models.iter().map(|m| (&m.slot, &m.name, m.state)).collect::<Vec<_>>(), "device ready");
        self.update(|d| {
            d.state = "ready".into();
            d.models = models;
            d.error = None;
        });
    }

    /// The next tasks of one kind; blocks. `None` when the device must stop.
    pub fn next_batch(&mut self) -> Option<Vec<Task>> {
        let mut jobs = self.queue.jobs.lock().ok()?;
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return None;
            }
            let batch = jobs.pop_batch(self.batch);
            if !batch.is_empty() {
                drop(jobs);
                let (tasks, replies): (Vec<_>, Vec<_>) =
                    batch.into_iter().map(|j| (j.task, j.reply)).unzip();
                self.current = replies;
                self.started = Some(Instant::now());
                self.update(|d| d.busy = true);
                return Some(tasks);
            }
            jobs = self
                .queue
                .wake
                .wait_timeout(jobs, Duration::from_millis(500))
                .ok()?
                .0;
        }
    }

    /// The results of the tasks from [Port::next_batch], in the same order.
    pub fn complete(&mut self, results: Vec<Result<Output, String>>) {
        let secs = self
            .started
            .take()
            .map_or(0.0, |s| s.elapsed().as_secs_f64());
        let replies = std::mem::take(&mut self.current);
        let count = replies.len() as u64;
        self.update(|d| {
            d.busy = false;
            d.inferences += count;
            d.batches += 1;
            d.busy_secs += secs;
        });
        let mut results = results.into_iter();
        for reply in replies {
            let result = results
                .next()
                .unwrap_or_else(|| Err("no result".into()))
                .map_err(EngineError::Failed);
            let _ = reply.send(result);
        }
    }
}

impl Drop for Port {
    fn drop(&mut self) {
        for reply in self.current.drain(..) {
            let _ = reply.send(Err(EngineError::Failed("device stopped".into())));
        }
    }
}

/// Queue and session state as `/health` shows it.
#[derive(Debug, Clone, Serialize)]
pub struct QueueStatus {
    pub length: usize,
    pub capacity: usize,
    pub sessions: usize,
    pub max_sessions: usize,
}

pub struct Settings {
    /// Jobs that can wait; more are rejected (HTTP 503).
    pub queue: usize,
    /// Realtime sessions (one per call channel) while the device is ready.
    pub sessions: usize,
    /// Most jobs in one GPU batch.
    pub batch: usize,
    /// A live turn decodes its audio again after this time of new audio.
    pub partial_interval: Duration,
    /// A longer turn is cut (see [crate::recognizer]).
    pub max_turn: Duration,
}

pub struct Engine {
    pub diagnostics: Diagnostics,
    pub catalog: Arc<Catalog>,
    pub gpu: Option<Arc<Gpu>>,
    backend: Arc<dyn Backend>,
    queue: Arc<Queue>,
    status: Arc<Mutex<DeviceStatus>>,
    stop: Arc<AtomicBool>,
    sessions: AtomicUsize,
    max_sessions: usize,
    batch: usize,
    /// [Settings::partial_interval] in samples at 16 kHz.
    pub partial_samples: usize,
    /// [Settings::max_turn] in samples at 16 kHz.
    pub max_turn_samples: usize,
}

fn samples_of(duration: Duration) -> usize {
    (duration.as_secs_f64() * f64::from(MODEL_RATE)) as usize
}

/// Holds one realtime session slot.
pub struct SessionGuard(Arc<Engine>);

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.0.sessions.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Engine {
    /// Starts the device thread and the supervisor (needs a Tokio runtime).
    pub fn start(
        backend: Arc<dyn Backend>,
        catalog: Catalog,
        gpu: Option<Gpu>,
        settings: Settings,
    ) -> Arc<Self> {
        let engine = Arc::new(Self {
            diagnostics: Diagnostics::default(),
            catalog: Arc::new(catalog),
            gpu: gpu.map(Arc::new),
            backend,
            queue: Arc::new(Queue {
                jobs: Mutex::new(Jobs::default()),
                wake: Condvar::new(),
                capacity: settings.queue.max(1),
            }),
            status: Arc::new(Mutex::new(DeviceStatus {
                id: "gpu:0".into(),
                state: "starting".into(),
                busy: false,
                load: 0.0,
                inferences: 0,
                batches: 0,
                models: Vec::new(),
                error: None,
                busy_secs: 0.0,
                last_busy_secs: 0.0,
                running: false,
            })),
            stop: Arc::new(AtomicBool::new(false)),
            sessions: AtomicUsize::new(0),
            max_sessions: settings.sessions.max(1),
            batch: settings.batch.max(1),
            partial_samples: samples_of(settings.partial_interval).max(1),
            max_turn_samples: samples_of(settings.max_turn).max(10 * MODEL_RATE as usize),
        });
        engine.start_device();
        let weak = Arc::downgrade(&engine);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            let mut n = 0u64;
            loop {
                tick.tick().await;
                let Some(engine) = weak.upgrade() else { return };
                engine.measure_load(5.0);
                n += 1;
                // Start a failed device again every 15 s.
                if n.is_multiple_of(3) {
                    engine.start_device();
                }
            }
        });
        engine
    }

    fn measure_load(&self, secs: f64) {
        if let Ok(mut d) = self.status.lock() {
            d.load = ((d.busy_secs - d.last_busy_secs) / secs).clamp(0.0, 1.0) as f32;
            d.last_busy_secs = d.busy_secs;
        }
    }

    /// Starts the device thread unless it runs or the engine stops.
    fn start_device(&self) {
        if self.stop.load(Ordering::Relaxed) {
            return;
        }
        {
            let Ok(mut d) = self.status.lock() else {
                return;
            };
            if d.running {
                return;
            }
            d.running = true;
            if d.state != "error" {
                d.state = "starting".into();
            }
        }
        let (backend, catalog, gpu) =
            (self.backend.clone(), self.catalog.clone(), self.gpu.clone());
        let (queue, status, stop) = (self.queue.clone(), self.status.clone(), self.stop.clone());
        let batch = self.batch;
        let spawned = std::thread::Builder::new()
            .name("cuda-device".into())
            .spawn(move || {
                let mut port = Port {
                    queue,
                    status: status.clone(),
                    stop,
                    batch,
                    current: Vec::new(),
                    started: None,
                };
                let result = backend.serve(&catalog, gpu.as_deref(), &mut port);
                drop(port);
                if let Err(err) = &result {
                    tracing::warn!(error = %err, "device failed");
                }
                if let Ok(mut d) = status.lock() {
                    d.running = false;
                    d.busy = false;
                    match result {
                        Err(err) => {
                            d.state = "error".into();
                            d.error = Some(err);
                        }
                        Ok(()) => d.state = "stopped".into(),
                    }
                }
            });
        if let Err(err) = spawned {
            tracing::error!(error = %err, "starting the device thread failed");
            if let Ok(mut d) = self.status.lock() {
                d.running = false;
            }
        }
    }

    pub fn ready(&self) -> bool {
        self.status.lock().is_ok_and(|d| d.state == "ready")
    }

    /// Slots whose model loaded.
    pub fn loaded_slots(&self) -> Vec<String> {
        self.status
            .lock()
            .map(|d| {
                d.models
                    .iter()
                    .filter(|m| m.state == "ready")
                    .map(|m| m.slot.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Runs one task. Fails at once when the queue is full or the device is
    /// not ready.
    pub async fn run(&self, task: Task, priority: Priority) -> Result<Output, EngineError> {
        if !self.ready() {
            return Err(EngineError::Unavailable);
        }
        let (tx, rx) = oneshot::channel();
        {
            let mut jobs = self
                .queue
                .jobs
                .lock()
                .map_err(|_| EngineError::Unavailable)?;
            if jobs.len() >= self.queue.capacity {
                return Err(EngineError::Overloaded);
            }
            let job = Job { task, reply: tx };
            match priority {
                Priority::Live => jobs.live.push_back(job),
                Priority::File => jobs.file.push_back(job),
            }
        }
        self.queue.wake.notify_one();
        rx.await
            .unwrap_or_else(|_| Err(EngineError::Failed("device stopped".into())))
    }

    /// Takes a realtime session slot.
    pub fn try_session(self: &Arc<Self>) -> Result<SessionGuard, EngineError> {
        let max = self.max_sessions();
        if max == 0 {
            return Err(EngineError::Unavailable);
        }
        let mut current = self.sessions.load(Ordering::Relaxed);
        loop {
            if current >= max {
                return Err(EngineError::Overloaded);
            }
            match self.sessions.compare_exchange(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(SessionGuard(self.clone())),
                Err(now) => current = now,
            }
        }
    }

    pub fn max_sessions(&self) -> usize {
        if self.ready() && !self.loaded_languages().is_empty() {
            self.max_sessions
        } else {
            0
        }
    }

    /// Language slots with a loaded model, in the order of [crate::models::LANGUAGES].
    pub fn loaded_languages(&self) -> Vec<String> {
        let loaded = self.loaded_slots();
        crate::models::LANGUAGES
            .iter()
            .filter(|l| loaded.iter().any(|s| s == *l))
            .map(|l| (*l).to_owned())
            .collect()
    }

    pub fn has_langid(&self) -> bool {
        self.loaded_slots().iter().any(|s| s == "langid")
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    pub fn device(&self) -> Option<DeviceStatus> {
        self.status.lock().ok().map(|d| d.clone())
    }

    pub fn queue_status(&self) -> QueueStatus {
        QueueStatus {
            length: self.queue.jobs.lock().map(|j| j.len()).unwrap_or(0),
            capacity: self.queue.capacity,
            sessions: self.sessions.load(Ordering::Relaxed),
            max_sessions: self.max_sessions(),
        }
    }

    /// Stops the device thread (tests and shutdown).
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.queue.wake.notify_all();
    }

    /// Waits until the device is ready (or the timeout ends).
    pub async fn wait_ready(&self, timeout: Duration) -> bool {
        let until = Instant::now() + timeout;
        while Instant::now() < until {
            if self.ready() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(language: &str) -> (Job, oneshot::Receiver<Result<Output, EngineError>>) {
        let (reply, rx) = oneshot::channel();
        let task = Task {
            kind: TaskKind::Decode {
                language: language.into(),
            },
            samples: Vec::new(),
        };
        (Job { task, reply }, rx)
    }

    #[test]
    fn batches_take_live_jobs_first_and_one_kind() {
        let mut jobs = Jobs::default();
        let mut receivers = Vec::new();
        for (lang, live) in [
            ("de", false),
            ("de", true),
            ("en", true),
            ("de", true),
            ("de", true),
        ] {
            let (j, rx) = job(lang);
            receivers.push(rx);
            if live {
                jobs.live.push_back(j);
            } else {
                jobs.file.push_back(j);
            }
        }
        let batch = jobs.pop_batch(2);
        assert_eq!(batch.len(), 2);
        assert!(batch.iter().all(|j| j.task.kind
            == TaskKind::Decode {
                language: "de".into()
            }));
        assert_eq!(jobs.live.len(), 2, "en and the third live de job wait");
        let batch = jobs.pop_batch(8);
        assert_eq!(
            batch[0].task.kind,
            TaskKind::Decode {
                language: "en".into()
            }
        );
        assert_eq!(batch.len(), 1);
        assert_eq!(jobs.pop_batch(8).len(), 1);
        assert_eq!(jobs.pop_batch(8).len(), 1, "then the file job");
        assert!(jobs.pop_batch(8).is_empty());
    }

    #[test]
    fn closed_jobs_are_dropped() {
        let mut jobs = Jobs::default();
        let (j, rx) = job("de");
        jobs.live.push_back(j);
        drop(rx);
        assert!(jobs.pop_batch(4).is_empty());
    }
}
