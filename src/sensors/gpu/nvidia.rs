use anyhow::{Context, Result, anyhow};
use log::{debug, info, warn};
use nvml_wrapper::{Device, Nvml, error::NvmlError};

use std::sync::OnceLock;

use crate::sensors::{gpu::GpuType, gpus::Gpu};

static NVML: OnceLock<Nvml> = OnceLock::new();

fn nvml() -> Result<&'static Nvml, NvmlError> {
    if let Some(nvml) = NVML.get() {
        return Ok(nvml);
    }

    let nvml = Nvml::init();

    match nvml {
        Ok(nvml_instance) => {
            debug!("Successfully connected to NVML");

            Ok(NVML.get_or_init(|| nvml_instance))
        }

        Err(error) => {
            warn!("Connection to NVML failed, reason: {error}");
            Err(error)
        }
    }
}

pub struct NvidiaGpu<'a> {
    // Index returned by NVML
    pub index: u32,
    pub name: String,
    pub uuid: String,
    vram_total: u64,
    device: Option<Device<'a>>,
}

impl NvidiaGpu<'_> {
    pub fn new(index: u32, name: String, uuid: String) -> Self {
        let mut device = None;
        let mut vram = 0;

        if let Ok(nvml) = nvml()
            && let Ok(dev) = nvml.device_by_index(index)
            && let Ok(mem) = dev.memory_info()
        {
            vram = mem.total;
            device = Some(dev);
        }

        NvidiaGpu {
            index,
            name,
            uuid,
            vram_total: vram,
            device,
        }
    }
}

impl super::GpuIf for NvidiaGpu<'_> {
    fn gpu_type(&self) -> GpuType {
        GpuType::Nvidia
    }

    fn restart(&mut self) {
        if self.device.is_none()
            && let Ok(nvml) = nvml()
            && let Ok(dev) = nvml.device_by_index(self.index)
        {
            self.device = Some(dev);
        }
    }

    fn stop(&mut self) {
        if self.device.is_some() {
            // Drop device
            self.device = None;
        }
    }

    fn is_active(&self) -> bool {
        self.device.is_some()
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn id(&self) -> String {
        self.uuid.clone()
    }

    fn usage(&self) -> Result<u32> {
        self.with_device(|device_ref| {
            let rates = device_ref.utilization_rates()?;
            Ok(rates.gpu)
        })
    }

    fn temperature(&self) -> Result<u32> {
        self.with_device(|device_ref| {
            let temp = device_ref
                .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)?
                * 1000;
            Ok(temp)
        })
    }

    fn vram_total(&self) -> u64 {
        self.vram_total
    }

    fn vram_used(&self) -> Result<u64> {
        self.with_device(|device_ref| {
            let mem = device_ref.memory_info()?;
            Ok(mem.used)
        })
    }
}

impl NvidiaGpu<'_> {
    pub fn get_gpus() -> Vec<Gpu> {
        let mut v: Vec<Gpu> = Vec::new();

        // Nvidia GPUs
        if let Ok(count) = NvidiaGpu::gpus() {
            let nvidia_gpus = (0..count)
                .filter_map(|i| {
                    // Try to get both name and UUID, skip this GPU if either fails
                    let name = NvidiaGpu::name(i).ok()?;
                    let uuid = NvidiaGpu::uuid(i).ok()?;

                    Some(Gpu::new(Box::new(NvidiaGpu::new(i, name, uuid))))
                })
                .collect::<Vec<_>>();

            v.extend(nvidia_gpus);
        } else {
            info!("No Nvidia GPUs found");
        }
        v
    }

    pub fn uuid(idx: u32) -> Result<String> {
        nvml()
            .context("unable to establish NVML connection")
            .and_then(|nvml| {
                let dev = nvml.device_by_index(idx)?;
                dev.uuid().context("Unable to retrieve uuid")
            })
    }

    pub fn name(idx: u32) -> Result<String> {
        nvml()
            .context("unable to establish NVML connection")
            .and_then(|nvml| {
                let dev = nvml.device_by_index(idx)?;
                dev.name().context("Unable to retrieve name")
            })
    }

    fn gpus() -> Result<u32> {
        nvml()
            .context("unable to establish NVML connection")
            .and_then(|nvml| nvml.device_count().context("failed to get GPU count"))
    }

    fn with_device<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Device) -> Result<T>,
    {
        match self.device.as_ref() {
            Some(device_ref) => f(device_ref),
            None => Err(anyhow!("nvml device not loaded")),
        }
    }
}
