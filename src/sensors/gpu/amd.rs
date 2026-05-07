use anyhow::{Context, Result, anyhow};
use hex;
use log::{debug, info};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use crate::sensors::gpu::GpuType;
use crate::sensors::gpus::Gpu;
//use log::{debug, warn};

pub struct AmdGpu {
    name: String,
    id: String,
    usage_path: String,
    vram_used_path: String,
    power_status_path: String,
    temp_input_path: Option<String>,
    vram_total: u64,
    paused: bool,
}

impl AmdGpu {
    pub fn new(name: &str, card: &str, id: &str, vram_total: u64) -> Self {
        let base = format!("/sys/class/drm/{card}/device");
        let temp_input_path = AmdGpu::find_temp_input_path(card);
        Self {
            name: name.to_string(),
            id: id.to_string(),
            usage_path: format!("{base}/gpu_busy_percent"),
            vram_used_path: format!("{base}/mem_info_vram_used"),
            power_status_path: format!("{base}/power/runtime_status"),
            temp_input_path,
            vram_total,
            paused: false,
        }
    }

    fn powered_on(&self) -> bool {
        Self::read_file_to_string(&self.power_status_path).map_or(true, |s| s != "suspended")
    }

    fn parse_u32_file(path: &str) -> Option<u32> {
        Self::read_file_to_string(path).ok()?.parse().ok()
    }

    fn parse_u64_file(path: &str) -> Option<u64> {
        Self::read_file_to_string(path).ok()?.parse().ok()
    }

    fn read_file_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
        fs::read_to_string(path).map(|s| s.trim().to_string())
    }

    fn get_amd_cards() -> Vec<String> {
        debug!("AmdGpu::get_amd_cards().");
        let mut cards = Vec::new();
        if let Ok(entries) = fs::read_dir("/sys/class/drm/") {
            for entry in entries.flatten() {
                let path = entry.path();
                debug!("                    entry {path:?}");
                if path.join("device/vendor").exists()
                    && let Ok(vendor_id) = Self::read_file_to_string(path.join("device/vendor"))
                {
                    if vendor_id == "0x1002" {
                        debug!("                    AMD vendor ID");
                        if let Some(card) = path.file_name().and_then(|n| n.to_str()) {
                            if card.contains("card") {
                                debug!("                    phyical Card.");
                                cards.push(card.to_string());
                            } else {
                                debug!("                    virtual card");
                            }
                        }
                    } else {
                        debug!("                    Not AMD");
                    }
                }
            }
        }
        cards
    }

    fn find_temp_input_path(card: &str) -> Option<String> {
        log::info!("AMD find_temp_input_path({card})");
        let hwmon_base = format!("/sys/class/drm/{card}/device/hwmon");
        let entries = fs::read_dir(hwmon_base).ok()?;

        for entry in entries.flatten() {
            let path = entry.path().join("temp1_input");
            if path.exists() {
                log::info!("    Found temperature file {path:?}");
                return Some(path.to_string_lossy().to_string());
            }
        }

        log::info!("    Couldn't find temp1_input.");
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
            let truncated = truncated.split("[1002:").next().unwrap_or(model);
            truncated
                .replace("Corporation", "")
                .replace("[AMD/ATI]", "")
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
                && let Some((slot, rest)) = line.split_once(' ')
            {
                let model = rest.trim();
                let name = clean_gpu_name(model);
                map.push((slot.to_lowercase().to_string(), name));
            }
        }
        map
    }

    fn get_gpu_name(card: &str, lspci_map: &Vec<(String, String)>) -> String {
        info!("Resolving GPU name for card: {card}");

        // Use static lookup table first, with nice names
        let device_id_path = format!("/sys/class/drm/{card}/device/device");
        if let Ok(dev_id) = AmdGpu::read_file_to_string(&device_id_path) {
            info!("Read device ID from sysfs: {dev_id}");
            if let Some(name) = AMD_GPU_DEVICE_IDS.get(dev_id.to_uppercase().as_str()) {
                debug!("Found name in static map: {name}");
                return (*name).to_string();
            }
            info!("No entry in static map for device ID: {dev_id}");
        } else {
            debug!("Failed to read device ID from path: {device_id_path}");
        }

        // Fallback: Get PCI slot and look for it in the lspci list
        if let Some(slot) = &AmdGpu::get_pci_slot(card) {
            info!("Resolved PCI slot for card {card}: {slot:?}");
            for (p, n) in lspci_map {
                if slot.contains(p) {
                    info!("Found name in lspci_map: {n}");
                    return n.clone();
                }
            }
            debug!("No entry in lspci_map for slot: {slot}");
        }

        debug!("Falling back to unknown GPU name");
        "Unknown AMD GPU".to_string()
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
        debug!("AmdGpu::get_gpus().");

        let mut gpus = Vec::new();

        let lspci_map = AmdGpu::get_lspci_gpu_names();
        debug!("Available lspci_map entries:");
        for (k, v) in &lspci_map {
            debug!("  {k} -> {v}");
        }

        let cards = AmdGpu::get_amd_cards();

        for card in cards {
            debug!("                    Found card {card}");
            if let Some(vram_total) = AmdGpu::get_vram_total(&card) {
                debug!("                    total vram {vram_total}");
                if let Some(id) = AmdGpu::generate_gpu_id(&card) {
                    debug!("                    id {id}");
                    let name = AmdGpu::get_gpu_name(&card, &lspci_map);
                    debug!("                    name {name}");
                    gpus.push(Gpu::new(Box::new(AmdGpu::new(
                        &name, &card, &id, vram_total,
                    ))));
                }
            }
        }
        gpus
    }
}

impl super::GpuIf for AmdGpu {
    fn gpu_type(&self) -> GpuType {
        GpuType::Amd
    }

    fn restart(&mut self) {
        debug!("AmdGpu::restart({}).", self.name);
        self.paused = false;
    }

    fn stop(&mut self) {
        debug!("AmdGpu::stop({}).", self.name);
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

    fn vram_total(&self) -> u64 {
        debug!("AmdGpu::vram_total({}) - {}.", self.name, self.vram_total);
        self.vram_total
    }

    fn usage(&self) -> Result<u32> {
        if !self.is_active() {
            return Err(anyhow!("AMD device paused"));
        }
        if !self.powered_on() {
            return Ok(0);
        }
        Ok(Self::parse_u32_file(&self.usage_path).unwrap_or(0))
    }

    fn temperature(&self) -> Result<u32> {
        if !self.powered_on() {
            return Ok(0);
        }

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

    fn vram_used(&self) -> Result<u64> {
        if !self.is_active() {
            return Err(anyhow!("AMD device paused"));
        }
        if !self.powered_on() {
            return Ok(0);
        }
        Ok(Self::parse_u64_file(&self.vram_used_path).unwrap_or(0))
    }
}

impl std::fmt::Debug for AmdGpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AmdGpu {{ name: {}, id: {}, paused: {} }}",
            self.name, self.id, self.paused
        )
    }
}

// A hashmap containing AMD graphics card subsystem device IDs and their names
// Keys are the values found in /sys/class/drm/card?/device/subsystem_device
pub static AMD_GPU_DEVICE_IDS: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();

        // Radeon RX 7000 Series
        m.insert("0x744C", "AMD Radeon RX 7700S");
        m.insert("0x73FF", "AMD Radeon RX 7900 XTX");
        m.insert("0x73DF", "AMD Radeon RX 7900 XT");
        m.insert("0x7470", "AMD Radeon RX 7800 XT");
        m.insert("0x7460", "AMD Radeon RX 7700 XT");
        m.insert("0x7420", "AMD Radeon RX 7600");
        m.insert("0x7422", "AMD Radeon RX 7600 XT");

        // Radeon RX 6000 Series
        m.insert("0x73BF", "AMD Radeon RX 6950 XT");
        m.insert("0x73A5", "AMD Radeon RX 6900 XT");
        m.insert("0x73A3", "AMD Radeon RX 6800 XT");
        m.insert("0x73AB", "AMD Radeon RX 6800");
        m.insert("0x73DF", "AMD Radeon RX 6750 XT");
        m.insert("0x73D5", "AMD Radeon RX 6700 XT");
        m.insert("0x73FF", "AMD Radeon RX 6700");
        m.insert("0x73EF", "AMD Radeon RX 6650 XT");
        m.insert("0x73E8", "AMD Radeon RX 6600 XT");
        m.insert("0x73E3", "AMD Radeon RX 6600");
        m.insert("0x7422", "AMD Radeon RX 6500 XT");
        m.insert("0x7424", "AMD Radeon RX 6400");

        // Radeon RX 5000 Series
        m.insert("0x731F", "AMD Radeon RX 5700 XT");
        m.insert("0x7340", "AMD Radeon RX 5700");
        m.insert("0x7341", "AMD Radeon RX 5600 XT");
        m.insert("0x7347", "AMD Radeon RX 5500 XT");

        // Radeon RX Vega Series
        m.insert("0x687F", "AMD Radeon VII");
        m.insert("0x6863", "AMD Radeon RX Vega 64");
        m.insert("0x6867", "AMD Radeon RX Vega 56");

        // Radeon RX 500 Series
        m.insert("0x67DF", "AMD Radeon RX 590");
        m.insert("0x67FF", "AMD Radeon RX 580");
        m.insert("0x67EF", "AMD Radeon RX 570");
        m.insert("0x67E0", "AMD Radeon RX 560");
        m.insert("0x699F", "AMD Radeon RX 550");

        // APUs - Integrated Graphics
        m.insert("0x15BF", "AMD Radeon 780M iGPU");
        m.insert("0x1681", "AMD Radeon 780M iGPU");
        m.insert("0x15E7", "AMD Radeon 760M iGPU");
        m.insert("0x15D8", "AMD Radeon 680M iGPU");
        m.insert("0x1638", "AMD Radeon 660M iGPU");
        m.insert("0x164C", "AMD Radeon 610M iGPU");
        m.insert("0x15DD", "AMD Radeon Vega 8 iGPU");
        m.insert("0x15D8", "AMD Radeon Vega 7 iGPU");

        // Radeon Pro Series
        m.insert("0x73A2", "AMD Radeon Pro W6800");
        m.insert("0x73A3", "AMD Radeon Pro W6600");
        m.insert("0x6867", "AMD Radeon Pro VII");
        m.insert("0x66AF", "AMD Radeon Pro WX 9100");
        m.insert("0x67C4", "AMD Radeon Pro WX 7100");

        m
    });
