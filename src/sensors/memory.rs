use cosmic::{Element, iced::Alignment::Center, widget::Container};
use sysinfo::{MemoryRefreshKind, System};

use crate::{
    colorpicker::DemoGraph,
    config::{ChartColors, ChartKind, ColorVariant, DeviceKind, MemoryConfig},
    fl,
    sensors::INVALID_IMG,
    svg_graph::SvgColors,
};

use cosmic::widget;
use cosmic::widget::{settings, toggler};
use std::any::Any;

use cosmic::{
    iced::{
        Alignment,
        widget::{column, row},
    },
    iced_widget::Row,
};

use crate::app::Message;

use bounded_vec_deque::BoundedVecDeque;
use std::fmt::Write;

use super::Sensor;

const MAX_SAMPLES: usize = 21;

pub static COLOR_CHOICES_DBL_RING: std::sync::LazyLock<[(&'static str, ColorVariant); 5]> =
    std::sync::LazyLock::new(|| {
        [
            (fl!("graph-memory-used").leak(), ColorVariant::Graph1),
            (fl!("graph-memory-allocated").leak(), ColorVariant::Graph3),
            (fl!("graph-ring-unused").leak(), ColorVariant::Graph2),
            (fl!("graph-ring-back").leak(), ColorVariant::Background),
            (fl!("graph-ring-text").leak(), ColorVariant::Text),
        ]
    });

pub static COLOR_CHOICES_LINE_STACKED: std::sync::LazyLock<[(&'static str, ColorVariant); 4]> =
    std::sync::LazyLock::new(|| {
        [
            (fl!("graph-memory-used").leak(), ColorVariant::Graph1),
            (fl!("graph-memory-allocated").leak(), ColorVariant::Graph3),
            (fl!("graph-line-back").leak(), ColorVariant::Background),
            (fl!("graph-line-frame").leak(), ColorVariant::Frame),
        ]
    });

#[derive(Debug)]
pub struct Memory {
    samples_used: BoundedVecDeque<f64>,
    samples_allocated: BoundedVecDeque<f64>,
    total_memory: f64,
    system: System,
    graph_options: Vec<&'static str>,
    /// colors cached so we don't need to convert to string every time
    svg_colors: SvgColors,
    config: MemoryConfig,
}

impl DemoGraph for Memory {
    fn demo(&self) -> String {
        match self.config.chart {
            ChartKind::Ring => {
                // show a number of 40% of max
                let val = 40;
                let percentage: u8 = 40;

                if self.config.show_allocated {
                    let percentage2: u8 = 80;
                    crate::svg_graph::ring(
                        &format!("{val}"),
                        percentage,
                        Some(percentage2),
                        &self.svg_colors,
                    )
                } else {
                    crate::svg_graph::ring(&format!("{val}"), percentage, None, &self.svg_colors)
                }
            }
            ChartKind::Line => {
                if self.config.show_allocated {
                    crate::svg_graph::line_stacked(
                        &std::collections::VecDeque::from(DEMO_SAMPLES),
                        &std::collections::VecDeque::from(DEMO_SAMPLES_ALLOCATED),
                        38.0,
                        &self.svg_colors,
                    )
                } else {
                    crate::svg_graph::line(
                        &std::collections::VecDeque::from(DEMO_SAMPLES),
                        38.0,
                        &self.svg_colors,
                    )
                }
            }
            _ => {
                log::error!(
                    "Graph type {:?} not supported for memory",
                    self.config.chart
                );
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
            if self.config.show_allocated {
                (*COLOR_CHOICES_LINE_STACKED).into()
            } else {
                (*super::COLOR_CHOICES_LINE).into()
            }
        } else if self.config.show_allocated {
            (*COLOR_CHOICES_DBL_RING).into()
        } else {
            (*super::COLOR_CHOICES_RING).into()
        }
    }

    fn id(&self) -> Option<String> {
        None
    }

    fn kind(&self) -> ChartKind {
        self.config.chart
    }
}

impl Sensor for Memory {
    fn update_config(&mut self, config: &dyn Any, _refresh_rate: u32) {
        if let Some(cfg) = config.downcast_ref::<MemoryConfig>() {
            self.config = cfg.clone();
            self.svg_colors.set_colors(cfg.colors());
        }
    }

    fn graph_kind(&self) -> ChartKind {
        self.config.chart
    }

    fn set_graph_kind(&mut self, kind: ChartKind) {
        assert!(kind == ChartKind::Line || kind == ChartKind::Ring);
        self.config.chart = kind;
    }

    fn update(&mut self) {
        let r = MemoryRefreshKind::nothing().with_ram();

        self.system.refresh_memory_specifics(r);
        let new_val_used: f64 = self.system.used_memory() as f64 / 1_073_741_824.0;
        let new_val_allocated: f64 =
            self.total_memory - (self.system.free_memory() as f64 / 1_073_741_824.0);
        self.samples_used.push_back(new_val_used);
        self.samples_allocated.push_back(new_val_allocated);
    }

    fn demo_graph(&self) -> Box<dyn DemoGraph> {
        let mut dmo = Memory::default();
        dmo.update_config(&self.config, 0);
        Box::new(dmo)
    }

    #[cfg(feature = "lyon_charts")]
    fn chart<'a>(
        &self,
    ) -> cosmic::widget::Container<crate::app::Message, cosmic::Theme, cosmic::Renderer> {
        if self.config.kind == ChartKind::Ring {
            let mut latest = self.latest_sample();
            let mut text = String::with_capacity(10);

            let mut pct: u64 = ((latest / self.total_memory) * 100.0) as u64;
            if pct > 100 {
                pct = 100;
            }

            // If set, convert to percentage
            if self.config.percentage {
                latest = (latest * 100.0) / self.total_memory;
            }

            if latest < 10.0 {
                write!(text, "{latest:.2}").unwrap();
            } else if latest <= 99.9 {
                write!(text, "{latest:.1}").unwrap();
            } else {
                write!(text, "100").unwrap();
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
                Some(self.total_memory),
                &self.config.colors,
            ))
        }
    }

    #[cfg(not(feature = "lyon_charts"))]
    fn chart(
        &'_ self,
        _height_hint: u16,
        _width_hint: u16,
    ) -> cosmic::widget::Container<'_, crate::app::Message, cosmic::Theme, cosmic::Renderer> {
        let svg = if self.config.chart == ChartKind::Ring {
            let mut latest = self.latest_sample();
            let mut value = String::with_capacity(10);

            let mut pct: u64 = ((latest / self.total_memory) * 100.0) as u64;
            if pct > 100 {
                pct = 100;
            }

            // If set, convert to percentage
            if self.config.percentage {
                latest = (latest * 100.0) / self.total_memory;
            }

            if latest < 10.0 {
                let _ = write!(value, "{latest:.2}");
            } else if latest <= 99.9 {
                let _ = write!(value, "{latest:.1}");
            } else {
                let _ = write!(value, "100");
            }

            if self.config.show_allocated {
                let mut pct_allocated: u64 =
                    ((self.latest_sample_allocated() / self.total_memory) * 100.0) as u64;
                if pct_allocated > 100 {
                    pct_allocated = 100;
                }
                crate::svg_graph::ring(
                    &value,
                    pct as u8,
                    Some(pct_allocated as u8),
                    &self.svg_colors,
                )
            } else {
                crate::svg_graph::ring(&value, pct as u8, None, &self.svg_colors)
            }
        } else if self.config.show_allocated {
            crate::svg_graph::line_stacked(
                &self.samples_used,
                &self.samples_allocated,
                self.total_memory,
                &self.svg_colors,
            )
        } else {
            crate::svg_graph::line(&self.samples_used, self.total_memory, &self.svg_colors)
        };
        super::svg_icon_container::<Message>(svg)
    }

    fn settings_ui(&'_ self) -> Element<'_, crate::app::Message> {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();

        let mem = self.to_string(false);

        let mut text = column!(
            cosmic::widget::text::body(mem)
                .width(90)
                .align_x(Alignment::Center)
        );

        if self.config.show_allocated {
            let allocated = format!("{:.1} GB", self.latest_sample_allocated());
            text = text.push(
                cosmic::widget::text::body(allocated)
                    .width(90)
                    .align_x(Alignment::Center),
            );
        }

        let mut mem_elements = Vec::new();
        mem_elements.push(Element::from(
            column!(
                Container::new(self.chart(60, 60).width(60).height(60))
                    .width(90)
                    .align_x(Alignment::Center),
                text
            )
            .padding(5)
            .align_x(Alignment::Center),
        ));

        let config = &self.config;
        let selected: Option<usize> = Some(self.graph_kind().into());
        let mem_kind = self.graph_kind();

        let expl = widget::text::caption(fl!("allocated-explanation"));

        mem_elements.push(Element::from(
            column!(
                settings::item(
                    fl!("enable-chart"),
                    toggler(config.chart_visible())
                        .on_toggle(|value| { Message::ToggleMemoryChart(value) }),
                ),
                settings::item(
                    fl!("memory-show-allocated"),
                    toggler(config.show_allocated).on_toggle(Message::ToggleMemoryAllocated)
                ),
                row!(widget::Space::with_width(15), expl),
                settings::item(
                    fl!("enable-label"),
                    toggler(config.label_visible())
                        .on_toggle(|value| { Message::ToggleMemoryLabel(value) }),
                ),
                settings::item(
                    fl!("memory-as-percentage"),
                    toggler(config.percentage).on_toggle(Message::ToggleMemoryPercentage),
                ),
                row!(
                    widget::text::body(fl!("chart-type")),
                    widget::dropdown(&self.graph_options, selected, move |m| {
                        Message::SelectGraphType(DeviceKind::Memory, m.into())
                    },)
                    .width(70),
                    widget::horizontal_space(),
                    widget::button::standard(fl!("change-colors"))
                        .on_press(Message::ColorPickerOpen(DeviceKind::Memory, mem_kind, None)),
                )
                .align_y(Center)
            )
            .spacing(cosmic.space_xs()),
        ));

        Row::with_children(mem_elements)
            .align_y(Alignment::Center)
            .spacing(0)
            .into()
    }
}

impl Default for Memory {
    fn default() -> Self {
        let mut system = System::new();
        system.refresh_memory();

        let total_memory: f64 = system.total_memory() as f64 / 1_073_741_824.0;
        log::info!(
            "System memory: {} / {:.2} GB",
            system.total_memory(),
            total_memory
        );

        // value and percentage are pre-allocated and reused as they're changed often.
        let mut percentage = String::with_capacity(6);
        percentage.push('0');

        let mut value = String::with_capacity(6);
        value.push('0');

        let mut memory = Memory {
            samples_used: BoundedVecDeque::from_iter(
                std::iter::repeat_n(0.0, MAX_SAMPLES),
                MAX_SAMPLES,
            ),
            samples_allocated: BoundedVecDeque::from_iter(
                std::iter::repeat_n(0.0, MAX_SAMPLES),
                MAX_SAMPLES,
            ),
            total_memory,
            system,
            config: MemoryConfig::default(),
            graph_options: super::GRAPH_OPTIONS_RING_LINE.to_vec(),
            svg_colors: SvgColors::new(&ChartColors::default()),
        };
        memory.set_colors(&ChartColors::default());
        memory
    }
}

impl Memory {
    pub fn latest_sample(&self) -> f64 {
        *self.samples_used.back().unwrap_or(&0f64)
    }

    pub fn latest_sample_allocated(&self) -> f64 {
        *self.samples_allocated.back().unwrap_or(&0f64)
    }

    pub fn total(&self) -> f64 {
        self.total_memory
    }

    pub fn to_string(&self, vertical_panel: bool) -> String {
        let mut current_val = self.latest_sample();
        let unit: &str;

        if self.config.percentage {
            current_val = (current_val * 100.0) / self.total_memory;
            unit = "%";
        } else if !vertical_panel {
            unit = " GB";
        } else {
            unit = "GB";
        }

        if current_val < 10.0 {
            format!("{:.2}{unit}", (current_val * 100.0).trunc() / 100.0)
        } else if current_val < 100.0 {
            format!("{:.1}{unit}", (current_val * 10.0).trunc() / 10.0)
        } else {
            format!("{}{unit}", current_val.round())
        }
    }
}

const DEMO_SAMPLES: [f64; 21] = [
    0.00, 12.69, 12.64, 12.62, 12.66, 12.65, 12.63, 12.62, 12.61, 12.62, 19.06, 21.69, 21.81,
    21.29, 22.04, 21.76, 21.89, 15.26, 14.77, 14.50, 13.89,
];

const DEMO_SAMPLES_ALLOCATED: [f64; 21] = [
    15.27, 27.33, 27.29, 27.26, 27.29, 27.25, 27.26, 27.21, 27.20, 27.18, 29.90, 31.67, 31.72,
    31.20, 31.99, 31.69, 31.77, 26.15, 25.65, 25.42, 24.85,
];
