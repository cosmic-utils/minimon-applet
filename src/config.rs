use std::collections::HashMap;

use cosmic::{
    cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry},
    cosmic_theme::palette::Srgba,
};
use serde::{Deserialize, Serialize};

use crate::{fl, sensors::TempUnit};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub enum ColorVariant {
    Color1,
    Color2,
    Color3,
    Color4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphKind {
    Ring,
    Line,
    Heat,
}

impl From<usize> for GraphKind {
    fn from(index: usize) -> Self {
        match index {
            0 => GraphKind::Ring,
            1 => GraphKind::Line,
            2 => GraphKind::Heat,
            _ => panic!("Invalid index for SvgKind"),
        }
    }
}

impl From<GraphKind> for usize {
    fn from(kind: GraphKind) -> Self {
        match kind {
            GraphKind::Ring => 0,
            GraphKind::Line => 1,
            GraphKind::Heat => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Cpu,
    CpuTemp,
    Memory,
    Network(NetworkVariant),
    Disks(DisksVariant),
    Gpu,
    Vram,
    GpuTemp,
}

impl std::fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DeviceKind::Cpu => write!(f, "{}", fl!("sensor-cpu")),
            DeviceKind::CpuTemp => write!(f, "{}", fl!("sensor-cpu-temperature")),
            DeviceKind::Memory => write!(f, "{}", fl!("sensor-memory")),
            DeviceKind::Network(_) => write!(f, "{}", fl!("sensor-network")),
            DeviceKind::Disks(_) => write!(f, "{}", fl!("sensor-disks")),
            DeviceKind::Gpu => write!(f, "{}", fl!("sensor-gpu")),
            DeviceKind::Vram => write!(f, "{}", fl!("sensor-vram")),
            DeviceKind::GpuTemp => write!(f, "{}", fl!("sensor-gpu-temp")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, CosmicConfigEntry, PartialEq, Eq)]
#[version = 1]
pub struct GraphColors {
    pub color1: Srgba<u8>,
    pub color2: Srgba<u8>,
    pub color3: Srgba<u8>,
    pub color4: Srgba<u8>,
}

impl Default for GraphColors {
    fn default() -> Self {
        Self {
            color1: Srgba::from_components((0x2b, 0x2b, 0x2b, 0xff)),
            color2: Srgba::from_components((255, 255, 255, 255)),
            color3: Srgba::from_components((85, 85, 85, 255)),
            color4: Srgba::from_components((255, 6, 0, 255)),
        }
    }
}

impl GraphColors {
    pub fn new(kind: DeviceKind) -> Self {
        match kind {
            DeviceKind::Cpu => GraphColors::default(),
            DeviceKind::CpuTemp => GraphColors::default(),

            DeviceKind::Memory => GraphColors {
                color4: Srgba::from_components((187, 41, 187, 255)),
                ..Default::default()
            },

            DeviceKind::Network(_) => GraphColors {
                color1: Srgba::from_components((0x2b, 0x2b, 0x2b, 255)),
                color2: Srgba::from_components((47, 141, 255, 255)),
                color3: Srgba::from_components((0, 255, 0, 255)),
                color4: Srgba::from_components((0x2b, 0x2b, 0x2b, 255)),
            },

            DeviceKind::Disks(_) => GraphColors {
                color1: Srgba::from_components((0x2b, 0x2b, 0x2b, 255)),
                color2: Srgba::from_components((255, 102, 0, 255)),
                color3: Srgba::from_components((255, 255, 0, 255)),
                color4: Srgba::from_components((0x2b, 0x2b, 0x2b, 255)),
            },
            DeviceKind::Gpu => GraphColors {
                color4: Srgba::from_components((0, 255, 0, 255)),
                ..Default::default()
            },
            DeviceKind::Vram => GraphColors {
                color4: Srgba::from_components((0, 255, 0, 255)),
                ..Default::default()
            },
            DeviceKind::GpuTemp => GraphColors {
                color4: Srgba::from_components((255, 95, 31, 255)),
                ..Default::default()
            },
        }
    }

    pub fn set_color(&mut self, srgb: Srgba<u8>, variant: ColorVariant) {
        match variant {
            ColorVariant::Color1 => self.color1 = srgb,
            ColorVariant::Color2 => self.color2 = srgb,
            ColorVariant::Color3 => self.color3 = srgb,
            ColorVariant::Color4 => self.color4 = srgb,
        }
    }

    pub fn get_color(self, variant: ColorVariant) -> Srgba<u8> {
        match variant {
            ColorVariant::Color1 => self.color1,
            ColorVariant::Color2 => self.color2,
            ColorVariant::Color3 => self.color3,
            ColorVariant::Color4 => self.color4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq, Eq)]
#[version = 1]
pub struct CpuConfig {
    pub chart: bool,
    pub label: bool,
    pub kind: GraphKind,
    pub colors: GraphColors,
    pub no_decimals: bool,
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            chart: true,
            label: false,
            kind: GraphKind::Ring,
            colors: GraphColors::new(DeviceKind::Cpu),
            no_decimals: false,
        }
    }
}

impl CpuConfig {
    pub fn is_visible(&self) -> bool {
        self.chart || self.label
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq, Eq)]
#[version = 1]
pub struct CpuTempConfig {
    pub chart: bool,
    pub label: bool,
    pub kind: GraphKind,
    pub colors: GraphColors,
    pub unit: TempUnit,
}

impl Default for CpuTempConfig {
    fn default() -> Self {
        Self {
            chart: true,
            label: false,
            kind: GraphKind::Heat,
            colors: GraphColors::new(DeviceKind::CpuTemp),
            unit: TempUnit::Celcius,
        }
    }
}

impl CpuTempConfig {
    pub fn is_visible(&self) -> bool {
        self.chart || self.label
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq)]
#[version = 1]
pub struct MemoryConfig {
    pub chart: bool,
    pub label: bool,
    pub kind: GraphKind,
    pub colors: GraphColors,
    pub percentage: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            chart: true,
            label: false,
            kind: GraphKind::Ring,
            colors: GraphColors::new(DeviceKind::Memory),
            percentage: false,
        }
    }
}

impl MemoryConfig {
    pub fn is_visible(&self) -> bool {
        self.chart || self.label
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkVariant {
    Download,
    Upload,
    Combined,
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq)]
#[version = 1]
pub struct NetworkConfig {
    pub chart: bool,
    pub label: bool,
    pub adaptive: bool,
    pub bandwidth: u64,
    pub unit: Option<usize>,
    pub colors: GraphColors,
    pub variant: NetworkVariant,
}

impl NetworkConfig {
    pub fn is_visible(&self) -> bool {
        self.chart || self.label
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            chart: true,
            label: false,
            adaptive: true,
            bandwidth: 62_500_000, // 500Mbit/s
            unit: Some(0),
            colors: GraphColors::new(DeviceKind::Network(NetworkVariant::Combined)),
            variant: NetworkVariant::Combined,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DisksVariant {
    Write,
    Read,
    Combined,
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq)]
#[version = 1]
pub struct DisksConfig {
    pub chart: bool,
    pub label: bool,
    pub colors: GraphColors,
    pub variant: DisksVariant,
}

impl DisksConfig {
    pub fn is_visible(&self) -> bool {
        self.chart || self.label
    }
}

impl Default for DisksConfig {
    fn default() -> Self {
        Self {
            chart: false,
            label: false,
            colors: GraphColors::new(DeviceKind::Disks(DisksVariant::Combined)),
            variant: DisksVariant::Combined,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq, Eq)]
#[version = 1]
pub struct GpuUsageConfig {
    pub chart: bool,
    pub label: bool,
    pub kind: GraphKind,
    pub colors: GraphColors,
}

impl Default for GpuUsageConfig {
    fn default() -> Self {
        Self {
            chart: true,
            label: false,
            kind: GraphKind::Ring,
            colors: GraphColors::new(DeviceKind::GpuTemp),
        }
    }
}

impl GpuUsageConfig {
    pub fn is_visible(&self) -> bool {
        self.chart || self.label
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq, Eq)]
#[version = 1]
pub struct GpuVramConfig {
    pub chart: bool,
    pub label: bool,
    pub kind: GraphKind,
    pub colors: GraphColors,
}

impl Default for GpuVramConfig {
    fn default() -> Self {
        Self {
            chart: true,
            label: false,
            kind: GraphKind::Ring,
            colors: GraphColors::new(DeviceKind::Vram),
        }
    }
}

impl GpuVramConfig {
    pub fn is_visible(&self) -> bool {
        self.chart || self.label
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq, Eq)]
#[version = 1]
pub struct GpuTempConfig {
    pub chart: bool,
    pub label: bool,
    pub kind: GraphKind,
    pub colors: GraphColors,
    pub unit: TempUnit,
}

impl Default for GpuTempConfig {
    fn default() -> Self {
        Self {
            chart: false,
            label: false,
            kind: GraphKind::Heat,
            colors: GraphColors::new(DeviceKind::GpuTemp),
            unit: TempUnit::Celcius,
        }
    }
}

impl GpuTempConfig {
    pub fn is_visible(&self) -> bool {
        self.chart || self.label
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq, Eq)]
#[version = 1]
pub struct GpuConfig {
    pub usage: GpuUsageConfig,
    pub vram: GpuVramConfig,
    pub temp: GpuTempConfig,
    pub pause_on_battery: bool,
    pub stack_labels: bool,
}

impl GpuConfig {
    pub fn is_visible(&self) -> bool {
        self.usage.is_visible() || self.vram.is_visible() || self.temp.is_visible()
    }
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            usage: GpuUsageConfig::default(),
            vram: GpuVramConfig::default(),
            temp: GpuTempConfig::default(),
            pause_on_battery: true,
            stack_labels: true,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub enum ContentType {
    CpuUsage,
    CpuTemp,
    MemoryUsage,
    NetworkUsage,
    DiskUsage,
    GpuInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq, Eq)]
#[version = 1]
pub struct ContentOrder {
    pub order: Vec<ContentType>,
}

impl Default for ContentOrder {
    fn default() -> Self {
        Self {
            order: vec![
                ContentType::CpuUsage,
                ContentType::CpuTemp,
                ContentType::MemoryUsage,
                ContentType::NetworkUsage,
                ContentType::DiskUsage,
                ContentType::GpuInfo,
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq)]
#[version = 1]
pub struct MinimonConfig {
    pub refresh_rate: u32,
    pub label_size_default: u16,
    pub monospace_labels: bool,

    pub cpu: CpuConfig,
    pub cputemp: CpuTempConfig,
    pub memory: MemoryConfig,

    pub network1: NetworkConfig,
    pub network2: NetworkConfig,

    pub disks1: DisksConfig,
    pub disks2: DisksConfig,

    pub gpus: HashMap<String, GpuConfig>,

    pub sysmon: usize,

    pub symbols: bool,
    pub panel_spacing: u16,
    
    pub content_order: ContentOrder,
}

impl Default for MinimonConfig {
    fn default() -> Self {
        Self {
            refresh_rate: 1000,
            label_size_default: 11,
            monospace_labels: false,
            cpu: CpuConfig::default(),
            cputemp: CpuTempConfig::default(),
            memory: MemoryConfig::default(),
            network1: NetworkConfig {
                variant: NetworkVariant::Combined,
                ..Default::default()
            },
            network2: NetworkConfig {
                variant: NetworkVariant::Upload,
                ..Default::default()
            },
            disks1: DisksConfig {
                variant: DisksVariant::Combined,
                ..Default::default()
            },
            disks2: DisksConfig {
                variant: DisksVariant::Read,
                ..Default::default()
            },
            gpus: HashMap::new(),
            sysmon: 0,
            symbols: false,
            panel_spacing: 3, // Slider setting for cosmic.space_xs()
            content_order: ContentOrder::default(),
        }
    }
}
