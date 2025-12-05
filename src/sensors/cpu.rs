use crate::{
    barchart::StackedBarSvg,
    colorpicker::DemoGraph,
    config::{ChartColors, ChartKind, ColorVariant, CpuConfig, DeviceKind},
    fl,
    sensors::INVALID_IMG,
    svg_graph::SvgColors,
};
use bounded_vec_deque::BoundedVecDeque;
use cosmic::{
    Element, Renderer, Theme, iced::Alignment::Center, iced_widget::Column, widget::Container,
};
use std::{any::Any, sync::LazyLock};

use cosmic::widget;
use cosmic::widget::{settings, toggler};

use cosmic::{
    iced::{
        Alignment,
        widget::{column, row},
    },
    iced_widget::Row,
};

use crate::app::Message;

use std::{
    collections::HashMap,
    fmt::Write,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use super::Sensor;

const MAX_SAMPLES: usize = 21;

pub static COLOR_CHOICES_RING: LazyLock<[(&'static str, ColorVariant); 4]> = LazyLock::new(|| {
    [
        (fl!("graph-cpu-load").leak(), ColorVariant::Graph1),
        (fl!("graph-cpu-idle").leak(), ColorVariant::Graph2),
        (fl!("graph-ring-back").leak(), ColorVariant::Background),
        (fl!("graph-ring-text").leak(), ColorVariant::Text),
    ]
});

pub static COLOR_CHOICES_LINE: LazyLock<[(&'static str, ColorVariant); 3]> = LazyLock::new(|| {
    [
        (fl!("graph-cpu-load").leak(), ColorVariant::Graph1),
        (fl!("graph-line-back").leak(), ColorVariant::Background),
        (fl!("graph-line-frame").leak(), ColorVariant::Frame),
    ]
});

static GRAPH_OPTIONS_RING_LINE_BARS: LazyLock<[&'static str; 3]> = LazyLock::new(|| {
    [
        fl!("graph-type-ring").leak(),
        fl!("graph-type-line").leak(),
        fl!("graph-type-bars").leak(),
    ]
});

pub static COLOR_CHOICES_BARS: LazyLock<[(&'static str, ColorVariant); 4]> = LazyLock::new(|| {
    [
        (fl!("graph-bars-system").leak(), ColorVariant::Graph2),
        (fl!("graph-bars-user").leak(), ColorVariant::Graph1),
        (fl!("graph-line-back").leak(), ColorVariant::Background),
        (fl!("graph-line-frame").leak(), ColorVariant::Frame),
    ]
});

#[derive(Debug, Clone, Copy, Default)]
struct CpuStat {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CpuLoad {
    pub user_pct: f64,
    pub system_pct: f64,
}

#[derive(Debug)]
pub struct Cpu {
    // Total CPU load since last update split into user and system
    total_cpu_load: CpuLoad,
    // Load per core since last update split into user and system
    core_loads: HashMap<usize, CpuLoad>,
    // Current Load per core since /proc
    current_core_stats: HashMap<usize, CpuStat>,
    // Load per core in last update
    prev_core_stats: HashMap<usize, CpuStat>,
    // Total CPU load for the last MAX_SAMPLES updates
    samples_sum: BoundedVecDeque<f64>,
    // CPU load for the last MAX_SAMPLES updates, split into user and system
    samples_split: BoundedVecDeque<CpuLoad>,
    graph_options: Vec<&'static str>,
    /// colors cached so we don't need to convert to string every time
    svg_colors: SvgColors,
    config: CpuConfig,
}

impl DemoGraph for Cpu {
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
            ChartKind::Heat => {
                log::error!("Wrong graph choice!");
                INVALID_IMG.to_string()
            }
            ChartKind::StackedBars => {
                let mut map = HashMap::new();
                map.insert(
                    0,
                    CpuLoad {
                        user_pct: 15.5,
                        system_pct: 8.2,
                    },
                );
                map.insert(
                    1,
                    CpuLoad {
                        user_pct: 42.1,
                        system_pct: 12.7,
                    },
                );
                map.insert(
                    2,
                    CpuLoad {
                        user_pct: 78.9,
                        system_pct: 18.3,
                    },
                );
                map.insert(
                    3,
                    CpuLoad {
                        user_pct: 25.6,
                        system_pct: 5.4,
                    },
                );
                StackedBarSvg::default().svg(&map, &self.svg_colors)
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
            ChartKind::Line => (*COLOR_CHOICES_LINE).into(),
            ChartKind::Ring => (*COLOR_CHOICES_RING).into(),
            ChartKind::StackedBars => (*COLOR_CHOICES_BARS).into(),
            _ => panic!(
                "CPU color_choices {:?} wrong chart type!",
                self.config.chart
            ),
        }
    }

    fn id(&self) -> Option<String> {
        None
    }

    fn kind(&self) -> ChartKind {
        self.config.chart
    }
}

impl Sensor for Cpu {
    fn update_config(&mut self, config: &dyn Any, _refresh_rate: u32) {
        if let Some(cfg) = config.downcast_ref::<CpuConfig>() {
            self.config = cfg.clone();
            self.svg_colors.set_colors(cfg.colors());
        }
    }

    fn graph_kind(&self) -> ChartKind {
        self.config.chart
    }

    fn set_graph_kind(&mut self, kind: ChartKind) {
        assert!(
            kind == ChartKind::Line || kind == ChartKind::Ring || kind == ChartKind::StackedBars
        );
        self.config.chart = kind;
    }

    fn update(&mut self) {
        self.update_stats();
        self.samples_split.push_back(self.total_cpu_load);
        self.samples_sum
            .push_back(self.total_cpu_load.user_pct + self.total_cpu_load.system_pct);
    }

    fn demo_graph(&self) -> Box<dyn DemoGraph> {
        let mut dmo = Cpu::new(true);
        dmo.update_config(&self.config, 0);
        Box::new(dmo)
    }

    #[cfg(feature = "lyon_charts")]
    fn chart<'a>(&self) -> cosmic::widget::Container<crate::app::Message, Theme, Renderer> {
        if self.config.kind == ChartKind::Ring {
            let latest = self.latest_sample();
            let mut value = String::with_capacity(10);

            if self.config.no_decimals {
                write!(value, "{}%", latest.round()).unwrap();
            } else if latest < 10.0 {
                write!(value, "{latest:.2}").unwrap()
            } else if latest <= 99.9 {
                write!(value, "{latest:.1}").unwrap();
            } else {
                write!(value, "100").unwrap();
            }
            chart_container!(crate::charts::ring::RingChart::new(
                latest as f32,
                &value,
                &self.config.colors,
            ))
        } else {
            chart_container!(crate::charts::line::LineChart::new(
                MAX_SAMPLES,
                &self.samples_sum,
                &VecDeque::new(),
                Some(100.0),
                &self.config.colors,
            ))
        }
    }

    #[cfg(not(feature = "lyon_charts"))]
    fn chart(
        &'_ self,
        height_hint: u16,
        _width_hint: u16,
    ) -> cosmic::widget::Container<'_, crate::app::Message, Theme, Renderer> {
        let svg = match self.config.chart {
            ChartKind::Ring => {
                let latest = self.latest_sample();
                let mut value = String::with_capacity(10);

                if self.config.no_decimals {
                    let _ = write!(value, "{}%", latest.round());
                } else if latest < 10.0 {
                    let _ = write!(value, "{latest:.2}");
                } else if latest <= 99.9 {
                    let _ = write!(value, "{latest:.1}");
                } else {
                    let _ = write!(value, "100");
                }

                let percentage: u8 = latest.round().clamp(0.0, 100.0) as u8;

                crate::svg_graph::ring(&value, percentage, None, &self.svg_colors)
            }
            ChartKind::Line => crate::svg_graph::line(&self.samples_sum, 100.0, &self.svg_colors),
            ChartKind::StackedBars => {
                StackedBarSvg::new(self.config.bar_width, height_hint, self.config.bar_spacing)
                    .svg(&self.core_loads, &self.svg_colors)
            }
            ChartKind::Heat => {
                log::error!("Heat not supported!");
                INVALID_IMG.to_string()
            }
        };
        super::svg_icon_container::<Message>(svg)
    }

    fn settings_ui(&'_ self) -> Element<'_, crate::app::Message> {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();

        let mut cpu_elements = Vec::new();
        let mut cpu_column = Vec::new();

        if self.graph_kind() != ChartKind::StackedBars {
            let cpu = self.to_string();
            cpu_elements.push(Element::from(
                column!(
                    Container::new(self.chart(60, 60).width(60).height(60))
                        .width(90)
                        .align_x(Alignment::Center),
                    cosmic::widget::text::body(cpu.to_string())
                        .width(90)
                        .align_x(Alignment::Center)
                )
                .padding(5)
                .align_x(Alignment::Center),
            ));
        } else {
            let width = StackedBarSvg::new(self.config.bar_width, 60, self.config.bar_spacing)
                .width(self.core_count());
            cpu_column.push(Element::from(row!(
                widget::horizontal_space(),
                self.chart(60, width).height(60).width(width),
                widget::horizontal_space()
            )));
        };

        // A bit ugly and error prone, the Heat type is not supported here so bars takes its place
        // in numbering for the dropdown
        let selected: Option<usize> = if self.graph_kind() == ChartKind::StackedBars {
            Some(2)
        } else {
            Some(self.graph_kind().into())
        };

        let config = &self.config;
        let cpu_kind = self.graph_kind();

        cpu_column.push(
            settings::item(
                fl!("enable-chart"),
                toggler(config.chart_visible()).on_toggle(Message::ToggleCpuChart),
            )
            .into(),
        );

        if self.graph_kind() == ChartKind::StackedBars {
            cpu_column.push(
                settings::item(
                    fl!("graph-bar-width"),
                    widget::spin_button(
                        self.config.bar_width.to_string(),
                        self.config.bar_width,
                        1,
                        1,
                        16,
                        Message::CpuBarSizeChanged,
                    ),
                )
                .into(),
            );

            let narrow = config.bar_spacing == 0;
            cpu_column.push(
                settings::item(
                    fl!("graph-bar-spacing"),
                    toggler(narrow).on_toggle(Message::CpuNarrowBarSpacing),
                )
                .into(),
            );
        }

        cpu_column.push(
            settings::item(
                fl!("enable-label"),
                toggler(config.label_visible()).on_toggle(Message::ToggleCpuLabel),
            )
            .into(),
        );
        if self.config.label_visible() {
            cpu_column.push(
                settings::item(
                    fl!("cpu-no-decimals"),
                    row!(
                        widget::checkbox("", config.no_decimals)
                            .on_toggle(Message::ToggleCpuNoDecimals)
                    ),
                )
                .into(),
            );
        }
        cpu_column.push(
            row!(
                widget::text::body(fl!("chart-type")),
                widget::dropdown(&self.graph_options, selected, move |m| {
                    let mut choice: ChartKind = m.into();
                    if choice != ChartKind::Ring && choice != ChartKind::Line {
                        choice = ChartKind::StackedBars
                    };
                    Message::SelectGraphType(DeviceKind::Cpu, choice)
                })
                .width(70),
                widget::horizontal_space(),
                widget::button::standard(fl!("change-colors")).on_press(Message::ColorPickerOpen(
                    DeviceKind::Cpu,
                    cpu_kind,
                    None
                )),
            )
            .align_y(Center)
            .into(),
        );

        cpu_elements.push(Element::from(
            Column::with_children(cpu_column).spacing(cosmic.space_xs()),
        ));

        Row::with_children(cpu_elements)
            .align_y(Alignment::Center)
            .spacing(0)
            .into()
    }
}

impl Cpu {
    pub fn new(is_horizontal: bool) -> Self {
        // value and percentage are pre-allocated and reused as they're changed often.
        let mut percentage = String::with_capacity(6);
        percentage.push('0');

        let mut value = String::with_capacity(6);
        value.push('0');

        let graph_opts: Vec<&'static str> = if is_horizontal {
            (*GRAPH_OPTIONS_RING_LINE_BARS).into()
        } else {
            (*super::GRAPH_OPTIONS_RING_LINE).into()
        };

        // Initialize CPU/Core structures
        let mut core_stats: HashMap<usize, CpuStat> = HashMap::new();
        Self::read_cpu_stats(&mut core_stats);
        log::info!("Found CPU Cores: {}", core_stats.len());

        let core_loads: HashMap<usize, CpuLoad> = core_stats
            .keys()
            .map(|&k| (k, CpuLoad::default()))
            .collect();

        let mut cpu = Cpu {
            total_cpu_load: CpuLoad {
                user_pct: 0.,
                system_pct: 0.,
            },
            core_loads,
            current_core_stats: core_stats.clone(),
            prev_core_stats: core_stats,
            samples_sum: BoundedVecDeque::from_iter(
                std::iter::repeat_n(0.0, MAX_SAMPLES),
                MAX_SAMPLES,
            ),
            samples_split: BoundedVecDeque::from_iter(
                std::iter::repeat_n(
                    CpuLoad {
                        user_pct: 0.,
                        system_pct: 0.,
                    },
                    MAX_SAMPLES,
                ),
                MAX_SAMPLES,
            ),
            graph_options: graph_opts.to_vec(),
            svg_colors: SvgColors::new(&ChartColors::default()),
            config: CpuConfig::default(),
        };
        cpu.set_colors(&ChartColors::default());
        cpu
    }

    pub fn latest_sample(&self) -> f64 {
        *self.samples_sum.back().unwrap_or(&0f64)
    }

    pub fn core_count(&self) -> usize {
        self.core_loads.len()
    }

    fn read_cpu_stats(cpu_stats: &mut HashMap<usize, CpuStat>) {
        // Open /proc/stat file
        let Ok(file) = File::open(Path::new("/proc/stat")) else {
            return;
        };

        let reader = BufReader::new(file);
        cpu_stats.clear();

        // Read each line from the file
        for line in reader.lines() {
            let Ok(line) = line else { continue };
            // Split line into parts
            let parts: Vec<&str> = line.split_whitespace().collect();

            // Check if line starts with 'cpu' followed by a number
            if parts.is_empty() || !parts[0].starts_with("cpu") || parts[0] == "cpu" {
                continue;
            }

            // Extract CPU number
            let Ok(core_num) = parts[0].trim_start_matches("cpu").parse::<usize>() else {
                continue;
            };

            // Ensure we have enough parts for all fields
            if parts.len() < 9 {
                continue;
            }

            // Parse all CPU time values
            let user = parts[1].parse::<u64>().unwrap_or(0);
            let nice = parts[2].parse::<u64>().unwrap_or(0);
            let system = parts[3].parse::<u64>().unwrap_or(0);
            let idle = parts[4].parse::<u64>().unwrap_or(0);
            let iowait = parts[5].parse::<u64>().unwrap_or(0);
            let irq = parts[6].parse::<u64>().unwrap_or(0);
            let softirq = parts[7].parse::<u64>().unwrap_or(0);
            let steal = parts[8].parse::<u64>().unwrap_or(0);

            // Create CpuStat struct and insert into HashMap
            let core_stats = CpuStat {
                user,
                nice,
                system,
                idle,
                iowait,
                irq,
                softirq,
                steal,
            };

            cpu_stats.insert(core_num, core_stats);
        }
    }

    // Update current CPU load by comparing to previous samples
    fn update_stats(&mut self) {
        // Read current CPU stats
        self.current_core_stats.clear();
        Cpu::read_cpu_stats(&mut self.current_core_stats);

        // Running totals for average computation
        let mut total_user_pct = 0.0;
        let mut total_system_pct = 0.0;
        let mut counted_cores = 0;

        self.core_loads.clear();

        for (&core_num, current) in &self.current_core_stats {
            if let Some(prev) = self.prev_core_stats.get_mut(&core_num) {
                // Compute time deltas
                let user = current.user.saturating_sub(prev.user);
                let nice = current.nice.saturating_sub(prev.nice);
                let system = current.system.saturating_sub(prev.system);
                let idle = current.idle.saturating_sub(prev.idle);
                let iowait = current.iowait.saturating_sub(prev.iowait);
                let irq = current.irq.saturating_sub(prev.irq);
                let softirq = current.softirq.saturating_sub(prev.softirq);
                let steal = current.steal.saturating_sub(prev.steal);

                let total = user + nice + system + idle + iowait + irq + softirq + steal;
                if total == 0 {
                    continue;
                }

                let total_f64 = total as f64;
                let user_pct = (user + nice) as f64 / total_f64 * 100.0;
                let system_pct = system as f64 / total_f64 * 100.0;

                self.core_loads.insert(
                    core_num,
                    CpuLoad {
                        user_pct,
                        system_pct,
                    },
                );

                total_user_pct += user_pct;
                total_system_pct += system_pct;
                counted_cores += 1;

                *prev = *current;
            }
        }

        if counted_cores > 0 {
            let core_count_f64 = f64::from(counted_cores);
            self.total_cpu_load = CpuLoad {
                user_pct: total_user_pct / core_count_f64,
                system_pct: total_system_pct / core_count_f64,
            };
        }
    }
}

use std::fmt;

impl fmt::Display for Cpu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let current_val = self.latest_sample();

        if self.config.no_decimals {
            write!(f, "{}%", current_val.round())
        } else if current_val < 10.0 {
            write!(f, "{current_val:.2}%")
        } else if current_val < 100.0 {
            write!(f, "{current_val:.1}%")
        } else {
            write!(f, "{current_val}%")
        }
    }
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
