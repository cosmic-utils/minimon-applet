use anyhow::{Context, Result, anyhow};
use hex;
use log::{debug, info};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::sensors::gpu::GpuType;
use crate::sensors::gpus::Gpu;

/// PCI vendor ID for Intel.
const INTEL_VENDOR_ID: &str = "0x8086";

/// Snapshot of the cumulative RC6 (idle/low-power) residency counter, used to
/// derive a usage percentage from the delta between two samples.
#[derive(Clone, Copy)]
struct Rc6Sample {
    at: Instant,
    residency_ms: u64,
}

pub struct IntelGpu {
    name: String,
    id: String,
    rc6_residency_path: Option<String>,
    vram_used_path: String,
    temp_input_path: Option<String>,
    vram_total: u64,
    last_rc6_sample: Cell<Option<Rc6Sample>>,
    paused: bool,
}

impl IntelGpu {
    pub fn new(name: &str, card: &str, id: &str) -> Self {
        let base = format!("/sys/class/drm/{card}/device");
        Self {
            name: name.to_string(),
            id: id.to_string(),
            rc6_residency_path: IntelGpu::find_rc6_residency_path(card),
            vram_used_path: format!("{base}/mem_info_vram_used"),
            temp_input_path: IntelGpu::find_temp_input_path(card),
            vram_total: IntelGpu::get_vram_total(card).unwrap_or(0),
            last_rc6_sample: Cell::new(None),
            paused: false,
        }
    }

    fn read_file_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
        fs::read_to_string(path).map(|s| s.trim().to_string())
    }

    fn parse_u64_file(path: &str) -> Option<u64> {
        Self::read_file_to_string(path).ok()?.parse().ok()
    }

    fn get_intel_cards() -> Vec<String> {
        debug!("IntelGpu::get_intel_cards().");
        let mut cards = Vec::new();
        if let Ok(entries) = fs::read_dir("/sys/class/drm/") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.join("device/vendor").exists()
                    && let Ok(vendor_id) = Self::read_file_to_string(path.join("device/vendor"))
                    && vendor_id == INTEL_VENDOR_ID
                    && let Some(card) = path.file_name().and_then(|n| n.to_str())
                    && card.starts_with("card")
                {
                    debug!("                    Found Intel card {card}");
                    cards.push(card.to_string());
                }
            }
        }
        cards
    }

    /// Locates the `gt*/rc6_residency_ms` counter (cumulative time spent in the
    /// GPU's idle/low-power state), used to estimate GPU busy percentage since
    /// the i915/xe drivers don't expose a direct usage percentage.
    fn find_rc6_residency_path(card: &str) -> Option<String> {
        let gt_base = format!("/sys/class/drm/{card}/gt");
        let entries = fs::read_dir(gt_base).ok()?;

        for entry in entries.flatten() {
            let path = entry.path().join("rc6_residency_ms");
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
        }

        // Older/simpler layout used directly under the card.
        let fallback = format!("/sys/class/drm/{card}/power/rc6_residency_ms");
        Path::new(&fallback).exists().then_some(fallback)
    }

    fn find_temp_input_path(card: &str) -> Option<String> {
        let hwmon_base = format!("/sys/class/drm/{card}/device/hwmon");
        let entries = fs::read_dir(hwmon_base).ok()?;

        for entry in entries.flatten() {
            let path = entry.path().join("temp1_input");
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
        }

        None
    }

    fn get_vram_total(card: &str) -> Option<u64> {
        let path = format!("/sys/class/drm/{card}/device/mem_info_vram_total");
        Self::parse_u64_file(&path)
    }

    fn get_pci_slot(card: &str) -> Option<String> {
        let path = format!("/sys/class/drm/{card}/device/uevent");
        Self::read_file_to_string(path)
            .ok()?
            .lines()
            .find_map(|line| {
                line.strip_prefix("PCI_SLOT_NAME=")
                    .map(|s| s.to_lowercase().to_string())
            })
    }

    fn get_lspci_gpu_names() -> Vec<(String, String)> {
        fn clean_gpu_name(model: &str) -> String {
            let (_, truncated) = model.split_once("]:").unwrap_or((model, model));
            let truncated = truncated.split("[8086:").next().unwrap_or(model);
            truncated
                .replace("Corporation", "")
                .replace("compatible controller", "")
                .replace("controller", "")
                .replace("VGA", "")
                .replace("3D", "")
                .replace("Display", "")
                .replace(':', "")
                .replace("  ", " ")
                .replace('[', "(")
                .replace(']', ")")
                .trim()
                .to_string()
        }

        let mut map = Vec::new();
        let output = Command::new("lspci").arg("-nn").output();
        let Ok(output) = output else {
            return map;
        };
        let Ok(stdout) = String::from_utf8(output.stdout) else {
            return map;
        };

        for line in stdout.lines() {
            if (line.contains("VGA") || line.contains("Display") || line.contains("3D"))
                && line.contains("[8086:")
                && let Some((slot, rest)) = line.split_once(' ')
            {
                let model = rest.trim();
                let name = clean_gpu_name(model);
                map.push((slot.to_lowercase().to_string(), name));
            }
        }
        map
    }

    fn get_gpu_name(card: &str, lspci_map: &[(String, String)]) -> String {
        info!("Resolving GPU name for card: {card}");

        if let Some(slot) = &IntelGpu::get_pci_slot(card) {
            for (p, n) in lspci_map {
                if slot.contains(p) {
                    debug!("Found name in lspci_map: {n}");
                    return n.clone();
                }
            }
            debug!("No entry in lspci_map for slot: {slot}");
        }

        "Intel Graphics".to_string()
    }

    fn generate_gpu_id(card: &str) -> Option<String> {
        let device_path = PathBuf::from(format!("/sys/class/drm/{card}/device"));
        let pci_address = device_path.canonicalize().ok()?;
        let subsystem_vendor =
            Self::read_file_to_string(device_path.join("subsystem_vendor")).ok()?;
        let subsystem_device =
            Self::read_file_to_string(device_path.join("subsystem_device")).ok()?;

        let mut hasher = Sha256::new();
        hasher.update(pci_address.to_string_lossy().as_bytes());
        hasher.update(subsystem_vendor.as_bytes());
        hasher.update(subsystem_device.as_bytes());

        Some(hex::encode(hasher.finalize()))
    }

    pub fn get_gpus() -> Vec<Gpu> {
        debug!("IntelGpu::get_gpus().");

        let mut gpus = Vec::new();

        let lspci_map = IntelGpu::get_lspci_gpu_names();
        let cards = IntelGpu::get_intel_cards();

        for card in cards {
            debug!("                    Found card {card}");
            if let Some(id) = IntelGpu::generate_gpu_id(&card) {
                let name = IntelGpu::get_gpu_name(&card, &lspci_map);
                debug!("                    name {name}, id {id}");
                gpus.push(Gpu::new(Box::new(IntelGpu::new(&name, &card, &id))));
            }
        }
        gpus
    }
}

impl super::GpuIf for IntelGpu {
    fn gpu_type(&self) -> GpuType {
        GpuType::Intel
    }

    fn restart(&mut self) {
        debug!("IntelGpu::restart({}).", self.name);
        self.paused = false;
        self.last_rc6_sample.set(None);
    }

    fn stop(&mut self) {
        debug!("IntelGpu::stop({}).", self.name);
        self.paused = true;
    }

    fn is_active(&self) -> bool {
        !self.paused
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn id(&self) -> String {
        self.id.clone()
    }

    fn usage(&self) -> Result<u32> {
        if !self.is_active() {
            return Err(anyhow!("Intel device paused"));
        }

        let path = self
            .rc6_residency_path
            .as_ref()
            .context("RC6 residency path not found")?;

        let residency_ms: u64 = Self::read_file_to_string(path)
            .with_context(|| format!("Failed to read RC6 residency from {path}"))?
            .parse()
            .context("Failed to parse RC6 residency value")?;

        let now = Instant::now();
        let previous = self.last_rc6_sample.replace(Some(Rc6Sample {
            at: now,
            residency_ms,
        }));

        let Some(previous) = previous else {
            // No baseline yet, report idle until a second sample is available.
            return Ok(0);
        };

        let elapsed_ms = u64::try_from(now.duration_since(previous.at).as_millis())
            .unwrap_or(u64::MAX)
            .max(1);
        let idle_delta_ms = residency_ms.saturating_sub(previous.residency_ms);
        let idle_percent = (idle_delta_ms.min(elapsed_ms) * 100) / elapsed_ms;

        Ok(100u32.saturating_sub(u32::try_from(idle_percent).unwrap_or(100)))
    }

    fn temperature(&self) -> Result<u32> {
        let path = self
            .temp_input_path
            .as_ref()
            .context("Temperature path not found")?;

        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read temperature from {path}"))?;

        let temp_millidegrees: u32 = contents
            .trim()
            .parse()
            .context("Failed to parse temperature value")?;

        Ok(temp_millidegrees)
    }

    fn vram_total(&self) -> u64 {
        self.vram_total
    }

    fn vram_used(&self) -> Result<u64> {
        if !self.is_active() {
            return Err(anyhow!("Intel device paused"));
        }
        if self.vram_total == 0 {
            // Integrated GPUs share system memory and don't report a separate total.
            return Err(anyhow!("Intel iGPU has no dedicated VRAM"));
        }
        Self::parse_u64_file(&self.vram_used_path).context("Failed to read VRAM usage")
    }
}

impl std::fmt::Debug for IntelGpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "IntelGpu {{ name: {}, id: {}, paused: {} }}",
            self.name, self.id, self.paused
        )
    }
}
