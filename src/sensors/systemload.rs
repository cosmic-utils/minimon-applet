use std::{any::Any, collections::VecDeque, sync::LazyLock};

use cosmic::{
    Element,
    widget::{Container, settings},
};
use sysinfo::{LoadAvg, System};

use crate::{
    app::Message,
    colorpicker::DemoGraph,
    config::{ChartColors, ChartKind, ColorVariant, DeviceKind, SystemLoadConfig},
    fl,
};

use super::Sensor;
use bounded_vec_deque::BoundedVecDeque;

const MAX_SAMPLES: usize = 21;

static COLOR_CHOICES: LazyLock<[(&'static str, ColorVariant); 5]> = LazyLock::new(|| {
    [
        (fl!("system-load-one").leak(), ColorVariant::Graph1),
        (fl!("system-load-five").leak(), ColorVariant::Graph2),
        (fl!("system-load-fifteen").leak(), ColorVariant::Graph3),
        (fl!("graph-line-back").leak(), ColorVariant::Background),
        (fl!("graph-line-frame").leak(), ColorVariant::Frame),
    ]
});

#[derive(Debug)]
pub struct SystemLoad {
    average: LoadAvg,
    logical_cpus: usize,
    samples: [BoundedVecDeque<f64>; 3],
    config: SystemLoadConfig,
}

impl Default for SystemLoad {
    fn default() -> Self {
        let mut system = System::new();
        system.refresh_cpu_all();
        let logical_cpus = system.cpus().len().max(1);
        let config = SystemLoadConfig::default();
        let average = System::load_average();
        let samples = [average.one, average.five, average.fifteen].map(|value| {
            let mut samples =
                BoundedVecDeque::from_iter(std::iter::repeat_n(0.0, MAX_SAMPLES), MAX_SAMPLES);
            samples[MAX_SAMPLES - 1] = value;
            samples
        });
        Self {
            average,
            logical_cpus,
            samples,
            config,
        }
    }
}

impl SystemLoad {
    /// Load averages are counts, not CPU percentages; retain all three windows.
    pub fn value(&self, vertical: bool) -> String {
        let LoadAvg { one, five, fifteen } = self.average;
        if vertical {
            format!("{one:.2}\n{five:.2}\n{fifteen:.2}")
        } else {
            format!("{one:.2} | {five:.2} | {fifteen:.2}")
        }
    }

    /// Keep the value and graph color paired in the same 1/5/15-minute order.
    pub fn readings(&self) -> [(f64, cosmic::iced::Color); 3] {
        let colors = self.config.colors();
        [
            (self.average.one, colors.graph1),
            (self.average.five, colors.graph2),
            (self.average.fifteen, colors.graph3),
        ]
        .map(|(value, color)| (value, cosmic::iced::Color::from(color)))
    }

    fn record(&mut self, average: LoadAvg) {
        self.average = average;
        for (samples, value) in
            self.samples
                .iter_mut()
                .zip([self.average.one, self.average.five, self.average.fifteen])
        {
            samples.push_back(value);
        }
    }

    fn scale(&self) -> f64 {
        self.samples
            .iter()
            .flat_map(|samples| samples.iter())
            .copied()
            .fold(1.0, f64::max)
    }
}

impl Sensor for SystemLoad {
    fn update_config(&mut self, config: &dyn Any, _refresh_rate: u32) {
        if let Some(config) = config.downcast_ref::<SystemLoadConfig>() {
            self.config = config.clone();
        }
    }

    fn graph_kind(&self) -> ChartKind {
        ChartKind::Line
    }

    fn set_graph_kind(&mut self, kind: ChartKind) {
        assert_eq!(kind, ChartKind::Line);
    }

    fn update(&mut self) {
        self.record(System::load_average());
    }

    fn demo_graph(&self) -> Box<dyn DemoGraph> {
        let mut demo = Self::default();
        demo.update_config(&self.config, 0);
        Box::new(demo)
    }

    fn chart(
        &self,
        _height_hint: u16,
        _width_hint: u16,
    ) -> Container<'_, Message, cosmic::Theme, cosmic::Renderer> {
        let svg = crate::svg_graph::triple_line(
            [&self.samples[0], &self.samples[1], &self.samples[2]],
            self.scale(),
            self.config.colors(),
            Some(self.logical_cpus as f64),
        );
        super::svg_icon_container::<Message>(svg)
    }

    fn settings_ui(&self) -> Element<'_, Message> {
        let mut section = settings::section();
        for ((value, color), (label, _)) in self.readings().into_iter().zip(COLOR_CHOICES.iter()) {
            let mut text = cosmic::widget::text::body(format!("{value:.2}"));
            if self.config.use_graph_colors {
                text = text.class(cosmic::theme::Text::Color(color));
            }
            section = section.add(crate::ui::control_row(*label, text));
        }
        section
            .add(crate::ui::control_row(
                fl!("system-load-capacity"),
                cosmic::widget::text::body(format!("┄ {}", self.logical_cpus)),
            ))
            .add(
                settings::item::builder(fl!("enable-chart"))
                    .description(fl!("system-load-description"))
                    .toggler(self.config.chart_visible(), Message::ToggleSystemLoadChart),
            )
            .add(
                settings::item::builder(fl!("enable-value"))
                    .toggler(self.config.value_visible(), Message::ToggleSystemLoadValue),
            )
            .add(
                settings::item::builder(fl!("use-graph-colors"))
                    .description(fl!("use-graph-colors-description"))
                    .toggler(
                        self.config.use_graph_colors,
                        Message::ToggleSystemLoadGraphColors,
                    ),
            )
            .add(
                settings::item::builder(fl!("enable-label"))
                    .toggler(self.config.label_visible(), Message::ToggleSystemLoadLabel),
            )
            .add(
                settings::item::builder(fl!("enable-icon"))
                    .toggler(self.config.icon_visible(), Message::ToggleSystemLoadIcon),
            )
            .add(crate::ui::chart_colors_row(
                self.config.colors().graph1,
                Message::ColorPickerOpen(DeviceKind::SystemLoad, ChartKind::Line, None),
            ))
            .into()
    }
}

impl DemoGraph for SystemLoad {
    fn demo(&self) -> String {
        let samples = VecDeque::from([
            0.2, 0.3, 0.4, 0.3, 0.6, 0.8, 1.0, 1.2, 1.4, 1.3, 1.5, 1.8, 2.0, 1.9, 1.7, 1.8, 1.6,
            1.5, 1.4, 1.2, 1.0,
        ]);
        let samples = samples
            .into_iter()
            .map(|value| value * self.logical_cpus as f64)
            .collect::<VecDeque<_>>();
        let five = samples
            .iter()
            .map(|value| self.logical_cpus as f64 * 0.5 + value * 0.5)
            .collect();
        let fifteen = samples
            .iter()
            .map(|value| self.logical_cpus as f64 * 0.9 + value * 0.1)
            .collect();
        crate::svg_graph::triple_line(
            [&samples, &five, &fifteen],
            2.0 * self.logical_cpus as f64,
            self.config.colors(),
            Some(self.logical_cpus as f64),
        )
    }

    fn colors(&self) -> &ChartColors {
        self.config.colors()
    }

    fn set_colors(&mut self, colors: &ChartColors) {
        *self.config.colors_mut() = *colors;
    }

    fn kind(&self) -> ChartKind {
        ChartKind::Line
    }

    fn color_choices(&self) -> Vec<(&'static str, ColorVariant)> {
        (*COLOR_CHOICES).into()
    }

    fn id(&self) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_preserve_all_windows_and_values_above_one_hundred() {
        let mut sensor = SystemLoad::default();
        sensor.record(LoadAvg {
            one: 128.25,
            five: 16.5,
            fifteen: 0.75,
        });
        assert_eq!(sensor.value(false), "128.25 | 16.50 | 0.75");
        assert_eq!(sensor.value(true), "128.25\n16.50\n0.75");
        assert_eq!(sensor.scale(), 128.25);
    }

    #[test]
    fn history_is_bounded_and_scale_recovers_after_a_peak() {
        let mut sensor = SystemLoad::default();
        sensor.record(LoadAvg {
            one: 8.0,
            five: 4.0,
            fifteen: 2.0,
        });
        for _ in 0..MAX_SAMPLES {
            sensor.record(LoadAvg {
                one: 0.0,
                five: 0.0,
                fifteen: 0.0,
            });
        }
        for samples in &sensor.samples {
            assert_eq!(samples.len(), MAX_SAMPLES);
            assert!(samples.iter().all(|sample| *sample == 0.0));
        }
        assert_eq!(sensor.scale(), 1.0);
    }
    #[test]
    fn all_windows_keep_their_history_and_share_the_largest_scale() {
        let mut sensor = SystemLoad::default();
        for _ in 0..MAX_SAMPLES {
            sensor.record(LoadAvg {
                one: 2.0,
                five: 16.0,
                fifteen: 8.0,
            });
        }
        assert_eq!(sensor.scale(), 16.0);
        for (samples, expected) in sensor.samples.iter().zip([2.0, 16.0, 8.0]) {
            assert!(samples.iter().all(|value| *value == expected));
        }
        let readings = sensor.readings();
        assert_eq!(readings.map(|(value, _)| value), [2.0, 16.0, 8.0]);
        assert_ne!(readings[0].1, readings[1].1);
        assert_ne!(readings[1].1, readings[2].1);
        assert_ne!(readings[0].1, readings[2].1);
    }
}
