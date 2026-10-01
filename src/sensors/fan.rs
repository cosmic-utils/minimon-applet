use crate::{
    colorpicker::DemoGraph,
    config::{ChartColors, ChartKind, ColorVariant, DeviceKind, FanConfig},
    fl,
    sensors::INVALID_IMG,
    svg_graph::SvgColors,
};
use bounded_vec_deque::BoundedVecDeque;
use cosmic::Element;
use cosmic::widget;
use cosmic::widget::settings;

use log::info;

use crate::app::Message;
use crate::ui;
use std::any::Any;
use std::collections::VecDeque;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use super::Sensor;

const MAX_SAMPLES: usize = 21;

/// Lower bound for the chart scale, so a slow or stopped fan doesn't fill the chart.
const MIN_CHART_RPM: u32 = 1000;

static GRAPH_OPTIONS_LINE_HEAT: LazyLock<[&'static str; 2]> =
    LazyLock::new(|| [fl!("graph-type-line").leak(), fl!("graph-type-heat").leak()]);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FanInput {
    pub name: String,
    path: PathBuf,
}

/// Find every fan tachometer under `root`, whichever chip reports it.
pub fn find_fans(root: &Path) -> io::Result<Vec<FanInput>> {
    let mut fans = Vec::new();

    for entry in fs::read_dir(root)? {
        let hwmon = entry?.path();
        let chip = fs::read_to_string(hwmon.join("name")).unwrap_or_default();
        let chip = chip.trim();

        let Ok(files) = fs::read_dir(&hwmon) else {
            continue;
        };

        for file in files.flatten() {
            let file_name = file.file_name();
            let Some(index) = file_name
                .to_str()
                .and_then(|name| name.strip_prefix("fan"))
                .and_then(|name| name.strip_suffix("_input"))
            else {
                continue;
            };

            let name = match fs::read_to_string(hwmon.join(format!("fan{index}_label"))) {
                Ok(label) => label.trim().to_string(),
                Err(_) if chip.is_empty() => format!("fan{index}"),
                Err(_) => format!("{chip} fan{index}"),
            };
            info!("  found fan {} {name}", file.path().display());

            fans.push(FanInput {
                name,
                path: file.path(),
            });
        }
    }

    // read_dir order is arbitrary, keep the list stable between runs
    fans.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(fans)
}

fn read_rpm(path: &Path) -> io::Result<u32> {
    fs::read_to_string(path)?
        .trim()
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Parse error: {e}")))
}

// The chart type dropdown only lists Line and Heat, so its indexes differ from ChartKind's.
fn chart_index(kind: ChartKind) -> usize {
    usize::from(kind == ChartKind::Heat)
}

fn chart_kind(index: usize) -> ChartKind {
    if index == 1 {
        ChartKind::Heat
    } else {
        ChartKind::Line
    }
}

#[derive(Debug)]
pub struct Fan {
    fans: Vec<FanInput>,
    rpms: Vec<Option<u32>>,
    samples: BoundedVecDeque<f64>,
    max_rpm: u32,
    read_error_logged: bool,
    graph_options: Vec<&'static str>,
    /// colors cached so we don't need to convert to string every time
    svg_colors: SvgColors,
    config: FanConfig,
}

impl DemoGraph for Fan {
    fn demo(&self) -> String {
        match self.config.chart {
            ChartKind::Line => {
                crate::svg_graph::line(&VecDeque::from(DEMO_SAMPLES), 3000.0, &self.svg_colors)
            }
            ChartKind::Heat => {
                crate::svg_graph::heat(&VecDeque::from(DEMO_SAMPLES), 3000, &self.svg_colors)
            }
            ChartKind::Ring | ChartKind::StackedBars => {
                log::error!("Only Line and Heat are supported for Fan");
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
        if self.config.chart == ChartKind::Heat {
            (*super::COLOR_CHOICES_HEAT).into()
        } else {
            (*super::COLOR_CHOICES_LINE).into()
        }
    }

    fn id(&self) -> Option<String> {
        None
    }

    fn kind(&self) -> ChartKind {
        self.config.chart
    }
}

impl Sensor for Fan {
    fn update_config(&mut self, config: &dyn Any, _refresh_rate: u32) {
        if let Some(cfg) = config.downcast_ref::<FanConfig>() {
            self.config = cfg.clone();
            self.svg_colors.set_colors(cfg.colors());
        }
    }

    fn graph_kind(&self) -> ChartKind {
        self.config.chart
    }

    fn set_graph_kind(&mut self, kind: ChartKind) {
        assert!(kind == ChartKind::Line || kind == ChartKind::Heat);
        self.config.chart = kind;
    }

    fn update(&mut self) {
        let mut rpms = Vec::with_capacity(self.fans.len());

        for fan in &self.fans {
            match read_rpm(&fan.path) {
                Ok(rpm) => rpms.push(Some(rpm)),
                Err(e) => {
                    // A fan can disappear at runtime, don't repeat the same error every tick
                    if !self.read_error_logged {
                        info!("Error reading fan {}: {e:?}", fan.name);
                    }
                    rpms.push(None);
                }
            }
        }

        self.read_error_logged |= rpms.iter().any(Option::is_none);
        self.rpms = rpms;

        let fastest = self.fastest_rpm();
        self.max_rpm = self.max_rpm.max(fastest);
        self.samples.push_back(f64::from(fastest));
    }

    fn demo_graph(&self) -> Box<dyn DemoGraph> {
        let mut dmo = Fan::new(Vec::new());
        dmo.update_config(&self.config, 0);
        Box::new(dmo)
    }

    fn chart(
        &'_ self,
        _height_hint: u16,
        _width_hint: u16,
    ) -> cosmic::widget::Container<'_, crate::app::Message, cosmic::Theme, cosmic::Renderer> {
        let svg = match self.config.chart {
            ChartKind::Line => {
                crate::svg_graph::line(&self.samples, f64::from(self.chart_max()), &self.svg_colors)
            }
            ChartKind::Heat => {
                crate::svg_graph::heat(&self.samples, u64::from(self.chart_max()), &self.svg_colors)
            }
            ChartKind::Ring | ChartKind::StackedBars => {
                log::error!("Only Line and Heat are supported for Fan");
                INVALID_IMG.to_string()
            }
        };
        super::svg_icon_container::<Message>(svg)
    }

    fn settings_ui(&'_ self) -> Element<'_, crate::app::Message> {
        let config = &self.config;
        let kind = self.graph_kind();

        let section = settings::section()
            .add(
                settings::item::builder(fl!("enable-chart"))
                    .toggler(config.chart_visible(), Message::ToggleFanChart),
            )
            .add(
                settings::item::builder(fl!("enable-value"))
                    .toggler(config.value_visible(), Message::ToggleFanValue),
            )
            .add(crate::ui::value_colors_row(
                config.use_graph_colors,
                DeviceKind::Fan,
                None,
            ))
            .add(
                settings::item::builder(fl!("enable-label"))
                    .toggler(config.label_visible(), Message::ToggleFanLabel),
            )
            .add(
                settings::item::builder(fl!("enable-icon"))
                    .toggler(config.icon_visible(), Message::ToggleFanIcon),
            )
            .add(ui::chart_type_row(
                &self.graph_options,
                Some(chart_index(kind)),
                |index| Message::SelectGraphType(DeviceKind::Fan, chart_kind(index)),
            ))
            .add(ui::chart_color_row(
                ui::chart_swatch(config.colors(), kind),
                Message::ColorPickerOpen(DeviceKind::Fan, kind, None),
            ));

        let mut fans = String::with_capacity(128);
        fans.push_str(&fl!("fan-description"));
        for (fan, rpm) in self.fans.iter().zip(&self.rpms) {
            match rpm {
                Some(rpm) => _ = write!(fans, "\n{}: {rpm} RPM", fan.name),
                None => _ = write!(fans, "\n{}: -", fan.name),
            }
        }

        cosmic::widget::column::with_capacity(2)
            .push(section)
            .push(widget::text::caption(fans))
            .spacing(cosmic::theme::spacing().space_s)
            .into()
    }
}

impl Default for Fan {
    fn default() -> Self {
        info!("Find fan sensors");
        let fans = match find_fans(Path::new("/sys/class/hwmon")) {
            Ok(fans) => fans,
            Err(e) => {
                info!("Fan:detect: No fan sensors found. {e:?}");
                Vec::new()
            }
        };
        Fan::new(fans)
    }
}

impl Fan {
    fn new(fans: Vec<FanInput>) -> Self {
        let mut fan = Fan {
            rpms: vec![None; fans.len()],
            fans,
            samples: BoundedVecDeque::from_iter(std::iter::repeat_n(0.0, MAX_SAMPLES), MAX_SAMPLES),
            max_rpm: 0,
            read_error_logged: false,
            graph_options: GRAPH_OPTIONS_LINE_HEAT.to_vec(),
            svg_colors: SvgColors::new(&ChartColors::default()),
            config: FanConfig::default(),
        };
        fan.set_colors(&ChartColors::default());
        fan
    }

    // true if at least one fan tachometer was found
    pub fn is_found(&self) -> bool {
        !self.fans.is_empty()
    }

    /// Speed of the fastest fan, the one that best reflects how hard the cooling works.
    pub fn fastest_rpm(&self) -> u32 {
        self.rpms.iter().flatten().copied().max().unwrap_or(0)
    }

    fn chart_max(&self) -> u32 {
        self.max_rpm.max(MIN_CHART_RPM)
    }

    pub fn value(&self, horizontal: bool) -> String {
        if horizontal {
            format!("{} RPM", self.fastest_rpm())
        } else {
            self.fastest_rpm().to_string()
        }
    }

    pub fn value_style(&self) -> cosmic::theme::Text {
        super::temperature_value_style(
            self.config.use_graph_colors,
            self.config.chart,
            self.config.value_style(ColorVariant::Graph1),
            &self.samples,
            f64::from(self.chart_max()),
        )
    }
}

const DEMO_SAMPLES: [f64; 21] = [
    1200.0, 1200.0, 1250.0, 1300.0, 1400.0, 1550.0, 1700.0, 1850.0, 2000.0, 2150.0, 2300.0, 2400.0,
    2500.0, 2600.0, 2650.0, 2700.0, 2750.0, 2800.0, 2850.0, 2900.0, 3000.0,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway hwmon tree under the system temp dir, removed on drop.
    struct FakeHwmon(PathBuf);

    impl FakeHwmon {
        fn new(test: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("minimon-fan-{test}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            FakeHwmon(root)
        }

        fn chip(&self, dir: &str, name: &str, files: &[(&str, &str)]) {
            let chip = self.0.join(dir);
            fs::create_dir_all(&chip).unwrap();
            fs::write(chip.join("name"), format!("{name}\n")).unwrap();
            for (file, content) in files {
                fs::write(chip.join(file), format!("{content}\n")).unwrap();
            }
        }
    }

    impl Drop for FakeHwmon {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sensor(root: &FakeHwmon) -> Fan {
        let mut fan = Fan::new(find_fans(&root.0).unwrap());
        fan.update();
        fan
    }

    #[test]
    fn fastest_of_several_fans_is_reported() {
        let root = FakeHwmon::new("fastest");
        root.chip("hwmon0", "k10temp", &[("temp1_input", "45000")]);
        root.chip(
            "hwmon1",
            "thinkpad",
            &[("fan1_input", "1800"), ("fan2_input", "2400")],
        );

        let fan = sensor(&root);
        assert!(fan.is_found());
        assert_eq!(fan.fans.len(), 2);
        assert_eq!(fan.fastest_rpm(), 2400);
        assert_eq!(fan.value(true), "2400 RPM");
        assert_eq!(fan.value(false), "2400");
    }

    #[test]
    fn no_fans_means_not_found() {
        let root = FakeHwmon::new("none");
        root.chip("hwmon0", "acpitz", &[("temp1_input", "40000")]);

        let fan = sensor(&root);
        assert!(!fan.is_found());
        assert_eq!(fan.fastest_rpm(), 0);
    }

    #[test]
    fn stopped_fan_reads_zero_not_error() {
        let root = FakeHwmon::new("stopped");
        root.chip("hwmon0", "thinkpad", &[("fan1_input", "0")]);

        let fan = sensor(&root);
        assert_eq!(fan.rpms, vec![Some(0)]);
        assert!(!fan.read_error_logged);
    }

    #[test]
    fn label_file_names_the_fan() {
        let root = FakeHwmon::new("label");
        root.chip(
            "hwmon0",
            "nct6798",
            &[
                ("fan1_input", "900"),
                ("fan1_label", "CPU Fan"),
                ("fan2_input", "700"),
            ],
        );

        let fan = sensor(&root);
        let names: Vec<&str> = fan.fans.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["CPU Fan", "nct6798 fan2"]);
    }

    #[test]
    fn unreadable_fan_is_skipped() {
        let root = FakeHwmon::new("unreadable");
        root.chip(
            "hwmon0",
            "thinkpad",
            &[("fan1_input", "not a number"), ("fan2_input", "1500")],
        );

        let fan = sensor(&root);
        assert_eq!(fan.rpms, vec![None, Some(1500)]);
        assert_eq!(fan.fastest_rpm(), 1500);
        assert!(fan.read_error_logged);
    }

    #[test]
    fn chart_picker_round_trips_line_and_heat() {
        for kind in [ChartKind::Line, ChartKind::Heat] {
            assert_eq!(chart_kind(chart_index(kind)), kind);
        }
    }
}
