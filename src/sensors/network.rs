use bounded_vec_deque::BoundedVecDeque;

use cosmic::{Element, iced_widget::Column, widget::Container};
use log::info;
use sysinfo::Networks;

use crate::{
    colorpicker::DemoGraph,
    config::{ChartColors, ChartKind, ColorVariant, DeviceKind, NetworkConfig, NetworkVariant},
    fl,
    svg_graph::SvgColors,
};

use cosmic::widget;
use cosmic::widget::settings;

use crate::app::Message;
use cosmic::{
    iced::{
        Alignment,
        widget::{column, row},
    },
    iced_widget::Row,
};
use std::any::Any;

use super::Sensor;

const MAX_SAMPLES: usize = 30;
const GRAPH_SAMPLES: usize = 21;
const UNITS_SHORT: [&str; 5] = ["b", "K", "M", "G", "T"];
const UNITS_LONG: [&str; 5] = ["bps", "Kbps", "Mbps", "Gbps", "Tbps"];
const UNITS_SHORT_BYTES: [&str; 5] = ["B", "K", "M", "G", "T"];
const UNITS_LONG_BYTES: [&str; 5] = ["B/s", "KB/s", "MB/s", "GB/s", "TB/s"];

use std::sync::LazyLock;

pub static COLOR_CHOICES_COMBINED: LazyLock<[(&'static str, ColorVariant); 4]> =
    LazyLock::new(|| {
        [
            (fl!("graph-network-download").leak(), ColorVariant::Graph1),
            (fl!("graph-network-upload").leak(), ColorVariant::Graph2),
            (fl!("graph-network-back").leak(), ColorVariant::Background),
            (fl!("graph-network-frame").leak(), ColorVariant::Frame),
        ]
    });

pub static COLOR_CHOICES_DL: LazyLock<[(&'static str, ColorVariant); 3]> = LazyLock::new(|| {
    [
        (fl!("graph-network-download").leak(), ColorVariant::Graph1),
        (fl!("graph-network-back").leak(), ColorVariant::Background),
        (fl!("graph-network-frame").leak(), ColorVariant::Frame),
    ]
});

pub static COLOR_CHOICES_UL: LazyLock<[(&'static str, ColorVariant); 3]> = LazyLock::new(|| {
    [
        (fl!("graph-network-upload").leak(), ColorVariant::Graph2),
        (fl!("graph-network-back").leak(), ColorVariant::Background),
        (fl!("graph-network-frame").leak(), ColorVariant::Frame),
    ]
});

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UnitVariant {
    Short,
    Long,
}

#[derive(Debug)]
pub struct Network {
    networks: Networks,
    download: BoundedVecDeque<u64>,
    upload: BoundedVecDeque<u64>,
    max_y: Option<u64>,
    svg_colors: SvgColors,
    dropdown_options: Vec<&'static str>,
    config: NetworkConfig,
    refresh_rate: u32,
}

impl DemoGraph for Network {
    fn demo(&self) -> String {
        let download = std::collections::VecDeque::from(DL_DEMO);
        let upload = std::collections::VecDeque::from(UL_DEMO);

        match self.config.variant {
            NetworkVariant::Combined => crate::svg_graph::double_line(
                &download,
                &upload,
                GRAPH_SAMPLES,
                &self.svg_colors,
                None,
            ),
            NetworkVariant::Download => {
                crate::svg_graph::line_adaptive(&download, GRAPH_SAMPLES, &self.svg_colors, None)
            }
            NetworkVariant::Upload => {
                let mut cols = self.svg_colors.clone();
                cols.graph1 = cols.graph2.clone();
                crate::svg_graph::line_adaptive(&upload, GRAPH_SAMPLES, &cols, None)
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
            NetworkVariant::Combined => (*COLOR_CHOICES_COMBINED).into(),
            NetworkVariant::Download => (*COLOR_CHOICES_DL).into(),
            NetworkVariant::Upload => (*COLOR_CHOICES_UL).into(),
        }
    }

    fn id(&self) -> Option<String> {
        None
    }

    fn kind(&self) -> ChartKind {
        self.config.chart
    }
}

impl Sensor for Network {
    fn update_config(&mut self, config: &dyn Any, refresh_rate: u32) {
        if let Some(cfg) = config.downcast_ref::<NetworkConfig>() {
            self.config = cfg.clone();
            self.svg_colors.set_colors(cfg.colors());
            self.refresh_rate = refresh_rate;

            if self.config.show_bytes {
                self.dropdown_options = ["b", "Kb", "Mb", "Gb", "Tb"].into();
            } else {
                self.dropdown_options = ["B", "KB", "MB", "GB", "TB"].into();
            };

            if cfg.adaptive {
                self.max_y = None;
            } else {
                let unit = cfg.unit.unwrap_or(1).min(4); // ensure safe index
                let multiplier = [1, 1_000, 1_000_000, 1_000_000_000, 1_000_000_000_000];
                let sec_per_tic = refresh_rate as f64 / 1000.0;
                let new_y = (cfg.bandwidth * multiplier[unit]) as f64 * sec_per_tic;
                self.max_y = Some(new_y.round() as u64);
            }
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
        self.networks.refresh(true);
        let mut dl = 0;
        let mut ul = 0;

        for (_, network) in &self.networks {
            dl += network.received() * 8;
            ul += network.transmitted() * 8;
        }
        self.download.push_back(dl);
        self.upload.push_back(ul);
    }

    fn demo_graph(&self) -> Box<dyn DemoGraph> {
        let mut dmo = Network::default();
        dmo.update_config(&self.config, self.refresh_rate);
        Box::new(dmo)
    }

    #[cfg(feature = "lyon_charts")]
    fn chart(
        &self,
    ) -> cosmic::widget::Container<crate::app::Message, cosmic::Theme, cosmic::Renderer> {
        let mut colors = self.config.colors;
        match self.config.variant {
            NetworkVariant::Combined => {
                //A bit awkward, but to maintain compatibility with the SVG charts
                colors.color4 = self.config.colors.color2;
                colors.color2 = self.config.colors.color4;
                chart_container!(crate::charts::line::LineChart::new(
                    GRAPH_SAMPLES,
                    &self.download,
                    &self.upload,
                    self.max_y,
                    &colors,
                ))
            }
            NetworkVariant::Download => {
                //A bit awkward, but to maintain compatibility with the SVG charts
                colors.color4 = self.config.colors.color2;
                colors.color2 = self.config.colors.color4;
                chart_container!(crate::charts::line::LineChart::new(
                    GRAPH_SAMPLES,
                    &self.download,
                    &VecDeque::new(),
                    self.max_y,
                    &colors,
                ))
            }
            NetworkVariant::Upload => {
                //A bit awkward, but to maintain compatibility with the SVG charts
                colors.color4 = self.config.colors.color3;
                colors.color2 = self.config.colors.color4;
                chart_container!(crate::charts::line::LineChart::new(
                    GRAPH_SAMPLES,
                    &self.upload,
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
            NetworkVariant::Combined => crate::svg_graph::double_line(
                &self.download,
                &self.upload,
                GRAPH_SAMPLES,
                &self.svg_colors,
                self.max_y,
            ),
            NetworkVariant::Download => crate::svg_graph::line_adaptive(
                &self.download,
                GRAPH_SAMPLES,
                &self.svg_colors,
                self.max_y,
            ),
            NetworkVariant::Upload => {
                let mut cols = self.svg_colors.clone();
                cols.graph1 = cols.graph2.clone();
                crate::svg_graph::line_adaptive(&self.upload, GRAPH_SAMPLES, &cols, self.max_y)
            }
        };
        super::svg_icon_container::<Message>(svg)
    }

    fn settings_ui(&'_ self) -> Element<'_, crate::app::Message> {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();
        let mut net_elements = Vec::new();

        let sample_rate_ms = self.refresh_rate;

        let dlrate = format!(
            "↓ {}",
            &self.download_label(sample_rate_ms, UnitVariant::Long)
        );

        let ulrate = format!(
            "↑ {}",
            &self.upload_label(sample_rate_ms, UnitVariant::Long)
        );

        let config = &self.config;
        let k = self.config.variant;

        let mut rate = column!(
            Container::new(self.chart(60, 60).width(60).height(60))
                .width(90)
                .align_x(Alignment::Center)
        );

        rate = rate.push(Element::from(cosmic::widget::text::body("")));

        match self.config.variant {
            NetworkVariant::Combined => {
                rate = rate.push(
                    cosmic::widget::text::body(dlrate)
                        .width(90)
                        .align_x(Alignment::Center),
                );
                rate = rate.push(
                    cosmic::widget::text::body(ulrate)
                        .width(90)
                        .align_x(Alignment::Center),
                );
            }
            NetworkVariant::Download => {
                rate = rate.push(
                    cosmic::widget::text::body(dlrate)
                        .width(90)
                        .align_x(Alignment::Center),
                );
            }
            NetworkVariant::Upload => {
                rate = rate.push(
                    cosmic::widget::text::body(ulrate)
                        .width(90)
                        .align_x(Alignment::Center),
                );
            }
        }
        net_elements.push(Element::from(rate));

        let mut net_bandwidth_items = Vec::new();

        net_bandwidth_items.push(
            settings::item(
                fl!("enable-chart"),
                widget::toggler(config.chart_visible())
                    .on_toggle(move |t| Message::ToggleNetChart(k, t)),
            )
            .into(),
        );
        net_bandwidth_items.push(
            settings::item(
                fl!("enable-label"),
                widget::toggler(config.label_visible())
                    .on_toggle(move |t| Message::ToggleNetLabel(k, t)),
            )
            .into(),
        );
        net_bandwidth_items.push(
            settings::item(
                fl!("use-adaptive"),
                row!(
                    widget::checkbox("", config.adaptive)
                        .on_toggle(move |t| Message::ToggleAdaptiveNet(k, t))
                ),
            )
            .into(),
        );

        if !config.adaptive {
            net_bandwidth_items.push(
                settings::item(
                    fl!("net-bandwidth"),
                    row!(
                        widget::text_input("", config.bandwidth.to_string())
                            .width(100)
                            .on_input(move |b| Message::TextInputBandwidthChanged(k, b)),
                        widget::dropdown(&self.dropdown_options, config.unit, move |u| {
                            Message::NetworkSelectUnit(k, u)
                        },)
                        .width(50)
                    ),
                )
                .into(),
            );
        }

        net_bandwidth_items.push(
            row!(
                widget::horizontal_space(),
                widget::button::standard(fl!("change-colors")).on_press(Message::ColorPickerOpen(
                    DeviceKind::Network(self.config.variant),
                    ChartKind::Line,
                    None
                )),
                widget::horizontal_space()
            )
            .into(),
        );

        let net_right_column = Column::with_children(net_bandwidth_items);

        net_elements.push(Element::from(net_right_column.spacing(cosmic.space_xs())));

        let title_content = match (config.show_bytes, self.config.variant) {
            (true, NetworkVariant::Combined) => fl!("net-title-combined-bytes"),
            (true, NetworkVariant::Download) => fl!("net-title-dl-bytes"),
            (true, NetworkVariant::Upload) => fl!("net-title-ul-bytes"),
            (false, NetworkVariant::Combined) => fl!("net-title-combined"),
            (false, NetworkVariant::Download) => fl!("net-title-dl"),
            (false, NetworkVariant::Upload) => fl!("net-title-ul"),
        };

        let title = widget::text::heading(title_content);

        column![
            title,
            Row::with_children(net_elements).align_y(Alignment::Center)
        ]
        .spacing(cosmic::theme::spacing().space_xs)
        .into()
    }
}

impl Default for Network {
    fn default() -> Self {
        let networks = Networks::new_with_refreshed_list();
        Network {
            networks,
            download: BoundedVecDeque::from_iter(std::iter::repeat_n(0, MAX_SAMPLES), MAX_SAMPLES),
            upload: BoundedVecDeque::from_iter(std::iter::repeat_n(0, MAX_SAMPLES), MAX_SAMPLES),
            max_y: None,
            dropdown_options: ["b", "Kb", "Mb", "Gb", "Tb"].into(),
            svg_colors: SvgColors::new(&ChartColors::default()),
            config: NetworkConfig::default(),
            refresh_rate: 1000,
        }
    }
}

impl Network {
    fn makestr(val: u64, format: UnitVariant, show_bytes: bool) -> String {
        let mut value = val as f64;

        if show_bytes {
            value /= 8.0;
        }

        let mut unit_index = 0;

        let units = match (show_bytes, format) {
            (false, UnitVariant::Short) => UNITS_SHORT,
            (false, UnitVariant::Long) => UNITS_LONG,
            (true, UnitVariant::Short) => UNITS_SHORT_BYTES,
            (true, UnitVariant::Long) => UNITS_LONG_BYTES,
        };

        // Scale the value to the appropriate unit
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
            info!("Value: {value}. formatted: {value:.2}. string: {value_str}");
            value_str.pop();
        }

        let unit_str = units[unit_index];
        let mut result = String::with_capacity(20);
        result.push_str(&value_str);

        if format == UnitVariant::Long {
            result.push(' ');
        }

        result.push_str(unit_str);

        if format == UnitVariant::Long {
            let padding = 9usize.saturating_sub(result.len());
            if padding > 0 {
                result = " ".repeat(padding) + &result;
            }
        }

        result
    }

    // If the sample rate doesn't match exactly one second (more or less),
    // we grab enough samples to cover it and average the value of samples cover a longer duration.
    fn last_second_bitrate(samples: &BoundedVecDeque<u64>, sample_interval_ms: u32) -> u64 {
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

    // Get bits per second
    pub fn download_label(&self, sample_interval_ms: u32, format: UnitVariant) -> String {
        let rate = Network::last_second_bitrate(&self.download, sample_interval_ms);
        Network::makestr(rate, format, self.config.show_bytes)
    }

    // Get bits per second
    pub fn upload_label(&self, sample_interval_ms: u32, format: UnitVariant) -> String {
        let rate = Network::last_second_bitrate(&self.upload, sample_interval_ms);
        Network::makestr(rate, format, self.config.show_bytes)
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
