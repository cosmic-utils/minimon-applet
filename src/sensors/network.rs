use std::collections::VecDeque;

use sysinfo::Networks;

use crate::{
    colorpicker::DemoGraph,
    config::{ColorVariant, DeviceKind, GraphColors, GraphKind},
    fl,
    svg_graph::SvgColors,
};
use lazy_static::lazy_static;

use super::Sensor;

const MAX_SAMPLES: usize = 30;
const GRAPH_SAMPLES: usize = 21;
const UNITS_SHORT: [&str; 5] = ["b", "K", "M", "G", "T"];
const UNITS_LONG: [&str; 5] = ["bps", "Kbps", "Mbps", "Gbps", "Tbps"];

lazy_static! {
    /// Translated color choices.
    ///
    /// The string values are intentionally leaked (`.leak()`) to convert them
    /// into `'static str` because:
    /// - These strings are only initialized once at program startup.
    /// - They are never deallocated since they are used globally.
    static ref COLOR_CHOICES: [(&'static str, ColorVariant); 4] = [
        (fl!("graph-network-download").leak(), ColorVariant::Color2),
        (fl!("graph-network-upload").leak(), ColorVariant::Color3),
        (fl!("graph-network-back").leak(), ColorVariant::Color1),
        (fl!("graph-network-frame").leak(), ColorVariant::Color4),
    ];
}

#[derive(Debug, PartialEq, Eq)]
pub enum UnitVariant {
    Short,
    Long,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NetworkVariant {
    Download,
    Upload,
    Combined,
}

#[derive(Debug)]
pub struct Network {
    networks: Networks,
    download: VecDeque<u64>,
    upload: VecDeque<u64>,
    max_y: Option<u64>,
    colors: GraphColors,
    svg_colors: SvgColors,
    kind: NetworkVariant,
}

impl DemoGraph for Network {
    fn demo(&self) -> String {
        let download = VecDeque::from(DL_DEMO);
        let upload = VecDeque::from(UL_DEMO);

        crate::svg_graph::double_line(&download, &upload, GRAPH_SAMPLES, &self.svg_colors, None)
    }

    fn colors(&self) -> GraphColors {
        self.colors
    }

    fn set_colors(&mut self, colors: GraphColors) {
        self.colors = colors;
        self.svg_colors.set_colors(&colors);
    }

    fn color_choices(&self) -> Vec<(&'static str, ColorVariant)> {
        (*COLOR_CHOICES).into()
    }
}

impl Sensor for Network {
    fn kind(&self) -> GraphKind {
        GraphKind::Line
    }

    fn set_kind(&mut self, kind: GraphKind) {
        assert!(kind == GraphKind::Line);
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

        if self.download.len() >= MAX_SAMPLES {
            self.download.pop_front();
        }
        self.download.push_back(dl);

        if self.upload.len() >= MAX_SAMPLES {
            self.upload.pop_front();
        }
        self.upload.push_back(ul);
    }

    fn demo_graph(&self, colors: GraphColors) -> Box<dyn DemoGraph> {
        let mut dmo = Network::new(self.kind);
        dmo.set_colors(colors);
        Box::new(dmo)
    }

    fn graph(&self) -> String {
        crate::svg_graph::double_line(
            &self.download,
            &self.upload,
            GRAPH_SAMPLES,
            &self.svg_colors,
            self.max_y,
        )
    }
}

impl Network {
    pub fn new(kind: NetworkVariant) -> Self {
        let networks = Networks::new_with_refreshed_list();
        let colors = GraphColors::new(DeviceKind::Network(kind));
        Network {
            networks,
            download: VecDeque::from(vec![0; MAX_SAMPLES]),
            upload: VecDeque::from(vec![0; MAX_SAMPLES]),
            max_y: None,
            colors,
            kind,
            svg_colors: SvgColors::new(&colors),
        }
    }

    pub fn set_max_y(&mut self, max: Option<u64>) {
        self.max_y = max;
    }

    fn makestr(val: u64, format: UnitVariant) -> String {
        let mut value = val as f64;
        let mut unit_index = 0;
        let units = if format == UnitVariant::Short {
            UNITS_SHORT
        } else {
            UNITS_LONG
        };

        // Find the appropriate unit
        while value >= 999.0 && unit_index < units.len() - 1 {
            value /= 1024.0;
            unit_index += 1;
        }

        if value < 10.0 {
            format!("{:.2} {}", value, units[unit_index])
        } else if value < 99.0 {
            format!("{:.1} {}", value, units[unit_index])
        } else {
            format!("{:.0} {}", value, units[unit_index])
        }
    }

    // If the sample rate doesn't match exactly one second (more or less),
    // we grab enough samples to cover it and average the value of samples cover a longer duration.
    fn last_second_bitrate(samples: &VecDeque<u64>, sample_interval_ms: u64) -> u64 {
        let mut total_duration = 0u64;
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
        let scale = 1000.0 / total_duration as f64;

        (total_bitrate as f64 * scale).floor() as u64
    }

    // Get bits per second
    pub fn download_label(&self, sample_interval_ms: u64, format: UnitVariant) -> String {
        let rate = Network::last_second_bitrate(&self.download, sample_interval_ms);
        Network::makestr(rate, format)
    }

    // Get bits per second
    pub fn upload_label(&self, sample_interval_ms: u64, format: UnitVariant) -> String {
        let rate = Network::last_second_bitrate(&self.upload, sample_interval_ms);
        Network::makestr(rate, format)
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
