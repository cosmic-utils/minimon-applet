use bounded_vec_deque::BoundedVecDeque;

use sysinfo::{DiskRefreshKind, Disks as DisksInfo};

use crate::{
    colorpicker::DemoGraph,
    config::{ChartColors, ChartKind, ColorVariant, DeviceKind, DisksConfig},
    fl,
    svg_graph::SvgColors,
};

use cosmic::{
    Element,
    widget::{Column, Container},
};

use cosmic::widget;
use cosmic::widget::settings;

use cosmic::{
    iced::{
        Alignment,
        widget::{column, row},
    },
    iced_widget::Row,
};

use crate::app::Message;
use crate::config::DisksVariant;
use std::any::Any;

use super::Sensor;

const MAX_SAMPLES: usize = 30;
const GRAPH_SAMPLES: usize = 21;
const UNITS_SHORT: [&str; 5] = ["B", "K", "M", "G", "T"];
const UNITS_LONG: [&str; 5] = ["B/s", "KB/s", "MB/s", "GB/s", "TB/s"];
use std::sync::LazyLock;

pub static COLOR_CHOICES_COMBINED: LazyLock<[(&'static str, ColorVariant); 4]> =
    LazyLock::new(|| {
        [
            (fl!("graph-disks-write").leak(), ColorVariant::Graph1),
            (fl!("graph-disks-read").leak(), ColorVariant::Graph2),
            (fl!("graph-disks-back").leak(), ColorVariant::Background),
            (fl!("graph-disks-frame").leak(), ColorVariant::Frame),
        ]
    });

pub static COLOR_CHOICES_WRITE: LazyLock<[(&'static str, ColorVariant); 3]> = LazyLock::new(|| {
    [
        (fl!("graph-disks-write").leak(), ColorVariant::Graph1),
        (fl!("graph-disks-back").leak(), ColorVariant::Background),
        (fl!("graph-disks-frame").leak(), ColorVariant::Frame),
    ]
});

pub static COLOR_CHOICES_READ: LazyLock<[(&'static str, ColorVariant); 3]> = LazyLock::new(|| {
    [
        (fl!("graph-disks-read").leak(), ColorVariant::Graph2),
        (fl!("graph-disks-back").leak(), ColorVariant::Background),
        (fl!("graph-disks-frame").leak(), ColorVariant::Frame),
    ]
});

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UnitVariant {
    Short,
    Long,
}

#[derive(Debug)]
pub struct Disks {
    disks: DisksInfo,
    write: BoundedVecDeque<u64>,
    read: BoundedVecDeque<u64>,
    max_y: Option<u64>,
    svg_colors: SvgColors,
    config: DisksConfig,
    refresh_rate: u32,
}

impl DemoGraph for Disks {
    fn demo(&self) -> String {
        let write = std::collections::VecDeque::from(DL_DEMO);
        let read = std::collections::VecDeque::from(UL_DEMO);

        match self.config.variant {
            DisksVariant::Combined => {
                crate::svg_graph::double_line(&write, &read, GRAPH_SAMPLES, &self.svg_colors, None)
            }
            DisksVariant::Write => {
                crate::svg_graph::line_adaptive(&write, GRAPH_SAMPLES, &self.svg_colors, None)
            }
            DisksVariant::Read => {
                let mut cols = self.svg_colors.clone();
                cols.graph1 = cols.graph2.clone();
                crate::svg_graph::line_adaptive(&read, GRAPH_SAMPLES, &cols, None)
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
        match self.config.variant {
            DisksVariant::Combined => (*COLOR_CHOICES_COMBINED).into(),
            DisksVariant::Write => (*COLOR_CHOICES_WRITE).into(),
            DisksVariant::Read => (*COLOR_CHOICES_READ).into(),
        }
    }

    fn id(&self) -> Option<String> {
        None
    }

    fn kind(&self) -> ChartKind {
        self.config.chart
    }
}

impl Sensor for Disks {
    fn update_config(&mut self, config: &dyn Any, refresh_rate: u32) {
        if let Some(cfg) = config.downcast_ref::<DisksConfig>() {
            self.config = cfg.clone();
            self.svg_colors.set_colors(cfg.colors());
            self.refresh_rate = refresh_rate;
        }
    }

    fn graph_kind(&self) -> ChartKind {
        ChartKind::Line
    }

    fn set_graph_kind(&mut self, kind: ChartKind) {
        assert!(kind == ChartKind::Line);
    }

    /// Retrieve the amount of data transmitted since last update.
    fn update(&mut self) {
        let r = DiskRefreshKind::nothing().with_io_usage();
        self.disks.refresh_specifics(true, r);
        let mut wr = 0;
        let mut rd = 0;

        for disk in self.disks.list() {
            let usage = disk.usage();
            wr += usage.written_bytes;
            rd += usage.read_bytes;
        }

        self.write.push_back(wr);
        self.read.push_back(rd);
    }

    fn demo_graph(&self) -> Box<dyn DemoGraph> {
        let mut dmo = Disks::default();
        dmo.update_config(&self.config, 0);
        Box::new(dmo)
    }

    #[cfg(feature = "lyon_charts")]
    fn chart(
        &self,
    ) -> cosmic::widget::Container<crate::app::Message, cosmic::Theme, cosmic::Renderer> {
        //A bit awkward, but to maintain compatibility with the SVG charts
        let mut colors = self.config.colors;
        match self.config.variant {
            DisksVariant::Combined => {
                colors.color4 = self.config.colors.color2;
                colors.color2 = self.config.colors.color4;
                chart_container!(crate::charts::line::LineChart::new(
                    GRAPH_SAMPLES,
                    &self.write,
                    &self.read,
                    self.max_y,
                    &colors,
                ))
            }
            DisksVariant::Write => {
                colors.color4 = self.config.colors.color2;
                colors.color2 = self.config.colors.color4;
                chart_container!(crate::charts::line::LineChart::new(
                    GRAPH_SAMPLES,
                    &self.write,
                    &VecDeque::new(),
                    self.max_y,
                    &colors,
                ))
            }
            DisksVariant::Read => {
                //A bit awkward, but to maintain compatibility with the SVG charts
                colors.color4 = self.config.colors.color3;
                colors.color2 = self.config.colors.color4;
                chart_container!(crate::charts::line::LineChart::new(
                    GRAPH_SAMPLES,
                    &self.read,
                    &VecDeque::new(),
                    self.max_y,
                    &colors,
                ))
            }
        }
    }

    #[cfg(not(feature = "lyon_charts"))]
    fn chart(
        &'_ self,
        _height_hint: u16,
        _width_hint: u16,
    ) -> cosmic::widget::Container<'_, crate::app::Message, cosmic::Theme, cosmic::Renderer> {
        let svg = match self.config.variant {
            DisksVariant::Combined => crate::svg_graph::double_line(
                &self.write,
                &self.read,
                GRAPH_SAMPLES,
                &self.svg_colors,
                self.max_y,
            ),
            DisksVariant::Write => crate::svg_graph::line_adaptive(
                &self.write,
                GRAPH_SAMPLES,
                &self.svg_colors,
                self.max_y,
            ),
            DisksVariant::Read => {
                let mut cols = self.svg_colors.clone();
                cols.graph1 = cols.graph2.clone();
                crate::svg_graph::line_adaptive(&self.read, GRAPH_SAMPLES, &cols, self.max_y)
            }
        };
        super::svg_icon_container::<Message>(svg)
    }

    fn settings_ui(&'_ self) -> Element<'_, crate::app::Message> {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();
        let mut disk_elements = Vec::new();

        let sample_rate_ms = self.refresh_rate;

        let wrrate = format!("W {}", &self.write_label(sample_rate_ms, UnitVariant::Long));

        let rdrate = format!("R {}", &self.read_label(sample_rate_ms, UnitVariant::Long));

        let config = &self.config;
        let k = self.config.variant;

        let mut rate = column!(
            Container::new(self.chart(60, 60).width(60).height(60))
                .width(90)
                .align_x(Alignment::Center)
        );

        rate = rate.push(Element::from(cosmic::widget::text::body("")));

        match self.config.variant {
            DisksVariant::Combined => {
                rate = rate.push(
                    cosmic::widget::text::body(wrrate)
                        .width(90)
                        .align_x(Alignment::Center),
                );
                rate = rate.push(
                    cosmic::widget::text::body(rdrate)
                        .width(90)
                        .align_x(Alignment::Center),
                );
            }
            DisksVariant::Write => {
                rate = rate.push(
                    cosmic::widget::text::body(wrrate)
                        .width(90)
                        .align_x(Alignment::Center),
                );
            }
            DisksVariant::Read => {
                rate = rate.push(
                    cosmic::widget::text::body(rdrate)
                        .width(90)
                        .align_x(Alignment::Center),
                );
            }
        }
        disk_elements.push(Element::from(rate));

        let mut disk_bandwidth_items = Vec::new();

        disk_bandwidth_items.push(
            settings::item(
                fl!("enable-chart"),
                widget::toggler(config.chart_visible())
                    .on_toggle(move |t| Message::ToggleDisksChart(k, t)),
            )
            .into(),
        );
        disk_bandwidth_items.push(
            settings::item(
                fl!("enable-label"),
                widget::toggler(config.label_visible())
                    .on_toggle(move |t| Message::ToggleDisksLabel(k, t)),
            )
            .into(),
        );

        disk_bandwidth_items.push(
            row!(
                widget::horizontal_space(),
                widget::button::standard(fl!("change-colors")).on_press(Message::ColorPickerOpen(
                    DeviceKind::Disks(self.config.variant),
                    ChartKind::Line,
                    None
                )),
                widget::horizontal_space()
            )
            .into(),
        );

        let disk_right_column = Column::with_children(disk_bandwidth_items);

        disk_elements.push(Element::from(disk_right_column.spacing(cosmic.space_xs())));

        let title_content = match self.config.variant {
            DisksVariant::Combined => fl!("disks-title-combined"),
            DisksVariant::Write => fl!("disks-title-write"),
            DisksVariant::Read => fl!("disks-title-read"),
        };
        let title = widget::text::heading(title_content);

        column![
            title,
            Row::with_children(disk_elements).align_y(Alignment::Center)
        ]
        .spacing(cosmic::theme::spacing().space_xs)
        .into()
    }
}

impl Default for Disks {
    fn default() -> Self {
        let disks = DisksInfo::new_with_refreshed_list();
        Disks {
            disks,
            write: BoundedVecDeque::from_iter(std::iter::repeat_n(0, MAX_SAMPLES), MAX_SAMPLES),
            read: BoundedVecDeque::from_iter(std::iter::repeat_n(0, MAX_SAMPLES), MAX_SAMPLES),
            max_y: None,
            svg_colors: SvgColors::new(&ChartColors::default()),
            config: DisksConfig::default(),
            refresh_rate: 1000,
        }
    }
}

impl Disks {
    fn makestr(val: u64, format: UnitVariant) -> String {
        let mut formatted = String::with_capacity(20);

        let mut value = val as f64;
        let mut unit_index = 0;
        let units = if format == UnitVariant::Short {
            UNITS_SHORT
        } else {
            UNITS_LONG
        };

        // Find the appropriate unit
        while value >= 999.0 && unit_index < units.len() - 1 {
            value /= 1000.0;
            unit_index += 1;
        }

        // Format the number with varying precision, prevent the formatter from rounding up
        let mut value_str = if value < 10.0 {
            format!("{:.2}", (value * 100.0).trunc() / 100.0)
        } else if value < 100.0 {
            format!("{:.1}", (value * 10.0).trunc() / 10.0)
        } else {
            format!("{:.0}", value.trunc())
        };

        // This happens when value is something like 9.9543456789908765453456 and it's rounded up to 10.
        if value_str.len() == 5 {
            log::info!("Value: {value}. formatted: {value:.2}. string: {value_str}");
            value_str.pop();
        }

        formatted.push_str(&value_str);

        if format == UnitVariant::Long {
            formatted.push(' ');
        }

        formatted.push_str(units[unit_index]);

        if format == UnitVariant::Long {
            let padding = 9usize.saturating_sub(formatted.len());
            if padding > 0 {
                formatted = " ".repeat(padding) + &formatted;
            }
        }

        formatted
    }

    // If the sample rate doesn't match exactly one second (more or less),
    // we grab enough samples to cover it and average the value of samples cover a longer duration.
    fn last_second_rate(samples: &BoundedVecDeque<u64>, sample_interval_ms: u32) -> u64 {
        let mut total_duration = 0u32;
        let mut total_bitrate = 0u64;

        // Iterate from newest to oldest
        for &bitrate in samples.iter().rev() {
            if total_duration >= 1000 {
                break;
            }

            total_bitrate += bitrate;
            total_duration += sample_interval_ms;
        }

        // Scale to exactly 1000ms
        let scale = 1000.0 / f64::from(total_duration);

        (total_bitrate as f64 * scale).floor() as u64
    }

    // Get bytes per second
    pub fn write_label(&self, sample_interval_ms: u32, format: UnitVariant) -> String {
        let val = Disks::last_second_rate(&self.write, sample_interval_ms);
        Disks::makestr(val, format)
    }

    // Get bytes per second
    pub fn read_label(&self, sample_interval_ms: u32, format: UnitVariant) -> String {
        let val = Disks::last_second_rate(&self.read, sample_interval_ms);
        Disks::makestr(val, format)
    }
}

const DL_DEMO: [u64; 21] = [
    208, 2071, 0, 1056588, 912575, 912875, 912975, 912600, 1397, 1173024, 1228, 6910, 2493,
    1102101, 380, 2287, 1109656, 1541, 3798, 1132822, 68479,
];
const UL_DEMO: [u64; 21] = [
    0, 1687, 0, 9417, 9161, 838, 6739, 1561, 212372, 312372, 412372, 512372, 512372, 512372,
    412372, 312372, 112372, 864, 0, 8587, 760,
];
