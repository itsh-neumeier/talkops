//! GPU values through NVML (`libnvidia-ml.so.1`, mounted by the NVIDIA
//! Container Toolkit). NVML is optional: without it, `/health` shows no GPU
//! values and the service still works.
//!
//! NVML does not apply `CUDA_VISIBLE_DEVICES`. Compose gives the container
//! only the selected GPU (`device_ids`), so index 0 is that GPU.

use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use serde::Serialize;

pub struct Gpu {
    nvml: Nvml,
    index: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GpuStatus {
    pub index: u32,
    pub name: Option<String>,
    pub driver: Option<String>,
    pub memory_total_mib: Option<u64>,
    pub memory_used_mib: Option<u64>,
    /// GPU use in percent over the last sample period of the driver.
    pub utilization_percent: Option<u32>,
    pub temperature_c: Option<u32>,
}

const MIB: u64 = 1024 * 1024;

impl Gpu {
    /// `None` if NVML is not available.
    pub fn open(index: u32) -> Option<Self> {
        match Nvml::init() {
            Ok(nvml) => Some(Self { nvml, index }),
            Err(err) => {
                tracing::warn!(error = %err, "NVML not available; no GPU values in /health");
                None
            }
        }
    }

    /// VRAM in use, MiB.
    pub fn used_mib(&self) -> Option<u64> {
        let device = self.nvml.device_by_index(self.index).ok()?;
        device.memory_info().ok().map(|m| m.used / MIB)
    }

    pub fn status(&self) -> GpuStatus {
        let device = self.nvml.device_by_index(self.index).ok();
        let memory = device.as_ref().and_then(|d| d.memory_info().ok());
        GpuStatus {
            index: self.index,
            name: device.as_ref().and_then(|d| d.name().ok()),
            driver: self.nvml.sys_driver_version().ok(),
            memory_total_mib: memory.as_ref().map(|m| m.total / MIB),
            memory_used_mib: memory.as_ref().map(|m| m.used / MIB),
            utilization_percent: device
                .as_ref()
                .and_then(|d| d.utilization_rates().ok())
                .map(|u| u.gpu),
            temperature_c: device
                .as_ref()
                .and_then(|d| d.temperature(TemperatureSensor::Gpu).ok()),
        }
    }
}
