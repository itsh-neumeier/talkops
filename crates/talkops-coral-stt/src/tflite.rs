//! The TensorFlow Lite backend: Edge TPUs through the libedgetpu delegate,
//! or the CPU. Uses the safe `edgefirst-tflite` wrapper, which loads
//! `libtensorflowlite_c.so` and the delegate at run time.

use std::path::{Path, PathBuf};

use edgefirst_tflite::{Delegate, DelegateOptions, Interpreter, Library, Model, TensorType};

use crate::ctc::Matrix;
use crate::devices::{self, DeviceSpec};
use crate::engine::{Backend, Output, Port, Task, TaskKind};
use crate::models::{Catalog, Meta};

pub struct TfliteBackend {
    /// `libtensorflowlite_c.so`; `None` searches the usual paths.
    pub library: Option<PathBuf>,
    /// `libedgetpu.so.1` for Edge TPUs; `None` runs on the CPU.
    pub delegate: Option<PathBuf>,
    /// CPU threads per interpreter.
    pub threads: i32,
    pub sysfs: PathBuf,
    pub dev: PathBuf,
}

struct Slot<'lib> {
    name: String,
    language: Option<String>,
    meta: Meta,
    interpreter: Interpreter<'lib>,
}

fn quantize(values: &[f32], scale: f32, zero: i32, min: i32, max: i32) -> Vec<i32> {
    let scale = if scale == 0.0 { 1.0 } else { scale };
    values
        .iter()
        .map(|v| ((v / scale).round() as i32 + zero).clamp(min, max))
        .collect()
}

/// Puts the features into the layout of the model input.
fn arrange(features: &[f32], meta: &Meta) -> Vec<f32> {
    let frames = meta.window_frames;
    let n = meta.frontend.features;
    if meta.input_layout == "ntc" {
        let mut out = vec![0.0; features.len()];
        for f in 0..n {
            for t in 0..frames {
                out[t * n + f] = features[f * frames + t];
            }
        }
        out
    } else {
        features.to_vec()
    }
}

fn run(slot: &mut Slot<'_>, task: &Task) -> Result<Output, String> {
    let err = |e: edgefirst_tflite::Error| e.to_string();
    let input = arrange(&task.features, &slot.meta);
    {
        let mut inputs = slot.interpreter.inputs_mut().map_err(err)?;
        let tensor = inputs.first_mut().ok_or("model without input")?;
        let q = tensor.quantization_params();
        match tensor.tensor_type() {
            TensorType::Float32 => tensor.copy_from_slice(&input).map_err(err)?,
            TensorType::Int8 => {
                let v: Vec<i8> = quantize(&input, q.scale, q.zero_point, -128, 127)
                    .into_iter()
                    .map(|x| x as i8)
                    .collect();
                tensor.copy_from_slice(&v).map_err(err)?
            }
            TensorType::UInt8 => {
                let v: Vec<u8> = quantize(&input, q.scale, q.zero_point, 0, 255)
                    .into_iter()
                    .map(|x| x as u8)
                    .collect();
                tensor.copy_from_slice(&v).map_err(err)?
            }
            other => return Err(format!("unsupported input type {other:?}")),
        }
    }
    slot.interpreter.invoke().map_err(err)?;
    let outputs = slot.interpreter.outputs().map_err(err)?;
    let tensor = outputs.first().ok_or("model without output")?;
    let q = tensor.quantization_params();
    let deq = |v: f32| (v - q.zero_point as f32) * q.scale;
    let values: Vec<f32> = match tensor.tensor_type() {
        TensorType::Float32 => tensor.as_slice::<f32>().map_err(err)?.to_vec(),
        TensorType::Int8 => tensor
            .as_slice::<i8>()
            .map_err(err)?
            .iter()
            .map(|v| deq(f32::from(*v)))
            .collect(),
        TensorType::UInt8 => tensor
            .as_slice::<u8>()
            .map_err(err)?
            .iter()
            .map(|v| deq(f32::from(*v)))
            .collect(),
        other => return Err(format!("unsupported output type {other:?}")),
    };
    match task.kind {
        TaskKind::LangId => Ok(Output::Probs(values)),
        TaskKind::Ctc { .. } => {
            let shape = tensor.shape().map_err(err)?;
            let classes = slot.meta.classes_with_blank();
            let dims: Vec<usize> = shape.into_iter().filter(|d| *d != 1).collect();
            let m = match dims.as_slice() {
                [rows, cols] if *cols == classes => Matrix::new(*rows, *cols, values),
                [rows, cols] if *rows == classes => Matrix::new(*rows, *cols, values).transposed(),
                _ => return Err(format!("unexpected output shape {dims:?}")),
            };
            Ok(Output::Matrix(m))
        }
    }
}

fn open_library(path: Option<&Path>) -> Result<Library, String> {
    match path {
        Some(p) => Library::from_path(p),
        None => Library::new(),
    }
    .map_err(|e| format!("TensorFlow Lite library: {e}"))
}

impl Backend for TfliteBackend {
    fn name(&self) -> &'static str {
        if self.delegate.is_some() {
            "tpu"
        } else {
            "cpu"
        }
    }

    fn discover(&self) -> Vec<DeviceSpec> {
        if self.delegate.is_some() {
            devices::discover(&self.sysfs, &self.dev)
        } else {
            vec![DeviceSpec {
                id: "cpu".into(),
                kind: "cpu".into(),
                node: None,
                note: None,
            }]
        }
    }

    fn serve(&self, spec: &DeviceSpec, catalog: &Catalog, port: &mut Port) -> Result<(), String> {
        let lib = open_library(self.library.as_deref())?;
        let mut models = Vec::new();
        let mut slots = Vec::new();
        for file in catalog.files() {
            let model = Model::from_file(&lib, &file.path)
                .map_err(|e| format!("{}: {e}", file.path.display()))?;
            let mut builder = Interpreter::builder(&lib)
                .map_err(|e| e.to_string())?
                .num_threads(self.threads);
            if let Some(delegate) = &self.delegate {
                let options = DelegateOptions::new().option("device", spec.id.clone());
                let d = Delegate::load_with_options(delegate, &options)
                    .map_err(|e| format!("Edge TPU {}: {e}", spec.id))?;
                builder = builder.delegate(d);
            }
            let mut interpreter = builder
                .build(&model)
                .map_err(|e| format!("{}: {e}", file.name))?;
            interpreter
                .allocate_tensors()
                .map_err(|e| format!("{}: {e}", file.name))?;
            models.push(model);
            slots.push(Slot {
                name: file.name.clone(),
                language: file.language.clone(),
                meta: file.meta.clone(),
                interpreter,
            });
        }
        port.ready(slots.iter().map(|s| s.name.clone()).collect());
        while let Some(task) = port.next_task() {
            let slot = slots.iter_mut().find(|s| match &task.kind {
                TaskKind::Ctc { language } => s.language.as_ref() == Some(language),
                TaskKind::LangId => s.name == "langid",
            });
            let Some(slot) = slot else {
                port.complete(Err("model not loaded".into()));
                continue;
            };
            let result = run(slot, &task);
            let failed = result.as_ref().err().cloned();
            port.complete(result);
            // An inference error on a TPU usually means the device is gone.
            if let Some(err) = failed
                && self.delegate.is_some()
            {
                return Err(err);
            }
        }
        drop(slots);
        drop(models);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantizes_with_clamp() {
        assert_eq!(
            quantize(&[0.0, 1.0, -100.0, 100.0], 0.5, 3, -128, 127),
            [3, 5, -128, 127]
        );
    }

    #[test]
    fn arranges_time_major() {
        let mut meta = crate::mock::catalog(false).ctc[0].meta.clone();
        meta.window_frames = 3;
        meta.frontend.features = 2;
        meta.input_layout = "ntc".into();
        assert_eq!(
            arrange(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &meta),
            [1.0, 4.0, 2.0, 5.0, 3.0, 6.0]
        );
    }
}
