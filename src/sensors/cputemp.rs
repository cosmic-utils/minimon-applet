use crate::{
    colorpicker::DemoGraph,
    config::{ChartColors, ChartKind, ColorVariant, CpuTempConfig, DeviceKind},
    fl,
    sensors::INVALID_IMG,
    svg_graph::SvgColors,
};
use cosmic::{Element, iced::Alignment::Center, widget::Container};

use cosmic::widget;
use cosmic::widget::{settings, toggler};

use cosmic::{
    iced::{
        Alignment,
        widget::{column, row},
    },
    iced_widget::Row,
};
use log::info;

use crate::app::Message;
use std::any::Any;

use bounded_vec_deque::BoundedVecDeque;
use std::{
    fs,
    path::{Path, PathBuf},
};

use std::fs::read_dir;
use std::io;

use super::{CpuVariant, Sensor, TempUnit};

const MAX_SAMPLES: usize = 21;

#[derive(Debug)]
pub struct HwmonTemp {
    pub temp_paths: Vec<PathBuf>,
    pub crit_temp: f64,
    pub cpu: super::CpuVariant,
}

impl HwmonTemp {
    /// Initialize and return the most relevant CPU temperature sensors
    pub fn find_cpu_sensor() -> io::Result<Option<HwmonTemp>> {
        info!("Find CPU temperature sensor");
        let hwmon_base = Path::new("/sys/class/hwmon");

        for entry in read_dir(hwmon_base)? {
            let hwmon = entry?.path();
            let name_path = hwmon.join("name");

            let Ok(name) = fs::read_to_string(&name_path) else {
                continue;
            };
            let name = name.trim().to_lowercase();
            info!("  path: {name_path:?}. name: {name}");

            if name.contains("coretemp")
                || name.contains("k10temp")
                || name.contains("cpu")
                || name.contains("zenpower")
            {
                let mut tdie: Option<(PathBuf, String)> = None;
                let mut tctl: Option<(PathBuf, String)> = None;
                let mut ccd: Option<(PathBuf, String)> = None;
                let mut core_fallbacks = vec![];

                for i in 0..100 {
                    let label_path = hwmon.join(format!("temp{i}_label"));
                    let input_path = hwmon.join(format!("temp{i}_input"));

                    if !input_path.exists() {
                        continue;
                    }
                    if let Ok(label) = fs::read_to_string(&label_path) {
                        let label = label.trim();

                        if label.eq_ignore_ascii_case("Tdie") {
                            info!("  found sensor {label_path:?} {label}");
                            tdie = Some((input_path.clone(), label.to_string()));
                        } else if label.eq_ignore_ascii_case("Tctl") {
                            info!("  found sensor {label_path:?} {label}");
                            tctl = Some((input_path.clone(), label.to_string()));
                        } else if label.eq_ignore_ascii_case("ccd") {
                            info!("  found sensor {label_path:?} {label}");
                            ccd = Some((input_path.clone(), label.to_string()));
                        } else if label.starts_with("Core") || label.contains("Package") {
                            info!("  found sensor {label_path:?} {label}");
                            core_fallbacks.push((input_path.clone(), label.to_string()));
                        }
                    }
                }

                // Prioritize Tdie > Tctl
                if let Some((path, _label)) = tdie.or(ccd).or(tctl) {
                    let crit_path = hwmon.join("temp1_crit");
                    let crit_temp = fs::read_to_string(&crit_path)
                        .ok()
                        .and_then(|v| v.trim().parse::<f64>().ok())
                        .map_or(100.0, |v| v / 1000.0);

                    return Ok(Some(HwmonTemp {
                        temp_paths: vec![path.clone()],
                        crit_temp,
                        cpu: CpuVariant::Amd,
                    }));
                } else if !core_fallbacks.is_empty() {
                    return Ok(Some(HwmonTemp {
                        temp_paths: core_fallbacks.iter().map(|(p, _)| p.clone()).collect(),
                        crit_temp: 100.0,
                        cpu: CpuVariant::Intel,
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Read current max temperature from all tracked sensor paths
    pub fn read_temp(&self) -> io::Result<f32> {
        let mut max_temp = f32::MIN;

        for path in &self.temp_paths {
            let raw = fs::read_to_string(path)?;
            let millideg: i32 = raw.trim().parse().map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("Parse error: {e}"))
            })?;
            let temp_c = millideg as f32 / 1000.0;
            max_temp = max_temp.max(temp_c);
        }

        Ok(max_temp)
    }
}

#[derive(Debug)]
pub struct CpuTemp {
    hwmon_temp: Option<HwmonTemp>,
    pub samples: BoundedVecDeque<f64>,
    graph_options: Vec<&'static str>,
    unit_options: Vec<&'static str>,
    /// colors cached so we don't need to convert to string every time
    svg_colors: SvgColors,
    config: CpuTempConfig,
}

impl DemoGraph for CpuTemp {
    fn demo(&self) -> String {
        match self.config.chart {
            ChartKind::Ring => {
                // show a number of 40% of max
                crate::svg_graph::ring(&format!("40°"), 40, None, &self.svg_colors)
            }

            ChartKind::Line => crate::svg_graph::line(
                &std::collections::VecDeque::from(DEMO_SAMPLES),
                100.0,
                &self.svg_colors,
            ),
            ChartKind::Heat => crate::svg_graph::heat(
                &std::collections::VecDeque::from(DEMO_SAMPLES),
                100,
                &self.svg_colors,
            ),
            ChartKind::StackedBars => {
                log::error!("StackedBars not supported for CpuTemp");
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
            ChartKind::StackedBars => panic!("StackedBars not supported for CpuTemp"),
        }
    }

    fn id(&self) -> Option<String> {
        None
    }

    fn kind(&self) -> ChartKind {
        self.config.chart
    }
}

impl Sensor for CpuTemp {
    fn update_config(&mut self, config: &dyn Any, _refresh_rate: u32) {
        if let Some(cfg) = config.downcast_ref::<CpuTempConfig>() {
            self.config = cfg.clone();
            self.svg_colors.set_colors(cfg.colors());
        }
    }

    fn graph_kind(&self) -> ChartKind {
        self.config.chart
    }

    fn set_graph_kind(&mut self, kind: ChartKind) {
        assert!(kind == ChartKind::Line || kind == ChartKind::Ring || kind == ChartKind::Heat);
        self.config.chart = kind;
    }

    fn update(&mut self) {
        if let Some(hw) = &self.hwmon_temp {
            match hw.read_temp() {
                Ok(temp) => {
                    self.samples.push_back(f64::from(temp));
                }
                Err(e) => info!("Error reading temp data {e:?}"),
            }
        }
    }

    fn demo_graph(&self) -> Box<dyn DemoGraph> {
        let mut dmo = CpuTemp::default();
        dmo.update_config(&self.config, 0);
        Box::new(dmo)
    }

    #[cfg(feature = "lyon_charts")]
    fn chart(
        &self,
    ) -> cosmic::widget::Container<crate::app::Message, cosmic::Theme, cosmic::Renderer> {
        let mut max: f64 = 100.0;
        if let Some(hwmon) = &self.hwmon_temp {
            max = hwmon.crit_temp;
        }
        match self.config.kind {
            ChartKind::Ring => {
                let latest = self.latest_sample();
                let mut value = self.to_string();

                // remove the °C/°F/°R/K unit if there's not enough space (assuming temp stays below 282°C = 1000°R)
                while value.len() > 3 {
                    let _ = value.pop();
                }
                chart_container!(crate::charts::ring::RingChart::new(
                    latest as f32,
                    &value,
                    &self.config.colors,
                ))
            }
            ChartKind::Line => chart_container!(crate::charts::line::LineChart::new(
                MAX_SAMPLES,
                &self.samples,
                &VecDeque::new(),
                Some(max),
                &self.config.colors,
            )),
            ChartKind::Heat => chart_container!(crate::charts::heat::HeatChart::new(
                MAX_SAMPLES,
                &self.samples,
                Some(max),
                &self.config.colors,
            )),
        }
    }

    #[cfg(not(feature = "lyon_charts"))]
    fn chart(
        &'_ self,
        _height_hint: u16,
        _width_hint: u16,
    ) -> cosmic::widget::Container<'_, crate::app::Message, cosmic::Theme, cosmic::Renderer> {
        let mut max: f64 = 100.0;
        if let Some(hwmon) = &self.hwmon_temp {
            max = hwmon.crit_temp;
        }
        let svg = match self.config.chart {
            ChartKind::Ring => {
                let latest = self.latest_sample();
                let mut value = self.to_string_raw();

                if value.len() < 3 {
                    value.push('°');
                }

                let offset_max = max - self.config.min_temp;
                let percentage: u8 = ((latest - self.config.min_temp) / offset_max * 100.0)
                    .max(0.0)
                    .round()
                    .clamp(0.0, max) as u8;

                crate::svg_graph::ring(&value, percentage, None, &self.svg_colors)
            }
            ChartKind::Line => {
                if self.config.min_temp == 0.0 {
                    crate::svg_graph::line(&self.samples, max, &self.svg_colors)
                } else {
                    let normalized =
                        super::normalize_temps_dynamic(&self.samples, self.config.min_temp);
                    crate::svg_graph::line(&normalized, max, &self.svg_colors)
                }
            }
            ChartKind::Heat => {
                if self.config.min_temp == 0.0 {
                    crate::svg_graph::heat(&self.samples, max as u64, &self.svg_colors)
                } else {
                    let normalized =
                        super::normalize_temps_dynamic(&self.samples, self.config.min_temp);
                    crate::svg_graph::heat(&normalized, max as u64, &self.svg_colors)
                }
            }
            ChartKind::StackedBars => {
                log::error!("StackedBars not supported for CpuTemp");
                INVALID_IMG.to_string()
            }
        };
        super::svg_icon_container::<Message>(svg)
    }

    fn settings_ui(&'_ self) -> Element<'_, crate::app::Message> {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();

        let mut temp_elements = Vec::new();

        let temp = self.to_string();

        temp_elements.push(Element::from(
            column!(
                Container::new(self.chart(60, 60).width(60).height(60))
                    .width(90)
                    .align_x(Alignment::Center),
                cosmic::widget::text::body(temp.to_string())
                    .width(90)
                    .align_x(Alignment::Center)
            )
            .padding(5)
            .align_x(Alignment::Center),
        ));

        let selected_graph: Option<usize> = Some(self.graph_kind().into());
        let selected_unit: Option<usize> = Some(self.config.unit.into());

        let config = &self.config;
        let temp_kind = self.graph_kind();
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
                    Message::CpuTempMinTempChanged(temp)
                })
        };

        temp_elements.push(Element::from(
            column!(
                settings::item(
                    fl!("enable-chart"),
                    toggler(config.chart_visible())
                        .on_toggle(|value| { Message::ToggleCpuTempChart(value) }),
                ),
                settings::item(
                    fl!("enable-label"),
                    toggler(config.label_visible())
                        .on_toggle(|value| { Message::ToggleCpuTempLabel(value) }),
                ),
                settings::item(
                    fl!("temperature-unit"),
                    widget::dropdown(&self.unit_options, selected_unit, |m| {
                        Message::SelectCpuTempUnit(m.into())
                    },)
                ),
                row!(
                    widget::text::body(fl!("chart-type")),
                    widget::dropdown(&self.graph_options, selected_graph, |m| {
                        Message::SelectGraphType(DeviceKind::CpuTemp, m.into())
                    },)
                    .width(70),
                    widget::horizontal_space(),
                    widget::button::standard(fl!("change-colors")).on_press(
                        Message::ColorPickerOpen(DeviceKind::CpuTemp, temp_kind, None)
                    ),
                )
                .align_y(Center),
            )
            .push(settings::item(fl!("min-temperature"), min_temp_input))
            .spacing(cosmic.space_xs()),
        ));

        let mut expl = String::with_capacity(128);
        if let Some(hw) = &self.hwmon_temp {
            if hw.cpu == super::CpuVariant::Amd {
                expl.push_str(&fl!("cpu-temp-amd"));
            } else {
                expl.push_str(&fl!("cpu-temp-intel"));
            }
        }

        column!(
            Element::from(widget::text::body(expl)),
            Element::from(
                Row::with_children(temp_elements)
                    .align_y(Alignment::Center)
                    .spacing(0)
            )
        )
        .spacing(10)
        .into()
    }
}

impl Default for CpuTemp {
    fn default() -> Self {
        let mut hwmon = None;

        match HwmonTemp::find_cpu_sensor() {
            Ok(hwmon_option) => {
                hwmon = hwmon_option;
                if hwmon.is_none() {
                    info!("CpuTemp:detect: No CPU Temp IF found.");
                }
            }
            Err(e) => info!("CpuTemp:detect: No CPU Temp IF found. {e:?}"),
        }

        let mut cpu = CpuTemp {
            hwmon_temp: hwmon,
            samples: BoundedVecDeque::from_iter(std::iter::repeat_n(0.0, MAX_SAMPLES), MAX_SAMPLES),
            graph_options: super::GRAPH_OPTIONS_RING_LINE_HEAT.to_vec(),
            svg_colors: SvgColors::new(&ChartColors::default()),
            unit_options: super::UNIT_OPTIONS.to_vec(),
            config: CpuTempConfig::default(),
        };
        cpu.set_colors(&ChartColors::default());
        cpu
    }
}

impl CpuTemp {
    // true if a CPU temperature hwmon path was found
    pub fn is_found(&self) -> bool {
        self.hwmon_temp.is_some()
    }

    pub fn latest_sample(&self) -> f64 {
        *self.samples.back().unwrap_or(&0f64)
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

use std::fmt;

impl fmt::Display for CpuTemp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let current_val = self.latest_sample();
        match self.config.unit {
            TempUnit::Celsius => write!(f, "{}°C", current_val.trunc()),
            TempUnit::Farenheit => write!(f, "{}°F", (current_val * 9.0 / 5.0 + 32.0).trunc()),
            TempUnit::Kelvin => write!(f, "{}K", (current_val + 273.15).trunc()),
            TempUnit::Rankine => write!(f, "{}°R", (current_val * 9.0 / 5.0 + 491.67).trunc()),
        }
    }
}

const DEMO_SAMPLES: [f64; 21] = [
    41.0, 42.0, 43.5, 45.0, 48.0, 51.0, 55.0, 57.0, 59.5, 62.0, 64.0, 67.0, 70.0, 74.0, 78.0, 83.0,
    87.0, 90.0, 95.0, 98.0, 100.0,
];
