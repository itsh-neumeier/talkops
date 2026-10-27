//! Runs a compiled TFLite model N times on an Edge TPU through the safe
//! `edgefirst-tflite` wrapper and prints latency and CPU time as JSON.
//!
//! ```sh
//! coral-probe --tflite /usr/lib/x86_64-linux-gnu/libtensorflowlite_c.so \
//!     --delegate libedgetpu.so.1 --device usb:0 \
//!     --model models/de/model_edgetpu.tflite --runs 200
//! ```
//!
//! `--device cpu` runs the model without the delegate (reference).

use std::path::PathBuf;
use std::time::Instant;

use edgefirst_tflite::{Delegate, DelegateOptions, Interpreter, Library, Model};

struct Args {
    tflite: Option<PathBuf>,
    delegate: String,
    device: String,
    model: PathBuf,
    runs: usize,
}

fn parse() -> Result<Args, String> {
    let mut a = Args {
        tflite: None,
        delegate: "libedgetpu.so.1".into(),
        device: "usb:0".into(),
        model: PathBuf::new(),
        runs: 100,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--tflite" => a.tflite = Some(value()?.into()),
            "--delegate" => a.delegate = value()?,
            "--device" => a.device = value()?,
            "--model" => a.model = value()?.into(),
            "--runs" => a.runs = value()?.parse().map_err(|e| format!("--runs: {e}"))?,
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    if a.model.as_os_str().is_empty() {
        return Err("--model is required".into());
    }
    Ok(a)
}

/// User plus system CPU seconds of this process, from /proc/self/stat.
fn cpu_secs() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<&str> = stat
        .rsplit_once(") ")
        .map(|(_, rest)| rest.split(' ').collect())
        .unwrap_or_default();
    let ticks = |i: usize| {
        fields
            .get(i)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    // utime and stime are fields 14 and 15; `rest` starts at field 3. Linux uses 100 ticks/s.
    (ticks(11) + ticks(12)) / 100.0
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse()?;
    let lib = match &args.tflite {
        Some(p) => Library::from_path(p)?,
        None => Library::new()?,
    };
    let model = Model::from_file(&lib, &args.model)?;
    let mut builder = Interpreter::builder(&lib)?.num_threads(1);
    if args.device != "cpu" {
        let opts = DelegateOptions::new().option("device", args.device.clone());
        builder = builder.delegate(Delegate::load_with_options(&args.delegate, &opts)?);
    }
    let started = Instant::now();
    let mut interpreter = builder.build(&model)?;
    interpreter.allocate_tensors()?;
    let load_secs = started.elapsed().as_secs_f64();

    // Input stays zero: the probe measures the runtime path, not accuracy.
    let mut times = Vec::with_capacity(args.runs);
    let cpu0 = cpu_secs();
    let wall0 = Instant::now();
    for _ in 0..args.runs {
        let t = Instant::now();
        interpreter.invoke()?;
        times.push(t.elapsed().as_secs_f64());
    }
    let wall = wall0.elapsed().as_secs_f64();
    let cpu = cpu_secs() - cpu0;
    times.sort_by(f64::total_cmp);
    let pick = |q: f64| times[((times.len() - 1) as f64 * q) as usize];
    let out = serde_json::json!({
        "kind": "probe",
        "model": args.model.file_name().map(|n| n.to_string_lossy().into_owned()),
        "device": args.device,
        "runs": args.runs,
        "load_secs": load_secs,
        "invoke_p50_ms": pick(0.5) * 1000.0,
        "invoke_p95_ms": pick(0.95) * 1000.0,
        "cpu_cores_during_invoke": if wall > 0.0 { cpu / wall } else { 0.0 },
    });
    println!("{out}");
    Ok(())
}
