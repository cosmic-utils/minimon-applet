use std::collections::HashMap;

use cosmic::{
    cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry},
    cosmic_theme::palette::Srgba,
};
use serde::{Deserialize, Serialize};

use crate::{fl, sensors::TempUnit};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub enum ColorVariant {
    Background,
    Frame,
    Text,
    Graph1,
    Graph2,
    Graph3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartKind {
    Ring,
    Line,
    Heat,
    StackedBars,
}

impl From<usize> for ChartKind {
    fn from(index: usize) -> Self {
        match index {
            0 => ChartKind::Ring,
            1 => ChartKind::Line,
            2 => ChartKind::Heat,
            3 => ChartKind::StackedBars,
            _ => {
                log::error!("GrapKind::From({}) Invalid index for ChartKind", index);
                ChartKind::Line
            }
        }
    }
}

impl From<ChartKind> for usize {
    fn from(kind: ChartKind) -> Self {
        match kind {
            ChartKind::Ring => 0,
            ChartKind::Line => 1,
            ChartKind::Heat => 2,
            ChartKind::StackedBars => 3,
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
pub struct ChartColors {
    pub background: Srgba<u8>,
    pub frame: Srgba<u8>,
    pub text: Srgba<u8>,
    pub graph1: Srgba<u8>,
    pub graph2: Srgba<u8>,
    pub graph3: Srgba<u8>,
}

macro_rules! rgba {
    ($r:expr, $g:expr, $b:expr, $a:expr) => {
        Srgba::from_components(($r as u8, $g as u8, $b as u8, $a as u8))
    };
}

impl Default for ChartColors {
    fn default() -> Self {
        Self {
            background: rgba!(0x2b, 0x2b, 0x2b, 0xff),
            frame: rgba!(255, 255, 255, 255),
            text: rgba!(255, 255, 255, 255),
            graph1: rgba!(255, 6, 0, 255),
            graph2: rgba!(85, 85, 85, 255),
            graph3: rgba!(255, 165, 0, 255),
        }
    }
}

impl ChartColors {
    pub fn new(device: DeviceKind, chart: ChartKind) -> Self {
        match device {
            DeviceKind::Cpu => match chart {
                ChartKind::Ring => ChartColors {
                    graph1: rgba!(255, 6, 0, 255),
                    ..Default::default()
                },
                ChartKind::Line => ChartColors {
                    graph1: rgba!(255, 6, 0, 85),
                    ..Default::default()
                },
                ChartKind::StackedBars => ChartColors {
                    graph1: rgba!(80, 80, 255, 255),
                    graph2: rgba!(255, 0, 0, 255),
                    ..Default::default()
                },
                ChartKind::Heat => ChartColors::default(),
            },

            DeviceKind::CpuTemp => match chart {
                ChartKind::Ring => ChartColors {
                    graph1: rgba!(255, 6, 0, 255),
                    ..Default::default()
                },
                ChartKind::Line => ChartColors {
                    graph1: rgba!(255, 90, 0, 85),
                    ..Default::default()
                },
                _ => ChartColors::default(),
            },

            DeviceKind::Memory => match chart {
                ChartKind::Ring => ChartColors {
                    graph1: rgba!(29, 172, 214, 255),
                    graph3: rgba!(44, 87, 101, 255),
                    ..Default::default()
                },
                ChartKind::Line => ChartColors {
                    graph1: rgba!(29, 172, 214, 140),
                    graph3: rgba!(44, 87, 101, 140),
                    ..Default::default()
                },
                _ => ChartColors::default(),
            },

            DeviceKind::Network(_) => ChartColors {
                graph1: rgba!(47, 141, 255, 85),
                graph2: rgba!(0, 255, 0, 85),
                ..Default::default()
            },

            DeviceKind::Disks(_) => ChartColors {
                graph1: rgba!(255, 102, 0, 85),
                graph2: rgba!(255, 255, 0, 85),
                ..Default::default()
            },
            DeviceKind::Gpu => match chart {
                ChartKind::Ring => ChartColors {
                    graph1: rgba!(0, 255, 0, 255),
                    ..Default::default()
                },
                ChartKind::Line => ChartColors {
                    graph1: rgba!(0, 255, 0, 85),
                    ..Default::default()
                },
                _ => ChartColors::default(),
            },
            DeviceKind::Vram => match chart {
                ChartKind::Ring => ChartColors {
                    graph1: rgba!(0, 255, 0, 255),
                    ..Default::default()
                },
                ChartKind::Line => ChartColors {
                    graph1: rgba!(0, 255, 0, 85),
                    ..Default::default()
                },
                _ => ChartColors::default(),
            },
            DeviceKind::GpuTemp => match chart {
                ChartKind::Ring => ChartColors {
                    graph1: rgba!(255, 6, 0, 255),
                    ..Default::default()
                },
                ChartKind::Line => ChartColors {
                    graph1: rgba!(255, 6, 0, 85),
                    ..Default::default()
                },
                _ => ChartColors::default(),
            },
        }
    }

    pub fn set_color(&mut self, srgb: Srgba<u8>, variant: ColorVariant) {
        match variant {
            ColorVariant::Background => self.background = srgb,
            ColorVariant::Frame => self.frame = srgb,
            ColorVariant::Text => self.text = srgb,
            ColorVariant::Graph1 => self.graph1 = srgb,
            ColorVariant::Graph2 => self.graph2 = srgb,
            ColorVariant::Graph3 => self.graph3 = srgb,
        }
    }

    pub fn get_color(self, variant: ColorVariant) -> Srgba<u8> {
        match variant {
            ColorVariant::Background => self.background,
            ColorVariant::Frame => self.frame,
            ColorVariant::Text => self.text,
            ColorVariant::Graph1 => self.graph1,
            ColorVariant::Graph2 => self.graph2,
            ColorVariant::Graph3 => self.graph3,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, CosmicConfigEntry, PartialEq, Eq)]
#[version = 1]
pub struct Colors {
    ring: ChartColors,
    line: ChartColors,
    heat: ChartColors,
    stackedbars: ChartColors,
}

impl Colors {
    pub fn new(device: DeviceKind) -> Self {
        Colors {
            ring: ChartColors::new(device, ChartKind::Ring),
            line: ChartColors::new(device, ChartKind::Line),
            heat: ChartColors::new(device, ChartKind::Heat),
            stackedbars: ChartColors::new(device, ChartKind::StackedBars),
        }
    }

    pub fn get(&self, chart: ChartKind) -> &ChartColors {
        match chart {
            ChartKind::Ring => &self.ring,
            ChartKind::Line => &self.line,
            ChartKind::Heat => &self.heat,
            ChartKind::StackedBars => &self.stackedbars,
        }
    }

    pub fn get_mut(&mut self, chart: ChartKind) -> &mut ChartColors {
        match chart {
            ChartKind::Ring => &mut self.ring,
            ChartKind::Line => &mut self.line,
            ChartKind::Heat => &mut self.heat,
            ChartKind::StackedBars => &mut self.stackedbars,
        }
    }
}

macro_rules! make_config {
    ($name:ident { $($extra:tt)* }) => {
        #[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq)]
        #[version = 1]
        pub struct $name {
            chart_visible: bool,
            label_visible: bool,
            pub chart: ChartKind,
            colors: Colors,
            $($extra)*
        }

       impl $name {
              pub fn visible(&self) -> bool {
                self.chart_visible() || self.label_visible()
            }
            pub fn chart_visible(&self) -> bool {
                self.chart_visible
            }
            pub fn label_visible(&self) -> bool {
                self.label_visible
            }
            pub fn show_chart(&mut self, visible: bool) {
                self.chart_visible = visible;
            }
            pub fn show_label(&mut self, visible: bool) {
                self.label_visible = visible;
            }
            pub fn colors(&self) -> &ChartColors {
                self.colors.get(self.chart)
            }
            pub fn colors_mut(&mut self) -> &mut ChartColors {
                self.colors.get_mut(self.chart)
            }
        }
    };
}

make_config!(CpuConfig {
    pub no_decimals: bool,
    pub bar_width: u16,
    pub bar_spacing: u16,
});

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            chart_visible: true,
            label_visible: false,
            chart: ChartKind::Ring,
            colors: Colors::new(DeviceKind::Cpu),
            no_decimals: false,
            bar_width: 4,
            bar_spacing: 1,
        }
    }
}

make_config!(CpuTempConfig {
    pub unit: TempUnit,
    pub min_temp: f64,
});

impl Default for CpuTempConfig {
    fn default() -> Self {
        Self {
            chart_visible: false,
            label_visible: false,
            chart: ChartKind::Heat,
            colors: Colors::new(DeviceKind::CpuTemp),
            unit: TempUnit::Celsius,
            min_temp: 0.0,
        }
    }
}

make_config!(MemoryConfig {
    pub percentage: bool,
    pub show_allocated: bool,
    pub stack_labels: bool,
});

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            chart_visible: true,
            label_visible: false,
            chart: ChartKind::Ring,
            colors: Colors::new(DeviceKind::Memory),
            percentage: false,
            show_allocated: false,
            stack_labels: false, // future use
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkVariant {
    Download,
    Upload,
    Combined,
}

make_config!(NetworkConfig {
    pub adaptive: bool,
    pub bandwidth: u64,
    pub unit: Option<usize>,
    pub variant: NetworkVariant,
    pub show_bytes: bool,
});

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            chart_visible: true,
            label_visible: false,
            chart: ChartKind::Line,
            colors: Colors::new(DeviceKind::Network(NetworkVariant::Combined)),
            adaptive: true,
            bandwidth: 62_500_000,
            unit: Some(0),
            variant: NetworkVariant::Combined,
            show_bytes: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DisksVariant {
    Write,
    Read,
    Combined,
}

make_config!(DisksConfig {
    pub variant: DisksVariant,
});

impl Default for DisksConfig {
    fn default() -> Self {
        Self {
            chart_visible: false,
            label_visible: false,
            chart: ChartKind::Line,
            colors: Colors::new(DeviceKind::Disks(DisksVariant::Combined)),
            variant: DisksVariant::Combined,
        }
    }
}

make_config!(GpuUsageConfig {});

impl Default for GpuUsageConfig {
    fn default() -> Self {
        Self {
            chart_visible: true,
            label_visible: false,
            chart: ChartKind::Ring,
            colors: Colors::new(DeviceKind::Gpu),
        }
    }
}

make_config!(GpuVramConfig {});

impl Default for GpuVramConfig {
    fn default() -> Self {
        Self {
            chart_visible: true,
            label_visible: false,
            chart: ChartKind::Ring,
            colors: Colors::new(DeviceKind::Vram),
        }
    }
}

make_config!(GpuTempConfig {
        pub unit: TempUnit,
        pub min_temp: f64,
});

impl Default for GpuTempConfig {
    fn default() -> Self {
        Self {
            chart_visible: false,
            label_visible: false,
            chart: ChartKind::Ring,
            colors: Colors::new(DeviceKind::GpuTemp),
            unit: TempUnit::Celsius,
            min_temp: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry, PartialEq)]
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
        self.usage.visible() || self.vram.visible() || self.temp.visible()
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

    pub sysmon: Option<String>,

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
            sysmon: None,
            symbols: false,
            panel_spacing: 3, // Slider setting for cosmic.space_xs()
            content_order: ContentOrder::default(),
        }
    }
}
