use bounded_vec_deque::BoundedVecDeque;
use cosmic::iced::Alignment::Center;
use cosmic::{Element, Renderer, Theme};
use log::info;
use std::fmt::Write;

use crate::sensors::{GpuConfig, INVALID_IMG};
use cosmic::widget::{self, Column, Container};
use cosmic::widget::{settings, toggler};
use cosmic::{
    iced::{
        Alignment,
        widget::{column, row},
    },
    iced_widget::Row,
};

use super::TempUnit;
use crate::app::Message;
use crate::colorpicker::DemoGraph;
use crate::config::DeviceKind;
use crate::{
    config::{ChartColors, ChartKind, ColorVariant, GpuTempConfig, GpuUsageConfig, GpuVramConfig},
    fl,
    svg_graph::SvgColors,
};
use std::any::Any;

use super::gpu::amd::AmdGpu;
use super::gpu::intel::IntelGpu;
use super::gpu::{GpuIf, nvidia::NvidiaGpu};

const MAX_SAMPLES: usize = 21;

#[cfg(feature = "lyon_charts")]
use std::sync::LazyLock;
#[cfg(feature = "lyon_charts")]
static DISABLED_COLORS: LazyLock<ChartColors> = LazyLock::new(|| ChartColors {
    color1: cosmic::cosmic_theme::palette::Srgba::from_components((0xFF, 0xFF, 0xFF, 0x20)),
    color2: cosmic::cosmic_theme::palette::Srgba::from_components((0x72, 0x72, 0x72, 0xFF)),
    color3: cosmic::cosmic_theme::palette::Srgba::from_components((0x72, 0x72, 0x72, 0xFF)),
    color4: cosmic::cosmic_theme::palette::Srgba::from_components((0x72, 0x72, 0x72, 0xFF)),
});

pub struct GpuGraph {
    id: String,
    samples: BoundedVecDeque<f64>,
    graph_options: Vec<&'static str>,
    svg_colors: SvgColors,
    disabled: bool,
    disabled_colors: SvgColors,
    config: GpuUsageConfig,
}

impl GpuGraph {
    fn new(id: &str) -> Self {
        let mut percentage = String::with_capacity(6);
        percentage.push('0');

        let mut value = String::with_capacity(6);
        value.push('0');

        GpuGraph {
            id: id.to_owned(),
            samples: BoundedVecDeque::from_iter(std::iter::repeat_n(0.0, MAX_SAMPLES), MAX_SAMPLES),
            graph_options: super::GRAPH_OPTIONS_RING_LINE.to_vec(),
            svg_colors: SvgColors::new(&ChartColors::default()),
            disabled: false,
            disabled_colors: SvgColors {
                background: String::from("#FFFFFF20"),
                frame: String::from("#727272FF"),
                text: String::from("#727272FF"),
                graph1: String::from("#727272FF"),
                graph2: String::from("#727272FF"),
                graph3: String::from("#727272FF"),
            },
            config: GpuUsageConfig::default(),
        }
    }

    fn update_config(&mut self, config: &dyn Any, _refresh_rate: u32) {
        if let Some(cfg) = config.downcast_ref::<GpuUsageConfig>() {
            self.config = cfg.clone();
            self.svg_colors = SvgColors::new(cfg.colors());
        }
    }

    pub fn clear(&mut self) {
        for sample in &mut self.samples {
            *sample = 0.0;
        }
    }

    #[cfg(feature = "lyon_charts")]
    pub fn chart<'a>(&self) -> cosmic::widget::Container<crate::app::Message, Theme, Renderer> {
        if self.config.kind == ChartKind::Ring {
            let mut latest = self.latest_sample();
            let mut text = String::with_capacity(10);
            let mut percentage = String::with_capacity(10);
            if latest > 100.0 {
                latest = 100.0;
            }
            if self.disabled {
                _ = write!(percentage, "0");
                _ = write!(text, "-");
            } else {
                if latest < 10.0 {
                    write!(text, "{latest:.2}").unwrap();
                } else if latest < 100.0 {
                    write!(text, "{latest:.1}").unwrap();
                } else {
                    write!(text, "{latest}").unwrap();
                }
                write!(percentage, "{latest}").unwrap();
            }
            chart_container!(crate::charts::ring::RingChart::new(
                latest as f32,
                &text,
                &self.config.colors,
            ))
        } else {
            chart_container!(crate::charts::line::LineChart::new(
                MAX_SAMPLES,
                &self.samples,
                &VecDeque::new(),
                Some(100.0),
                &self.config.colors,
            ))
        }
    }

    #[cfg(not(feature = "lyon_charts"))]
    pub fn chart(
        &'_ self,
    ) -> cosmic::widget::Container<'_, crate::app::Message, cosmic::Theme, cosmic::Renderer> {
        let svg = if self.config.chart == ChartKind::Ring {
            let latest = self.latest_sample();
            let mut value = String::with_capacity(10);
            let mut percentage: u8 = 0;

            if self.disabled {
                value.push('-');
            } else {
                if latest < 10.0 {
                    let _ = write!(value, "{latest:.2}");
                } else if latest < 100.0 {
                    let _ = write!(value, "{latest:.1}");
                } else {
                    let _ = write!(value, "{latest}");
                }
                percentage = latest.round().clamp(0.0, 100.0) as u8;
            }

            crate::svg_graph::ring(
                &value,
                percentage,
                None,
                if self.disabled {
                    &self.disabled_colors
                } else {
                    &self.svg_colors
                },
            )
        } else {
            crate::svg_graph::line(
                &self.samples,
                100.0,
                if self.disabled {
                    &self.disabled_colors
                } else {
                    &self.svg_colors
                },
            )
        };
        super::svg_icon_container::<Message>(svg)
    }

    pub fn latest_sample(&self) -> f64 {
        *self.samples.back().unwrap_or(&0f64)
    }

    pub fn graph_kind(&self) -> crate::config::ChartKind {
        self.config.chart
    }

    pub fn set_graph_kind(&mut self, kind: crate::config::ChartKind) {
        self.config.chart = kind;
    }

    pub fn update(&mut self, sample: u32) {
        self.samples.push_back(f64::from(sample));
    }
}

impl fmt::Display for GpuGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.disabled {
            write!(f, "---%")
        } else {
            let current_val = self.latest_sample();
            if current_val < 10.0 {
                write!(f, "{:.2}%", (current_val * 100.0).trunc() / 100.0)
            } else if current_val < 100.0 {
                write!(f, "{:.1}%", (current_val * 10.0).trunc() / 10.0)
            } else {
                write!(f, "{current_val}%")
            }
        }
    }
}

impl DemoGraph for GpuGraph {
    fn demo(&self) -> String {
        match self.config.chart {
            ChartKind::Ring => {
                // show a number of 40% of max
                let val = 40;
                let percentage: u8 = 40;
                crate::svg_graph::ring(&format!("{val}"), percentage, None, &self.svg_colors)
            }
            ChartKind::Line => crate::svg_graph::line(
                &std::collections::VecDeque::from(DEMO_SAMPLES),
                100.0,
                &self.svg_colors,
            ),
            _ => {
                log::error!("GPUGraph type not supported {:?}", self.config.chart);
                INVALID_IMG.to_string()
            }
        }
    }

    fn colors(&self) -> &ChartColors {
        self.config.colors()
    }

    fn set_colors(&mut self, colors: &ChartColors) {
        *self.config.colors_mut() = *colors;
        self.svg_colors.set_colors(colors);
    }

    fn color_choices(&self) -> Vec<(&'static str, ColorVariant)> {
        if self.config.chart == ChartKind::Line {
            (*super::COLOR_CHOICES_LINE).into()
        } else {
            (*super::COLOR_CHOICES_RING).into()
        }
    }

    fn id(&self) -> Option<String> {
        Some(self.id.clone())
    }

    fn kind(&self) -> ChartKind {
        self.config.chart
    }
}

pub struct VramGraph {
    id: String,
    samples: BoundedVecDeque<f64>,
    graph_options: Vec<&'static str>,
    total: f64,
    svg_colors: SvgColors,
    disabled: bool,
    disabled_colors: SvgColors,
    config: GpuVramConfig,
}

impl VramGraph {
    // id: a unique id, total: RAM size in GB
    fn new(id: &str, total: f64) -> Self {
        VramGraph {
            id: id.to_owned(),
            samples: BoundedVecDeque::from_iter(std::iter::repeat_n(0.0, MAX_SAMPLES), MAX_SAMPLES),
            graph_options: super::GRAPH_OPTIONS_RING_LINE.to_vec(),
            total,
            svg_colors: SvgColors::new(&ChartColors::default()),
            disabled: false,
            disabled_colors: SvgColors {
                background: String::from("#FFFFFF20"),
                frame: String::from("#727272FF"),
                text: String::from("#727272FF"),
                graph1: String::from("#727272FF"),
                graph2: String::from("#727272FF"),
                graph3: String::from("#727272FF"),
            },
            config: GpuVramConfig::default(),
        }
    }

    fn update_config(&mut self, config: &dyn Any, _refresh_rate: u32) {
        if let Some(cfg) = config.downcast_ref::<GpuVramConfig>() {
            self.config = cfg.clone();
            self.svg_colors = SvgColors::new(cfg.colors());
        }
    }

    pub fn clear(&mut self) {
        for sample in &mut self.samples {
            *sample = 0.0;
        }
    }

    #[cfg(feature = "lyon_charts")]
    pub fn chart<'a>(
        &self,
    ) -> cosmic::widget::Container<crate::app::Message, cosmic::Theme, cosmic::Renderer> {
        if self.config.kind == ChartKind::Ring {
            let latest = self.latest_sample();
            let mut text = String::with_capacity(10);
            let mut percentage = String::with_capacity(10);

            let mut pct: f32 = 0.0;
            if self.disabled {
                _ = write!(percentage, "0");
                _ = write!(text, "-");
            } else {
                pct = ((latest / self.total) * 100.0) as f32;
                if pct > 100.0 {
                    pct = 100.0;
                }

                if latest < 10.0 {
                    write!(text, "{latest:.2}").unwrap();
                } else if latest < 100.0 {
                    write!(text, "{latest:.1}").unwrap();
                } else {
                    write!(text, "{latest}").unwrap();
                }
            }

            chart_container!(crate::charts::ring::RingChart::new(
                pct,
                &text,
                &self.config.colors,
            ))
        } else {
            chart_container!(crate::charts::line::LineChart::new(
                MAX_SAMPLES,
                &self.samples,
                &VecDeque::new(),
                Some(self.total),
                &self.config.colors,
            ))
        }
    }

    #[cfg(not(feature = "lyon_charts"))]
    pub fn chart(&'_ self) -> cosmic::widget::Container<'_, crate::app::Message, Theme, Renderer> {
        let svg = if self.config.chart == ChartKind::Ring {
            let latest = self.latest_sample();
            let mut value = String::with_capacity(10);
            let mut percentage: u8 = 0;

            if self.disabled {
                value.push('-');
            } else {
                if latest < 10.0 {
                    let _ = write!(value, "{:.2}", (latest * 100.0).trunc() / 100.0);
                } else if latest < 100.0 {
                    let _ = write!(value, "{:.1}", (latest * 10.0).trunc() / 10.0);
                } else {
                    let _ = write!(value, "{}", latest.round());
                }
                percentage = ((latest / self.total) * 100.0).round().clamp(0.0, 100.0) as u8;
            }
            crate::svg_graph::ring(
                &value,
                percentage,
                None,
                if self.disabled {
                    &self.disabled_colors
                } else {
                    &self.svg_colors
                },
            )
        } else {
            crate::svg_graph::line(
                &self.samples,
                self.total,
                if self.disabled {
                    &self.disabled_colors
                } else {
                    &self.svg_colors
                },
            )
        };
        super::svg_icon_container::<Message>(svg)
    }

    pub fn latest_sample(&self) -> f64 {
        *self.samples.back().unwrap_or(&0f64)
    }

    pub fn graph_kind(&self) -> crate::config::ChartKind {
        self.config.chart
    }

    pub fn set_graph_kind(&mut self, kind: crate::config::ChartKind) {
        self.config.chart = kind;
    }

    pub fn string(&self, vertical_panel: bool) -> String {
        let current_val = self.latest_sample();
        let unit: &str = if vertical_panel { "GB" } else { " GB" };

        if self.disabled {
            format!("---{unit}")
        } else if current_val < 10.0 {
            format!("{:.2}{unit}", (current_val * 100.0).trunc() / 100.0)
        } else if current_val < 100.0 {
            format!("{:.1}{unit}", (current_val * 10.0).trunc() / 10.0)
        } else {
            format!("{}{unit}", current_val.round())
        }
    }

    pub fn total(&self) -> f64 {
        self.total
    }

    pub fn update(&mut self, sample: u64) {
        let new_val: f64 = sample as f64 / 1_073_741_824.0;
        self.samples.push_back(new_val);
    }
}

pub struct TempGraph {
    id: String,
    samples: BoundedVecDeque<f64>,
    unit_options: Vec<&'static str>,
    graph_options: Vec<&'static str>,
    max_temp: f64,
    svg_colors: SvgColors,
    disabled: bool,
    disabled_colors: SvgColors,
    config: GpuTempConfig,
}

impl TempGraph {
    // id: a unique id, total: RAM size in GB
    fn new(id: &str) -> Self {
        TempGraph {
            id: id.to_owned(),
            samples: BoundedVecDeque::from_iter(std::iter::repeat_n(0.0, MAX_SAMPLES), MAX_SAMPLES),
            unit_options: super::UNIT_OPTIONS.to_vec(),
            graph_options: super::GRAPH_OPTIONS_RING_LINE_HEAT.to_vec(),
            max_temp: 100.0,
            svg_colors: SvgColors::new(&ChartColors::default()),
            disabled: false,
            disabled_colors: SvgColors {
                background: String::from("#FFFFFF20"),
                frame: String::from("#727272FF"),
                text: String::from("#727272FF"),
                graph1: String::from("#727272FF"),
                graph2: String::from("#727272FF"),
                graph3: String::from("#727272FF"),
            },
            config: GpuTempConfig::default(),
        }
    }

    fn update_config(&mut self, config: &dyn Any, _refresh_rate: u32) {
        if let Some(cfg) = config.downcast_ref::<GpuTempConfig>() {
            self.config = cfg.clone();
            self.svg_colors = SvgColors::new(cfg.colors());
        }
    }

    pub fn clear(&mut self) {
        for sample in &mut self.samples {
            *sample = 0.0;
        }
    }

    #[cfg(feature = "lyon_charts")]
    pub fn chart(
        &self,
    ) -> cosmic::widget::Container<crate::app::Message, cosmic::Theme, cosmic::Renderer> {
        match self.config.kind {
            ChartKind::Ring => {
                let mut latest = self.latest_sample();
                let mut text = self.to_string();

                // remove the °C/°F/°R/K unit if there's not enough space (assuming temp stays below 282°C = 1000°R)
                while text.len() > 3 {
                    let _ = text.pop();
                }
                let mut percentage = String::with_capacity(10);

                write!(percentage, "{latest}").unwrap();

                if latest > 100.0 {
                    latest = 100.0;
                }

                chart_container!(crate::charts::ring::RingChart::new(
                    latest as f32,
                    &text,
                    if self.disabled {
                        &*DISABLED_COLORS
                    } else {
                        &self.config.colors
                    },
                ))
            }
            ChartKind::Line => chart_container!(crate::charts::line::LineChart::new(
                MAX_SAMPLES,
                &self.samples,
                &VecDeque::new(),
                Some(self.max_temp),
                if self.disabled {
                    &*DISABLED_COLORS
                } else {
                    &self.config.colors
                },
            )),
            ChartKind::Heat => chart_container!(crate::charts::heat::HeatChart::new(
                MAX_SAMPLES,
                &self.samples,
                Some(self.max_temp),
                if self.disabled {
                    &*DISABLED_COLORS
                } else {
                    &self.config.colors
                },
            )),
        }
    }

    #[cfg(not(feature = "lyon_charts"))]
    pub fn chart(&'_ self) -> cosmic::widget::Container<'_, crate::app::Message, Theme, Renderer> {
        let colors = if self.disabled {
            &self.disabled_colors
        } else {
            &self.svg_colors
        };
        let svg = match self.config.chart {
            ChartKind::Ring => {
                let latest = self.latest_sample();
                let mut value = self.to_string_raw();

                if value.len() < 3 {
                    value.push('°');
                }

                let max = 100.0;
                let offset_max = max - self.config.min_temp;
                let percentage: u8 = ((latest - self.config.min_temp) / offset_max * 100.0)
                    .max(0.0)
                    .round()
                    .clamp(0.0, max) as u8;

                crate::svg_graph::ring(&value, percentage, None, colors)
            }
            ChartKind::Line => {
                if self.config.min_temp == 0.0 {
                    crate::svg_graph::line(&self.samples, self.max_temp, colors)
                } else {
                    let normalized =
                        super::normalize_temps_dynamic(&self.samples, self.config.min_temp);
                    crate::svg_graph::line(&normalized, self.max_temp, colors)
                }
            }
            ChartKind::Heat => {
                if self.config.min_temp == 0.0 {
                    crate::svg_graph::heat(&self.samples, self.max_temp as u64, colors)
                } else {
                    let normalized =
                        super::normalize_temps_dynamic(&self.samples, self.config.min_temp);
                    crate::svg_graph::heat(&normalized, self.max_temp as u64, colors)
                }
            }
            ChartKind::StackedBars => {
                log::error!("StackedBars not supported for GpuTemp");
                INVALID_IMG.to_string()
            }
        };
        super::svg_icon_container::<Message>(svg)
    }

    pub fn latest_sample(&self) -> f64 {
        *self.samples.back().unwrap_or(&0f64)
    }

    pub fn graph_kind(&self) -> crate::config::ChartKind {
        self.config.chart
    }

    pub fn set_graph_kind(&mut self, kind: crate::config::ChartKind) {
        self.config.chart = kind;
    }

    pub fn update(&mut self, sample: u32) {
        let new_val = f64::from(sample) / 1000.0;
        self.samples.push_back(new_val);
    }

    pub fn to_string_raw(&self) -> String {
        let current_val = self.latest_sample();
        match self.config.unit {
            TempUnit::Celsius => current_val.trunc().to_string(),
            TempUnit::Farenheit => (current_val * 9.0 / 5.0 + 32.0).trunc().to_string(),
            TempUnit::Kelvin => (current_val + 273.15).trunc().to_string(),
            TempUnit::Rankine => (current_val * 9.0 / 5.0 + 491.67).trunc().to_string(),
        }
    }
}

impl DemoGraph for TempGraph {
    fn demo(&self) -> String {
        match self.config.chart {
            ChartKind::Ring => {
                // show a number of 40% of max
                let val = 40;
                let percentage: u8 = 40;
                crate::svg_graph::ring(&format!("{val}"), percentage, None, &self.svg_colors)
            }
            ChartKind::Line => crate::svg_graph::line(
                &std::collections::VecDeque::from(DEMO_SAMPLES),
                100.0,
                &self.svg_colors,
            ),
            ChartKind::Heat => crate::svg_graph::heat(
                &std::collections::VecDeque::from(HEAT_DEMO_SAMPLES),
                100,
                &self.svg_colors,
            ),
            ChartKind::StackedBars => {
                log::error!("StackedBars not supported for GpuTemp");
                INVALID_IMG.to_string()
            }
        }
    }

    fn colors(&self) -> &ChartColors {
        self.config.colors()
    }

    fn set_colors(&mut self, colors: &ChartColors) {
        *self.config.colors_mut() = *colors;
        self.svg_colors.set_colors(colors);
    }

    fn color_choices(&self) -> Vec<(&'static str, ColorVariant)> {
        match self.config.chart {
            ChartKind::Line => (*super::COLOR_CHOICES_LINE).into(),
            ChartKind::Ring => (*super::COLOR_CHOICES_RING).into(),
            ChartKind::Heat => (*super::COLOR_CHOICES_HEAT).into(),
            ChartKind::StackedBars => panic!("StackedBars not supported for GpuTemp"),
        }
    }

    fn id(&self) -> Option<String> {
        Some(self.id.clone())
    }

    fn kind(&self) -> ChartKind {
        self.config.chart
    }
}

use std::fmt;

impl fmt::Display for TempGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let current_val = self.latest_sample();
        if self.disabled || current_val <= 0.0 {
            match self.config.unit {
                TempUnit::Celsius => write!(f, "--°C"),
                TempUnit::Farenheit => write!(f, "---°F"),
                TempUnit::Kelvin => write!(f, "---K"),
                TempUnit::Rankine => write!(f, "---°R"),
            }
        } else {
            match self.config.unit {
                TempUnit::Celsius => write!(f, "{}°C", current_val.trunc()),
                TempUnit::Farenheit => write!(f, "{}°F", (current_val * 9.0 / 5.0 + 32.0).trunc()),
                TempUnit::Kelvin => write!(f, "{}K", (current_val + 273.15).trunc()),
                TempUnit::Rankine => write!(f, "{}°R", (current_val * 9.0 / 5.0 + 491.67).trunc()),
            }
        }
    }
}

impl DemoGraph for VramGraph {
    fn demo(&self) -> String {
        match self.config.chart {
            ChartKind::Ring => {
                // show a number of 40% of max
                let val = 40;
                let percentage: u8 = 40;
                crate::svg_graph::ring(&format!("{val}"), percentage, None, &self.svg_colors)
            }
            ChartKind::Line => crate::svg_graph::line(
                &std::collections::VecDeque::from(DEMO_SAMPLES),
                32.0,
                &self.svg_colors,
            ),
            _ => {
                log::error!("VRAM Graph type not supported {:?}", self.config.chart);
                INVALID_IMG.to_string()
            }
        }
    }

    fn colors(&self) -> &ChartColors {
        self.config.colors()
    }

    fn set_colors(&mut self, colors: &ChartColors) {
        *self.config.colors_mut() = *colors;
        self.svg_colors.set_colors(colors);
    }

    fn color_choices(&self) -> Vec<(&'static str, ColorVariant)> {
        if self.config.chart == ChartKind::Line {
            (*super::COLOR_CHOICES_LINE).into()
        } else {
            (*super::COLOR_CHOICES_RING).into()
        }
    }

    fn id(&self) -> Option<String> {
        Some(self.id.clone())
    }

    fn kind(&self) -> ChartKind {
        self.config.chart
    }
}

pub struct Gpu {
    gpu_if: Box<dyn GpuIf>,
    pub gpu: GpuGraph,
    pub vram: VramGraph,
    pub temp: TempGraph,
    is_laptop: bool,
    config: GpuConfig,
}

impl Gpu {
    pub fn new(gpu_if: Box<dyn GpuIf>) -> Self {
        let total = gpu_if.vram_total();
        let id = gpu_if.id();

        Gpu {
            gpu_if,
            gpu: GpuGraph::new(&id),
            vram: VramGraph::new(&id, total as f64 / 1_073_741_824.0),
            temp: TempGraph::new(&id),
            is_laptop: false,
            config: GpuConfig::default(),
        }
    }

    pub fn update_config(&mut self, config: &dyn Any, refresh_rate: u32) {
        if let Some(cfg) = config.downcast_ref::<GpuConfig>() {
            self.config = cfg.clone();
            self.gpu.update_config(&cfg.usage, refresh_rate);
            self.vram.update_config(&cfg.vram, refresh_rate);
            self.temp.update_config(&cfg.temp, refresh_rate);
        }
    }

    pub fn name(&self) -> String {
        self.gpu_if.as_ref().name().clone()
    }

    pub fn id(&self) -> String {
        self.gpu_if.as_ref().id().clone()
    }

    pub fn set_laptop(&mut self) {
        self.is_laptop = true;
    }

    pub fn demo_graph(&self, device: DeviceKind) -> Box<dyn DemoGraph> {
        match device {
            DeviceKind::Gpu => {
                let mut dmo = GpuGraph::new(&self.id());
                dmo.update_config(&self.gpu.config, 0);
                Box::new(dmo)
            }
            DeviceKind::Vram => {
                let mut dmo = VramGraph::new(&self.id(), self.vram.total);
                dmo.update_config(&self.vram.config, 0);
                Box::new(dmo)
            }
            DeviceKind::GpuTemp => {
                let mut dmo = TempGraph::new(&self.id());
                dmo.update_config(&self.temp.config, 0);
                Box::new(dmo)
            }
            _ => {
                log::error!("Gpu::demo_graph({device:?}) Wrong device kind");
                panic!("Gpu::demo_graph({device:?}) Wrong device kind")
            }
        }
    }

    pub fn update(&mut self) {
        if self.gpu_if.is_active() {
            if let Ok(sample) = self.gpu_if.usage() {
                self.gpu.update(sample);
            }
            if let Ok(sample) = self.gpu_if.vram_used() {
                self.vram.update(sample);
            }
            if let Ok(sample) = self.gpu_if.temperature() {
                self.temp.update(sample);
            }
        }
    }

    pub fn restart(&mut self) {
        info!("Restarting {}", self.name());
        self.gpu_if.restart();
        self.gpu.disabled = false;
        self.vram.disabled = false;
        self.temp.disabled = false;
    }

    pub fn stop(&mut self) {
        info!("Stopping {}", self.name());
        self.gpu_if.stop();
        self.gpu.clear();
        self.vram.clear();
        self.temp.clear();
        self.gpu.disabled = true;
        self.vram.disabled = true;
        self.temp.disabled = true;
    }

    pub fn is_active(&self) -> bool {
        self.gpu_if.is_active()
    }

    fn settings_usage_ui(
        &'_ self,
        config: &crate::config::GpuUsageConfig,
    ) -> Element<'_, crate::app::Message> {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();

        let mut gpu_elements = Vec::new();

        let usage = self.gpu.to_string();
        gpu_elements.push(Element::from(
            column!(
                Container::new(self.gpu.chart().width(60).height(60))
                    .width(90)
                    .align_x(Alignment::Center),
                cosmic::widget::text::body(usage.to_string())
                    .width(90)
                    .align_x(Alignment::Center)
            )
            .padding(cosmic::theme::spacing().space_xs)
            .align_x(Alignment::Center),
        ));

        let gpu_kind = self.gpu.graph_kind();
        let selected: Option<usize> = Some(gpu_kind.into());
        let id = self.id();
        gpu_elements.push(Element::from(
            column!(
                settings::item(
                    fl!("enable-chart"),
                    toggler(config.chart_visible()).on_toggle(move |value| {
                        Message::GpuToggleChart(self.id(), DeviceKind::Gpu, value)
                    }),
                ),
                settings::item(
                    fl!("enable-label"),
                    toggler(config.label_visible()).on_toggle(move |value| {
                        Message::GpuToggleLabel(self.id(), DeviceKind::Gpu, value)
                    }),
                ),
                row!(
                    widget::text::body(fl!("chart-type")),
                    widget::dropdown(&self.gpu.graph_options, selected, move |m| {
                        Message::GpuSelectGraphType(id.clone(), DeviceKind::Gpu, m.into())
                    },)
                    .width(70),
                    widget::horizontal_space(),
                    widget::button::standard(fl!("change-colors")).on_press(
                        Message::ColorPickerOpen(DeviceKind::Gpu, gpu_kind, Some(self.id())),
                    )
                )
                .align_y(Center),
            )
            .spacing(cosmic.space_xs()),
        ));

        column![
            widget::text::heading(fl!("gpu-title-usage")),
            Row::with_children(gpu_elements)
                .align_y(Alignment::Center)
                .spacing(cosmic.space_xs())
        ]
        .spacing(cosmic::theme::spacing().space_xs)
        .into()
    }

    fn settings_vram_ui(
        &'_ self,
        config: &crate::config::GpuVramConfig,
    ) -> Element<'_, crate::app::Message> {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();

        // VRAM load
        let mut vram_elements = Vec::new();
        let vram = self.vram.string(false);
        vram_elements.push(Element::from(
            column!(
                Container::new(self.vram.chart().width(60).height(60))
                    .width(90)
                    .align_x(Alignment::Center),
                cosmic::widget::text::body(vram.to_string())
                    .width(90)
                    .align_x(Alignment::Center)
            )
            .padding(cosmic::theme::spacing().space_xs)
            .align_x(Alignment::Center),
        ));

        let selected: Option<usize> = Some(self.vram.graph_kind().into());
        let mem_kind = self.vram.graph_kind();
        let id = self.id();
        vram_elements.push(Element::from(
            column!(
                settings::item(
                    fl!("enable-chart"),
                    toggler(config.chart_visible()).on_toggle(|value| {
                        Message::GpuToggleChart(self.id(), DeviceKind::Vram, value)
                    }),
                ),
                settings::item(
                    fl!("enable-label"),
                    toggler(config.label_visible()).on_toggle(|value| {
                        Message::GpuToggleLabel(self.id(), DeviceKind::Vram, value)
                    }),
                ),
                row!(
                    widget::text::body(fl!("chart-type")),
                    widget::dropdown(&self.vram.graph_options, selected, move |m| {
                        Message::GpuSelectGraphType(id.clone(), DeviceKind::Vram, m.into())
                    },)
                    .width(70),
                    widget::horizontal_space(),
                    widget::button::standard(fl!("change-colors")).on_press(
                        Message::ColorPickerOpen(DeviceKind::Vram, mem_kind, Some(self.id())),
                    )
                )
                .align_y(Center),
            )
            .spacing(cosmic.space_xs()),
        ));

        column![
            widget::text::heading(fl!("gpu-title-vram")),
            Row::with_children(vram_elements)
                .align_y(Alignment::Center)
                .spacing(cosmic.space_xs())
        ]
        .spacing(cosmic::theme::spacing().space_xs)
        .into()
    }

    fn settings_temp_ui(
        &'_ self,
        config: &crate::config::GpuTempConfig,
    ) -> Element<'_, crate::app::Message> {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();

        // GPU temperature
        let mut temp_elements = Vec::new();
        let temp = self.temp.to_string();
        temp_elements.push(Element::from(
            column!(
                Container::new(self.temp.chart().width(60).height(60))
                    .width(90)
                    .align_x(Alignment::Center),
                cosmic::widget::text::body(temp.to_string())
                    .width(90)
                    .align_x(Alignment::Center)
            )
            .padding(cosmic::theme::spacing().space_xs)
            .align_x(Alignment::Center),
        ));

        let selected: Option<usize> = Some(self.temp.graph_kind().into());
        let selected_unit: Option<usize> = Some(self.temp.config.unit.into());
        let temp_kind = self.temp.graph_kind();
        let id1 = self.id();
        let id2 = self.id();
        let id3 = self.id();
        let min_temp_val = config.min_temp;

        let min_temp_input = {
            let val_string = min_temp_val.to_string();
            widget::text_input("", val_string)
                .width(100)
                .on_input(move |temp_str| {
                    let temp = if temp_str.is_empty() {
                        0.0
                    } else {
                        temp_str.parse::<f64>().unwrap_or(min_temp_val)
                    };
                    Message::GpuTempMinTempChanged(id3.clone(), temp)
                })
        };

        temp_elements.push(Element::from(
            column!(
                settings::item(
                    fl!("enable-chart"),
                    toggler(config.chart_visible()).on_toggle(|value| {
                        Message::GpuToggleChart(self.id(), DeviceKind::GpuTemp, value)
                    }),
                ),
                settings::item(
                    fl!("enable-label"),
                    toggler(config.label_visible()).on_toggle(|value| {
                        Message::GpuToggleLabel(self.id(), DeviceKind::GpuTemp, value)
                    }),
                ),
                settings::item(
                    fl!("temperature-unit"),
                    widget::dropdown(&self.temp.unit_options, selected_unit, move |m| {
                        Message::SelectGpuTempUnit(id1.clone(), m.into())
                    },)
                ),
                row!(
                    widget::text::body(fl!("chart-type")),
                    widget::dropdown(&self.temp.graph_options, selected, move |m| {
                        Message::GpuSelectGraphType(id2.clone(), DeviceKind::GpuTemp, m.into())
                    },)
                    .width(70),
                    widget::horizontal_space(),
                    widget::button::standard(fl!("change-colors")).on_press(
                        Message::ColorPickerOpen(DeviceKind::GpuTemp, temp_kind, Some(self.id())),
                    )
                )
                .align_y(Center),
            )
            .push(settings::item(fl!("min-temperature"), min_temp_input))
            .spacing(cosmic.space_xs()),
        ));

        column![
            widget::text::heading(fl!("gpu-title-temperature")),
            Row::with_children(temp_elements)
                .align_y(Alignment::Center)
                .spacing(cosmic.space_xs())
        ]
        .spacing(cosmic::theme::spacing().space_xs)
        .into()
    }

    pub fn settings_ui(
        &'_ self,
        config: &crate::config::GpuConfig,
    ) -> cosmic::Element<'_, crate::app::Message> {
        let battery_disable = if self.is_laptop {
            Some(
                settings::item(
                    fl!("settings-disable-on-battery"),
                    widget::checkbox("", config.pause_on_battery).on_toggle(move |value| {
                        Message::ToggleDisableOnBattery(self.id().clone(), value)

                        //widget::toggler(config.pause_on_battery).on_toggle(move |value| {
                        //   Message::ToggleDisableOnBattery(self.id().clone(), value)
                    }),
                )
                .width(340),
            )
        } else {
            None
        };

        let usage = self.settings_usage_ui(&config.usage);
        let vram = self.settings_vram_ui(&config.vram);

        let stacked = if config.vram.label_visible() && config.usage.label_visible() {
            Some(settings::item(
                fl!("settings-gpu-stack-labels"),
                row!(
                    widget::toggler(config.stack_labels).on_toggle(move |value| {
                        Message::GpuToggleStackLabels(self.id().clone(), value)
                    })
                ),
            ))
        } else {
            None
        };

        let temp = self.settings_temp_ui(&config.temp);

        Column::new()
            .push_maybe(battery_disable)
            .push(usage)
            .push(temp)
            .push(vram)
            .push_maybe(stacked)
            .spacing(cosmic::theme::spacing().space_xs)
            .into()
    }
}

pub fn list_gpus() -> Vec<Gpu> {
    let mut v: Vec<Gpu> = Vec::new();

    v.extend(IntelGpu::get_gpus());
    v.extend(NvidiaGpu::get_gpus());
    v.extend(AmdGpu::get_gpus());
    v
}

const DEMO_SAMPLES: [f64; 21] = [
    0.0,
    12.689857482910156,
    12.642768859863281,
    12.615306854248047,
    12.658184051513672,
    12.65273666381836,
    12.626102447509766,
    12.624862670898438,
    12.613967895507813,
    12.619949340820313,
    19.061111450195313,
    21.691085815429688,
    21.810935974121094,
    21.28915786743164,
    22.041973114013672,
    21.764171600341797,
    21.89263916015625,
    15.258216857910156,
    14.770732879638672,
    14.496528625488281,
    13.892818450927734,
];

const HEAT_DEMO_SAMPLES: [f64; 21] = [
    41.0, 42.0, 43.5, 45.0, 48.0, 51.0, 55.0, 57.0, 59.5, 62.0, 64.0, 67.0, 70.0, 74.0, 78.0, 83.0,
    87.0, 90.0, 95.0, 98.0, 100.0,
];
