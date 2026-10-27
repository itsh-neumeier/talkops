//! Devices, the job queue and the load limit.
//!
//! Each device (one Edge TPU, the CPU or a mock) has its own OS thread that
//! loads the models and then takes jobs from one shared queue. One device
//! runs one inference at a time. Live jobs (realtime sessions) come before
//! file jobs. The queue has a fixed capacity: a full queue rejects new jobs
//! at once with [EngineError::Overloaded], so callers can fall back instead
//! of waiting. A supervisor task finds new or replugged devices and restarts
//! failed ones.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::oneshot;

use crate::ctc::Matrix;
use crate::devices::DeviceSpec;
use crate::models::Catalog;

/// What a device computes.
#[derive(Debug, Clone)]
pub enum TaskKind {
    /// CTC output of the model of a language.
    Ctc { language: String },
    /// Class probabilities of the language ID model.
    LangId,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub kind: TaskKind,
    /// Front end features, `features x window_frames`, row-major.
    pub features: Vec<f32>,
    /// The window audio at 16 kHz (used by the mock backend only).
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    /// CTC frames x classes.
    Matrix(Matrix),
    /// Language ID probabilities, in the order of the model classes.
    Probs(Vec<f32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Realtime sessions.
    Live,
    /// File transcriptions.
    File,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EngineError {
    #[error("all Edge TPUs are busy; try again later")]
    Overloaded,
    #[error("no Edge TPU is ready")]
    Unavailable,
    #[error("model not available: {0}")]
    UnknownModel(String),
    #[error("inference failed: {0}")]
    Failed(String),
}

/// A device implementation. `serve` runs on the device thread: it loads the
/// models, calls [Port::ready], and then works through [Port::next_task] until it
/// returns `None` or the device fails.
pub trait Backend: Send + Sync + 'static {
    /// `tpu`, `cpu` or `mock`.
    fn name(&self) -> &'static str;
    fn discover(&self) -> Vec<DeviceSpec>;
    fn serve(&self, spec: &DeviceSpec, catalog: &Catalog, port: &mut Port) -> Result<(), String>;
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

    fn pop(&mut self) -> Option<Job> {
        self.live.pop_front().or_else(|| self.file.pop_front())
    }
}

struct Queue {
    jobs: Mutex<Jobs>,
    wake: Condvar,
    capacity: usize,
}

/// The state of one device as `/health` shows it.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceStatus {
    pub id: String,
    pub kind: String,
    pub node: Option<String>,
    /// `starting`, `ready`, `error`, `missing` or `firmware-not-loaded`
    /// (a USB device before its first use, while starting).
    pub state: String,
    pub busy: bool,
    /// Share of time busy over the last seconds (0..1).
    pub load: f32,
    pub inferences: u64,
    /// Loaded model names.
    pub models: Vec<String>,
    pub error: Option<String>,
    #[serde(skip)]
    busy_secs: f64,
    #[serde(skip)]
    last_busy_secs: f64,
    #[serde(skip)]
    running: bool,
    #[serde(skip)]
    stop: Arc<AtomicBool>,
}

/// The connection of a device thread to the queue.
pub struct Port {
    queue: Arc<Queue>,
    devices: Arc<Mutex<Vec<DeviceStatus>>>,
    id: String,
    stop: Arc<AtomicBool>,
    current: Option<(oneshot::Sender<Result<Output, EngineError>>, Instant)>,
}

impl Port {
    fn update(&self, f: impl FnOnce(&mut DeviceStatus)) {
        if let Ok(mut all) = self.devices.lock()
            && let Some(d) = all.iter_mut().find(|d| d.id == self.id)
        {
            f(d);
        }
    }

    /// The models are loaded; the device takes jobs from now on.
    pub fn ready(&mut self, models: Vec<String>) {
        self.update(|d| {
            d.state = "ready".into();
            d.models = models;
            d.error = None;
        });
        tracing::info!(device = %self.id, "device ready");
    }

    /// The next task; blocks. `None` when the device must stop.
    pub fn next_task(&mut self) -> Option<Task> {
        let mut jobs = self.queue.jobs.lock().ok()?;
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return None;
            }
            if let Some(job) = jobs.pop() {
                drop(jobs);
                // A caller that gave up (closed session) needs no answer.
                if job.reply.is_closed() {
                    jobs = self.queue.jobs.lock().ok()?;
                    continue;
                }
                self.current = Some((job.reply, Instant::now()));
                self.update(|d| d.busy = true);
                return Some(job.task);
            }
            jobs = self
                .queue
                .wake
                .wait_timeout(jobs, Duration::from_millis(500))
                .ok()?
                .0;
        }
    }

    /// The result of the task from [Port::next_task].
    pub fn complete(&mut self, result: Result<Output, String>) {
        let Some((reply, started)) = self.current.take() else {
            return;
        };
        let secs = started.elapsed().as_secs_f64();
        self.update(|d| {
            d.busy = false;
            d.inferences += 1;
            d.busy_secs += secs;
        });
        let _ = reply.send(result.map_err(EngineError::Failed));
    }
}

impl Drop for Port {
    fn drop(&mut self) {
        if let Some((reply, _)) = self.current.take() {
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

pub struct Engine {
    pub diagnostics: crate::diagnostics::Diagnostics,
    pub catalog: Arc<Catalog>,
    backend: Arc<dyn Backend>,
    queue: Arc<Queue>,
    devices: Arc<Mutex<Vec<DeviceStatus>>>,
    sessions: AtomicUsize,
    sessions_per_device: usize,
}

/// Holds one realtime session slot.
pub struct SessionGuard(Arc<Engine>);

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.0.sessions.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Engine {
    /// Opens all devices and starts the supervisor (needs a Tokio runtime).
    pub fn start(
        backend: Arc<dyn Backend>,
        catalog: Catalog,
        capacity: usize,
        sessions_per_device: usize,
    ) -> Arc<Self> {
        let engine = Arc::new(Self {
            diagnostics: crate::diagnostics::Diagnostics::default(),
            catalog: Arc::new(catalog),
            backend,
            queue: Arc::new(Queue {
                jobs: Mutex::new(Jobs::default()),
                wake: Condvar::new(),
                capacity: capacity.max(1),
            }),
            devices: Arc::new(Mutex::new(Vec::new())),
            sessions: AtomicUsize::new(0),
            sessions_per_device: sessions_per_device.max(1),
        });
        engine.rescan();
        let weak = Arc::downgrade(&engine);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            let mut n = 0u64;
            loop {
                tick.tick().await;
                let Some(engine) = weak.upgrade() else { return };
                engine.measure_load(5.0);
                n += 1;
                // Look for new, replugged or failed devices every 15 s.
                if n.is_multiple_of(3) {
                    let e = engine.clone();
                    let _ = tokio::task::spawn_blocking(move || e.rescan()).await;
                }
            }
        });
        engine
    }

    fn measure_load(&self, secs: f64) {
        if let Ok(mut all) = self.devices.lock() {
            for d in all.iter_mut() {
                d.load = ((d.busy_secs - d.last_busy_secs) / secs).clamp(0.0, 1.0) as f32;
                d.last_busy_secs = d.busy_secs;
            }
        }
    }

    /// Starts threads for new devices and restarts failed ones.
    pub fn rescan(&self) {
        let specs = self.backend.discover();
        let mut start = Vec::new();
        {
            let Ok(mut all) = self.devices.lock() else {
                return;
            };
            for d in all.iter_mut() {
                if !specs.iter().any(|s| s.id == d.id) {
                    d.state = "missing".into();
                    d.stop.store(true, Ordering::Relaxed);
                }
            }
            for spec in specs {
                match all.iter_mut().find(|d| d.id == spec.id) {
                    Some(d) if d.running => {
                        d.node = spec.node.clone();
                    }
                    Some(d) => {
                        d.node = spec.node.clone();
                        d.state = spec.note.clone().unwrap_or_else(|| "starting".into());
                        d.running = true;
                        d.stop = Arc::new(AtomicBool::new(false));
                        start.push((spec, d.stop.clone()));
                    }
                    None => {
                        let stop = Arc::new(AtomicBool::new(false));
                        all.push(DeviceStatus {
                            id: spec.id.clone(),
                            kind: spec.kind.clone(),
                            node: spec.node.clone(),
                            state: spec.note.clone().unwrap_or_else(|| "starting".into()),
                            busy: false,
                            load: 0.0,
                            inferences: 0,
                            models: Vec::new(),
                            error: None,
                            busy_secs: 0.0,
                            last_busy_secs: 0.0,
                            running: true,
                            stop: stop.clone(),
                        });
                        start.push((spec, stop));
                    }
                }
            }
        }
        for (spec, stop) in start {
            let (backend, catalog) = (self.backend.clone(), self.catalog.clone());
            let (queue, devices) = (self.queue.clone(), self.devices.clone());
            let name = format!("coral-{}", spec.id.replace(':', "-"));
            let spawned = std::thread::Builder::new().name(name).spawn(move || {
                let mut port = Port {
                    queue,
                    devices: devices.clone(),
                    id: spec.id.clone(),
                    stop,
                    current: None,
                };
                let result = backend.serve(&spec, &catalog, &mut port);
                drop(port);
                if let Err(err) = &result {
                    tracing::warn!(device = %spec.id, error = %err, "device failed");
                }
                if let Ok(mut all) = devices.lock()
                    && let Some(d) = all.iter_mut().find(|d| d.id == spec.id)
                {
                    d.running = false;
                    d.busy = false;
                    if let Err(err) = result {
                        d.state = "error".into();
                        d.error = Some(err);
                    } else if d.state != "missing" {
                        d.state = "stopped".into();
                    }
                }
            });
            if let Err(err) = spawned {
                tracing::error!(error = %err, "starting a device thread failed");
            }
        }
    }

    fn ready_devices(&self) -> usize {
        self.devices
            .lock()
            .map(|all| all.iter().filter(|d| d.state == "ready").count())
            .unwrap_or(0)
    }

    /// Runs one task. Fails at once when the queue is full or no device is
    /// ready.
    pub async fn run(&self, task: Task, priority: Priority) -> Result<Output, EngineError> {
        if self.ready_devices() == 0 {
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

    /// Takes a realtime session slot (`sessions per device` x ready devices).
    pub fn try_session(self: &Arc<Self>) -> Option<SessionGuard> {
        let max = self.max_sessions();
        let mut current = self.sessions.load(Ordering::Relaxed);
        loop {
            if current >= max {
                return None;
            }
            match self.sessions.compare_exchange(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Some(SessionGuard(self.clone())),
                Err(now) => current = now,
            }
        }
    }

    pub fn max_sessions(&self) -> usize {
        self.ready_devices() * self.sessions_per_device
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    pub fn devices(&self) -> Vec<DeviceStatus> {
        self.devices.lock().map(|d| d.clone()).unwrap_or_default()
    }

    pub fn queue_status(&self) -> QueueStatus {
        QueueStatus {
            length: self.queue.jobs.lock().map(|j| j.len()).unwrap_or(0),
            capacity: self.queue.capacity,
            sessions: self.sessions.load(Ordering::Relaxed),
            max_sessions: self.max_sessions(),
        }
    }

    /// Stops all device threads (tests and shutdown).
    pub fn shutdown(&self) {
        if let Ok(all) = self.devices.lock() {
            for d in all.iter() {
                d.stop.store(true, Ordering::Relaxed);
            }
        }
        self.queue.wake.notify_all();
    }

    /// Waits until at least one device is ready (or the timeout ends).
    pub async fn wait_ready(&self, timeout: Duration) -> bool {
        let until = Instant::now() + timeout;
        while Instant::now() < until {
            if self.ready_devices() > 0 {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        false
    }
}
