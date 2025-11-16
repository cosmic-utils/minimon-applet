use cosmic::Element;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use crate::{
    config::{ColorVariant, GpuConfig},
    fl,
};

const INVALID_IMG: &str = r#"
<svg xmlns="http://www.w3.org/2000/svg"
     viewBox="0 0 24 24" width="24" height="24"
     role="img" aria-label="Invalid image"
     fill="none" stroke="currentColor" stroke-width="2"
     style="color:#e53935">
  <title>Invalid image</title>
  <line x1="6" y1="6" x2="18" y2="18" stroke-linecap="round"/>
  <line x1="18" y1="6" x2="6" y2="18" stroke-linecap="round"/>
</svg>"#;

#[cfg(feature = "lyon_charts")]
macro_rules! chart_container {
    ($chart:expr) => {
        Container::new(cosmic::widget::Canvas::new($chart))
    };
}

pub static COLOR_CHOICES_RING: LazyLock<[(&'static str, ColorVariant); 4]> = LazyLock::new(|| {
    [
        (fl!("graph-ring-r1").leak(), ColorVariant::Graph1),
        (fl!("graph-ring-r2").leak(), ColorVariant::Graph2),
        (fl!("graph-ring-back").leak(), ColorVariant::Background),
        (fl!("graph-ring-text").leak(), ColorVariant::Text),
    ]
});

pub static COLOR_CHOICES_LINE: LazyLock<[(&'static str, ColorVariant); 3]> = LazyLock::new(|| {
    [
        (fl!("graph-line-graph").leak(), ColorVariant::Graph1),
        (fl!("graph-line-back").leak(), ColorVariant::Background),
        (fl!("graph-line-frame").leak(), ColorVariant::Frame),
    ]
});

pub static COLOR_CHOICES_HEAT: LazyLock<[(&'static str, ColorVariant); 2]> = LazyLock::new(|| {
    [
        (fl!("graph-line-back").leak(), ColorVariant::Background),
        (fl!("graph-line-frame").leak(), ColorVariant::Frame),
    ]
});

pub static UNIT_OPTIONS: LazyLock<[&'static str; 4]> = LazyLock::new(|| {
    [
        fl!("temperature-unit-celsius").leak(),
        fl!("temperature-unit-fahrenheit").leak(),
        fl!("temperature-unit-kelvin").leak(),
        fl!("temperature-unit-rankine").leak(),
    ]
});

static GRAPH_OPTIONS_RING_LINE: LazyLock<[&'static str; 2]> =
    LazyLock::new(|| [fl!("graph-type-ring").leak(), fl!("graph-type-line").leak()]);

static GRAPH_OPTIONS_RING_LINE_HEAT: LazyLock<[&'static str; 3]> = LazyLock::new(|| {
    [
        fl!("graph-type-ring").leak(),
        fl!("graph-type-line").leak(),
        fl!("graph-type-heat").leak(),
    ]
});

use crate::{colorpicker::DemoGraph, config::ChartKind};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TempUnit {
    Celsius,
    Farenheit,
    Kelvin,
    Rankine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuVariant {
    Amd,
    Intel,
}

use std::any::Any;
pub trait Sensor {
    fn update_config(&mut self, config: &dyn Any, refresh_rate: u32);
    fn graph_kind(&self) -> ChartKind;
    fn set_graph_kind(&mut self, kind: ChartKind);
    fn update(&mut self);
    fn demo_graph(&self) -> Box<dyn DemoGraph>;
    fn chart(
        &'_ self,
        height_hint: u16,
        width_hint: u16,
    ) -> cosmic::widget::Container<'_, crate::app::Message, cosmic::Theme, cosmic::Renderer>;
    fn settings_ui(&'_ self) -> Element<'_, crate::app::Message>;
}

pub mod cpu;
pub mod cputemp;
pub mod disks;
pub mod gpu;
pub mod gpus;
pub mod memory;
pub mod network;

impl From<usize> for TempUnit {
    fn from(index: usize) -> Self {
        match index {
            0 => TempUnit::Celsius,
            1 => TempUnit::Farenheit,
            2 => TempUnit::Kelvin,
            3 => TempUnit::Rankine,
            _ => { log::error!("Invalid index for TempUnit"); TempUnit::Celsius},
        }
    }
}

impl From<TempUnit> for usize {
    fn from(kind: TempUnit) -> Self {
        match kind {
            TempUnit::Celsius => 0,
            TempUnit::Farenheit => 1,
            TempUnit::Kelvin => 2,
            TempUnit::Rankine => 3,
        }
    }
}
