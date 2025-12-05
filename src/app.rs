use cosmic::applet::cosmic_panel_config::PanelSize;
use cosmic::applet::{PanelType, Size};
use cosmic::config::FontConfig;
use cosmic::cosmic_config::CosmicConfigEntry;
use cosmic::cosmic_theme::palette::bool_mask::BoolMask;
use cosmic::cosmic_theme::palette::{FromColor, WithAlpha};
use cosmic::iced::advanced::graphics::text::cosmic_text::{Buffer, FontSystem, Metrics, Shaping};
use cosmic::iced::alignment::Horizontal::{self};
use cosmic::iced_winit::graphics::text::cosmic_text::Attrs;

use std::collections::{BTreeMap, VecDeque};
use std::{fs, time};

use cosmic::app::{Core, Task};
use cosmic::iced::Limits;
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::{self, Subscription};
use cosmic::widget::{button, container, horizontal_space, list, settings, spin_button, text};
use cosmic::{Apply, Element};
use cosmic::{widget, widget::autosize};

use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{self, AtomicU32};

use cosmic::{
    applet::cosmic_panel_config::PanelAnchor,
    iced::{Alignment, widget::column, widget::row},
    iced_widget::{Column, Row},
};

use zbus::blocking::Connection;
use zvariant::OwnedObjectPath;

use log::{error, info};

use crate::barchart::StackedBarSvg;
use crate::colorpicker::ColorPicker;
use crate::config::{
    ChartColors, ChartKind, ColorVariant, ContentType, DeviceKind, DisksVariant, GpuConfig,
    NetworkVariant,
};
use crate::sensors::cpu::Cpu;
use crate::sensors::cputemp::CpuTemp;
use crate::sensors::disks::{self, Disks};
use crate::sensors::gpus::{Gpu, list_gpus};
use crate::sensors::memory::Memory;
use crate::sensors::network::{self, Network};
use crate::sensors::{Sensor, TempUnit};
use crate::system_monitors;
use crate::{config::MinimonConfig, fl};

use cosmic::widget::Id as WId;

static AUTOSIZE_MAIN_ID: LazyLock<WId> = std::sync::LazyLock::new(|| WId::new("autosize-main"));

const ICON: &str = "io.github.cosmic_utils.minimon-applet";
const CPU_ICON: &str = "io.github.cosmic_utils.minimon-applet-cpu";
const TEMP_ICON: &str = "io.github.cosmic_utils.minimon-applet-temperature";
const RAM_ICON: &str = "io.github.cosmic_utils.minimon-applet-ram";
const GPU_ICON: &str = "io.github.cosmic_utils.minimon-applet-gpu";
const NETWORK_ICON: &str = "io.github.cosmic_utils.minimon-applet-network";
const DISK_ICON: &str = "io.github.cosmic_utils.minimon-applet-harddisk";

const DEFAULT_MONITOR: &str = "GNOME System Monitor";

pub static SETTINGS_CPU_CHOICE: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-cpu").leak());
pub static SETTINGS_CPU_TEMP_CHOICE: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-cpu-temperature").leak());
pub static SETTINGS_MEMORY_CHOICE: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-memory").leak());
pub static SETTINGS_NETWORK_CHOICE: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-network").leak());
pub static SETTINGS_DISKS_CHOICE: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-disks").leak());
pub static SETTINGS_GPU_CHOICE: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-gpu").leak());

pub static SETTINGS_GENERAL_HEADING: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-subpage-general").leak());
pub static SETTINGS_BACK: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-subpage-back").leak());
pub static SETTINGS_CPU_HEADING: LazyLock<&'static str> = LazyLock::new(|| fl!("cpu-title").leak());
pub static SETTINGS_CPU_TEMP_HEADING: LazyLock<&'static str> =
    LazyLock::new(|| fl!("cpu-temperature-title").leak());
pub static SETTINGS_MEMORY_HEADING: LazyLock<&'static str> =
    LazyLock::new(|| fl!("memory-title").leak());
pub static SETTINGS_NETWORK_HEADING: LazyLock<&'static str> =
    LazyLock::new(|| fl!("net-title").leak());
pub static SETTINGS_DISKS_HEADING: LazyLock<&'static str> =
    LazyLock::new(|| fl!("disks-title").leak());
pub static SETTINGS_GPU_HEADING: LazyLock<&'static str> = LazyLock::new(|| fl!("gpu-title").leak());

// The UI requires static lifetime of dropdown items
pub static SYSMON_LIST: LazyLock<BTreeMap<String, system_monitors::DesktopApp>> =
    LazyLock::new(system_monitors::get_desktop_applications);

pub static SYSMON_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| SYSMON_LIST.values().map(|app| app.name.as_str()).collect());

macro_rules! network_select {
    ($self:ident, $variant:expr) => {
        match $variant {
            NetworkVariant::Combined | NetworkVariant::Download => {
                (&mut $self.network1, &mut $self.config.network1)
            }
            _ => (&mut $self.network2, &mut $self.config.network2),
        }
    };
}

macro_rules! disks_select {
    ($self:ident, $variant:expr) => {
        match $variant {
            DisksVariant::Combined | DisksVariant::Write => {
                (&mut $self.disks1, &mut $self.config.disks1)
            }
            _ => (&mut $self.disks2, &mut $self.config.disks2),
        }
    };
}

macro_rules! settings_sub_page_heading {
    ($heading:ident) => {
        Minimon::sub_page_header(Some(&$heading), &SETTINGS_BACK, Message::Settings(None))
    };
}

#[derive(Debug, Clone)]
pub enum SettingsVariant {
    General,
    Cpu,
    CpuTemp,
    Memory,
    Network,
    Disks,
    Gpu(String),
}

pub struct Minimon {
    /// Application state which is managed by the COSMIC runtime.
    core: Core,
    /// The svg image to draw for the CPU load
    cpu: Cpu,
    /// The svg image to draw for the CPU load
    cputemp: CpuTemp,
    /// The svg image to draw for the Memory load
    memory: Memory,

    /// The network monitor, if combined we only use the first one
    network1: Network,
    network2: Network,

    /// The network monitor
    disks1: Disks,
    disks2: Disks,

    //GPUs, in Btree so they're always ordered the same.
    gpus: BTreeMap<String, Gpu>,

    /// The popup id.
    popup: Option<Id>,

    /// Current settings sub page
    settings_page: Option<SettingsVariant>,

    /// The color picker dialog
    colorpicker: ColorPicker,

    /// Settings stored on disk, including refresh rate, colors, etc.
    config: MinimonConfig,

    /// tick can be 250, 500 or 1000, depending on refresh rate modolu tick
    refresh_rate: Arc<AtomicU32>,

    // On AC or battery?
    is_laptop: bool,
    on_ac: bool,

    // Tracks whether any chart or label is showing on the panel
    data_is_visible: bool,

    // Used to measure label width, have to be cached because slow to load
    font_system: FontSystem,

    interface_font: Option<FontConfig>,

    // Pre-calc the max width of labels to avoid panel wobble
    label_cpu_width: Option<f32>,
    label_gpu_width: Option<f32>,
    label_network_width: Option<f32>,
    label_disks_width: Option<f32>,
    label_w_width: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct ContentOrderChange {
    pub current_index: usize,
    pub new_index: usize,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,

    ColorPickerOpen(DeviceKind, ChartKind, Option<String>),
    ColorPickerClose(bool, Option<String>),
    ColorPickerDefaults,
    ColorPickerAccent,

    ColorPickerSliderRedChanged(u8),
    ColorPickerSliderGreenChanged(u8),
    ColorPickerSliderBlueChanged(u8),
    ColorPickerSliderAlphaChanged(u8),
    ColorPickerSelectVariant(ColorVariant),

    ColorTextInputRedChanged(String),
    ColorTextInputGreenChanged(String),
    ColorTextInputBlueChanged(String),
    ColorTextInputAlphaChanged(String),

    ToggleNetBytes(bool),
    ToggleNetCombined(bool),
    ToggleNetChart(NetworkVariant, bool),
    ToggleNetLabel(NetworkVariant, bool),
    ToggleAdaptiveNet(NetworkVariant, bool),
    NetworkSelectUnit(NetworkVariant, usize),
    TextInputBandwidthChanged(NetworkVariant, String),

    ToggleDisksCombined(bool),
    ToggleDisksChart(DisksVariant, bool),
    ToggleDisksLabel(DisksVariant, bool),

    SelectGraphType(DeviceKind, ChartKind),
    Tick,
    SlowTimer,
    PopupClosed(Id),

    ToggleCpuChart(bool),
    ToggleCpuLabel(bool),
    ToggleCpuTempChart(bool),
    ToggleCpuTempLabel(bool),
    ToggleCpuNoDecimals(bool),
    CpuBarSizeChanged(u16),
    CpuNarrowBarSpacing(bool),
    ToggleMemoryChart(bool),
    ToggleMemoryLabel(bool),
    ToggleMemoryPercentage(bool),
    ToggleMemoryAllocated(bool),
    ConfigChanged(Box<MinimonConfig>),
    ThemeChanged(Box<cosmic::config::CosmicTk>),
    LaunchSystemMonitor(&'static system_monitors::DesktopApp),
    RefreshRateChanged(f64),
    LabelSizeChanged(u16),
    ToggleMonospaceLabels(bool),
    PanelSpacing(u16),
    SelectCpuTempUnit(TempUnit),
    CpuTempMinTempChanged(f64),

    Settings(Option<SettingsVariant>),

    GpuToggleChart(String, DeviceKind, bool),
    GpuToggleLabel(String, DeviceKind, bool),
    GpuToggleStackLabels(String, bool),
    GpuSelectGraphType(String, DeviceKind, ChartKind),
    SelectGpuTempUnit(String, TempUnit),
    GpuTempMinTempChanged(String, f64),
    ToggleDisableOnBattery(String, bool),
    ToggleSymbols(bool),
    SysmonSelect(usize),

    ChangeContentOrder(ContentOrderChange),

    Tip,
}

const APP_ID_DOCK: &str = "io.github.cosmic_utils.minimon-applet-dock";
const APP_ID_PANEL: &str = "io.github.cosmic_utils.minimon-applet-panel";
const APP_ID_OTHER: &str = "io.github.cosmic_utils.minimon-applet-other";

impl cosmic::Application for Minimon {
    type Executor = cosmic::executor::Default;

    type Flags = ();

    type Message = Message;

    const APP_ID: &'static str = "io.github.cosmic_utils.minimon-applet";

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let is_laptop = Minimon::is_laptop();
        if is_laptop {
            info!("Is laptop");
        }

        // Find GPUs
        let gpus: BTreeMap<String, Gpu> = list_gpus()
            .into_iter()
            .map(|mut gpu| {
                info!("Found GPU. Name: {}. UUID: {}", gpu.name(), gpu.id());
                if is_laptop {
                    gpu.set_laptop();
                }
                (gpu.id().to_string(), gpu)
            })
            .collect();

        let is_horizontal = core.applet.is_horizontal();

        let app = Minimon {
            core,
            cpu: Cpu::new(is_horizontal),
            cputemp: CpuTemp::default(),
            memory: Memory::default(),
            network1: Network::default(),
            network2: Network::default(),
            disks1: Disks::default(),
            disks2: Disks::default(),
            gpus,
            popup: None,
            settings_page: None,
            colorpicker: ColorPicker::default(),
            config: MinimonConfig::default(),
            refresh_rate: Arc::new(AtomicU32::new(1000)),
            is_laptop,
            on_ac: true,
            data_is_visible: false,
            font_system: FontSystem::new(),
            interface_font: None,
            label_cpu_width: None,
            label_gpu_width: None,
            label_network_width: None,
            label_disks_width: None,
            label_w_width: None,
        };

        (app, Task::none())
    }

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }

    fn subscription(&self) -> Subscription<Message> {
        fn time_subscription(tick: &std::sync::Arc<AtomicU32>) -> Subscription<time::Instant> {
            let atomic = tick.clone();
            let val = atomic.load(atomic::Ordering::Relaxed);
            iced::time::every(time::Duration::from_millis(u64::from(val)))
        }

        fn slow_time_subscription() -> Subscription<time::Instant> {
            iced::time::every(time::Duration::from_millis(3000))
        }

        let mut subscriptions: Vec<Subscription<Message>> = vec![
            time_subscription(&self.refresh_rate).map(|_| Message::Tick),
            self.core
                .watch_config(match self.core.applet.panel_type {
                    PanelType::Panel => APP_ID_PANEL,
                    PanelType::Dock => APP_ID_DOCK,
                    PanelType::Other(_) => APP_ID_OTHER,
                })
                .map(|u| Message::ConfigChanged(Box::new(u.config))),
        ];

        subscriptions.push(slow_time_subscription().map(|_| Message::SlowTimer));

        subscriptions.push(
            self.core
                .watch_config("com.system76.CosmicTk")
                .map(|u| Message::ThemeChanged(Box::new(u.config))),
        );

        Subscription::batch(subscriptions)
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&'_ self) -> Element<'_, Message> {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();
        let horizontal = matches!(
            self.core.applet.anchor,
            PanelAnchor::Top | PanelAnchor::Bottom
        );

        let mut limits = Limits::NONE.min_width(1.).min_height(1.);
        if let Some(b) = self.core.applet.suggested_bounds {
            if b.width > 0.0 {
                limits = limits.max_width(b.width);
            }
            if b.height > 0.0 {
                limits = limits.max_height(b.height);
            }
        }

        // If the applet is not visible, return an icon button to toggle the popup
        if !self.data_is_visible {
            return self
                .core
                .applet
                .icon_button(ICON)
                .on_press(Message::TogglePopup)
                .into();
        }

        // Build the full list of panel elements
        let mut elements: Vec<Element<Message>> = Vec::new();

        for content in &self.config.content_order.order {
            match content {
                ContentType::CpuUsage => {
                    elements.extend(self.cpu_panel_ui(horizontal));
                }
                ContentType::CpuTemp => {
                    elements.extend(self.cpu_temp_panel_ui(horizontal));
                }
                ContentType::MemoryUsage => {
                    elements.extend(self.memory_panel_ui(horizontal));
                }
                ContentType::NetworkUsage => {
                    elements.extend(self.network_panel_ui(horizontal));
                }
                ContentType::DiskUsage => {
                    elements.extend(self.disks_panel_ui(horizontal));
                }
                ContentType::GpuInfo => {
                    for gpu in self.gpus.values() {
                        elements.extend(self.gpu_panel_ui(gpu, horizontal));
                    }
                }
            }
        }

        let spacing = match self.config.panel_spacing {
            1 => cosmic.space_xxxs(),
            2 => cosmic.space_xxs(),
            3 => cosmic.space_xs(),
            4 => cosmic.space_s(),
            5 => cosmic.space_m(),
            6 => cosmic.space_l(),
            _ => {
                error!("Invalid spacing selected");
                cosmic.space_xs()
            }
        };

        // Layout horizontally or vertically
        let wrapper: Element<Message> = if horizontal {
            Row::from_vec(elements)
                .align_y(Alignment::Center)
                .spacing(spacing)
                .into()
        } else {
            Column::from_vec(elements)
                .align_x(Alignment::Center)
                .spacing(spacing)
                .into()
        };

        let button = widget::button::custom(wrapper)
            .padding(if horizontal {
                [0, self.core.applet.suggested_padding(true).1]
            } else {
                [self.core.applet.suggested_padding(true).0, 0]
            })
            .class(cosmic::theme::Button::AppletIcon)
            .on_press(Message::TogglePopup);

        autosize::autosize(container(button), AUTOSIZE_MAIN_ID.clone())
            .limits(limits)
            .into()
    }

    // Settings popup, can be list overview, individual page or colorpicker
    fn view_window(&'_ self, _id: Id) -> Element<'_, Self::Message> {
        // Get configured system monitor, else the DEFAULT one, else first one in the map, else None.
        fn get_sysmon(name: &Option<String>) -> Option<&'static system_monitors::DesktopApp> {
            match &name {
                Some(key) if SYSMON_LIST.contains_key(key.as_str()) => {
                    SYSMON_LIST.get(key.as_str())
                }
                _ => {
                    if SYSMON_LIST.contains_key(DEFAULT_MONITOR) {
                        SYSMON_LIST.get(DEFAULT_MONITOR)
                    } else {
                        SYSMON_LIST.values().next()
                    }
                }
            }
        }
        // Colorpicker
        if self.colorpicker.active() {
            let limits = Limits::NONE
                .max_width(400.0)
                .min_width(400.0)
                .min_height(200.0)
                .max_height(750.0);

            self.core
                .applet
                .popup_container(self.colorpicker.view_colorpicker())
                .limits(limits)
                .into()

        // Individual settingspage
        } else {
            let theme = cosmic::theme::active();

            let padding = if self.core.is_condensed() {
                theme.cosmic().space_s()
            } else {
                theme.cosmic().space_l()
            };

            let mut content = Column::new();

            if let Some(variant) = &self.settings_page {
                match variant {
                    SettingsVariant::Cpu => {
                        content = content.push(settings_sub_page_heading!(SETTINGS_CPU_HEADING));
                        content = content.push(self.cpu.settings_ui());
                    }
                    SettingsVariant::CpuTemp => {
                        content =
                            content.push(settings_sub_page_heading!(SETTINGS_CPU_TEMP_HEADING));
                        content = content.push(self.cputemp.settings_ui());
                    }
                    SettingsVariant::Memory => {
                        content = content.push(Minimon::sub_page_header(
                            Some(&SETTINGS_MEMORY_HEADING),
                            &SETTINGS_BACK,
                            Message::Settings(None),
                        ));
                        content = content.push(self.memory.settings_ui());
                    }
                    SettingsVariant::Network => {
                        content =
                            content.push(settings_sub_page_heading!(SETTINGS_NETWORK_HEADING));
                        content = content.push(settings::item(
                            fl!("enable-net-combined"),
                            widget::toggler(
                                self.config.network1.variant == NetworkVariant::Combined,
                            )
                            .on_toggle(Message::ToggleNetCombined),
                        ));
                        content = content.push(settings::item(
                            fl!("net-use-bytes"),
                            widget::toggler(self.config.network1.show_bytes)
                                .on_toggle(Message::ToggleNetBytes),
                        ));
                        content = content.push(self.network1.settings_ui());
                        if self.config.network1.variant == NetworkVariant::Download {
                            content = content.push(self.network2.settings_ui());
                        }
                    }
                    SettingsVariant::Disks => {
                        content = content.push(settings_sub_page_heading!(SETTINGS_DISKS_HEADING));
                        content = content.push(settings::item(
                            fl!("enable-disks-combined"),
                            widget::toggler(self.config.disks1.variant == DisksVariant::Combined)
                                .on_toggle(Message::ToggleDisksCombined),
                        ));
                        content = content.push(self.disks1.settings_ui());
                        if self.config.disks1.variant == DisksVariant::Write {
                            content = content.push(self.disks2.settings_ui());
                        }
                    }
                    SettingsVariant::Gpu(id) => {
                        content = content.push(settings_sub_page_heading!(SETTINGS_GPU_HEADING));

                        if let (Some(gpu), Some(config)) =
                            (self.gpus.get(id), self.config.gpus.get(id))
                        {
                            content = content.push(
                                widget::row::with_capacity(2)
                                    .push(text::heading(gpu.name()))
                                    .spacing(cosmic::theme::spacing().space_m),
                            );
                            content = content.push(gpu.settings_ui(config));
                        } else {
                            error!("SettingsVariant::Gpu: Not found {id}");
                        }
                    }
                    SettingsVariant::General => {
                        content =
                            content.push(settings_sub_page_heading!(SETTINGS_GENERAL_HEADING));
                        content = content.push(self.general_settings_ui());
                    }
                }

            // List settings overview
            } else {
                if let Some(sysmon) = get_sysmon(&self.config.sysmon) {
                    content = content.push(Element::from(row!(
                        widget::horizontal_space(),
                        widget::button::standard(sysmon.name.to_owned())
                            .on_press(Message::LaunchSystemMonitor(sysmon))
                            .trailing_icon(widget::button::link::icon()),
                        widget::horizontal_space()
                    )));
                }

                let cpu = widget::text::body(self.cpu.to_string());
                let cputemp = widget::text::body(self.cputemp.to_string());
                let memory = widget::text::body(format!(
                    "{} / {:.1} GB / {:.1} GB",
                    self.memory.to_string(false),
                    self.memory.latest_sample_allocated(),
                    self.memory.total()
                ));

                let sample_rate_ms = self.config.refresh_rate;
                let network = widget::text::body(format!(
                    "↓ {} ↑ {}",
                    &self
                        .network1
                        .download_label(sample_rate_ms, network::UnitVariant::Long),
                    &self
                        .network1
                        .upload_label(sample_rate_ms, network::UnitVariant::Long)
                ));

                let disks = widget::text::body(format!(
                    "w {} r {}",
                    &self
                        .disks1
                        .write_label(sample_rate_ms, disks::UnitVariant::Long),
                    &self
                        .disks1
                        .read_label(sample_rate_ms, disks::UnitVariant::Long)
                ));

                let mut sensor_settings = list::ListColumn::new()
                    .add(Minimon::go_next_with_item(
                        &SETTINGS_GENERAL_HEADING,
                        text::body(""),
                        Message::Settings(Some(SettingsVariant::General)),
                    ))
                    .add(Minimon::go_next_with_item(
                        &SETTINGS_CPU_CHOICE,
                        cpu,
                        Message::Settings(Some(SettingsVariant::Cpu)),
                    ));

                if self.cputemp.is_found() {
                    sensor_settings = sensor_settings.add(Minimon::go_next_with_item(
                        &SETTINGS_CPU_TEMP_CHOICE,
                        cputemp,
                        Message::Settings(Some(SettingsVariant::CpuTemp)),
                    ));
                }

                sensor_settings = sensor_settings
                    .add(Minimon::go_next_with_item(
                        &SETTINGS_MEMORY_CHOICE,
                        memory,
                        Message::Settings(Some(SettingsVariant::Memory)),
                    ))
                    .add(Minimon::go_next_with_item(
                        &SETTINGS_NETWORK_CHOICE,
                        network,
                        Message::Settings(Some(SettingsVariant::Network)),
                    ))
                    .add(Minimon::go_next_with_item(
                        &SETTINGS_DISKS_CHOICE,
                        disks,
                        Message::Settings(Some(SettingsVariant::Disks)),
                    ))
                    .padding(0);

                if self.has_gpus() {
                    for (key, gpu) in &self.gpus {
                        let temp = gpu.temp.to_string();
                        let info = widget::text::body(format!(
                            "{} {} / {:.2} GB {}",
                            gpu.gpu,
                            gpu.vram.string(false),
                            gpu.vram.total(),
                            temp
                        ));
                        sensor_settings = sensor_settings.add(Minimon::go_next_with_item(
                            &SETTINGS_GPU_CHOICE,
                            info,
                            Message::Settings(Some(SettingsVariant::Gpu(key.clone()))),
                        ));
                    }
                }

                content = content.push(sensor_settings);
            }

            content = content.padding(padding).spacing(padding);

            //let content = column!(sensor_settings);
            let limits = Limits::NONE
                .max_width(420.0)
                .min_width(360.0)
                .min_height(200.0)
                .max_height(600.0);

            self.core
                .applet
                .popup_container(content.apply(cosmic::widget::scrollable))
                .limits(limits)
                .into()
        }
    }

    /// Application messages are handled here. The application state can be modified based on
    /// what message was received. Commands may be returned for asynchronous execution on a
    /// background thread managed by the application's executor.
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::ThemeChanged(cosmictk) => {
                let new_font = cosmictk.interface_font;

                if self.interface_font.as_ref() != Some(&new_font) {
                    info!("Message::ThemeChanged. Font is now: {new_font:?}");
                    self.interface_font = Some(new_font);
                    self.calculate_max_label_widths();
                }
            }

            Message::TogglePopup => {
                info!("Message::TogglePopup");
                if let Some(p) = self.popup.take() {
                    self.colorpicker.deactivate();
                    // but have to go back to sleep if settings closed
                    self.maybe_stop_gpus();
                    return destroy_popup(p);
                } else {
                    self.calculate_max_label_widths();
                    let new_id = Id::unique();
                    self.popup.replace(new_id);

                    if let Some(main_id) = self.core.main_window_id() {
                        let mut popup_settings = self
                            .core
                            .applet
                            .get_popup_settings(main_id, new_id, None, None, None);
                        popup_settings.positioner.size_limits = Limits::NONE;
                        return get_popup(popup_settings);
                    }
                };
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.colorpicker.deactivate();
                    self.popup = None;
                }
            }
            Message::ColorPickerOpen(device, kind, id) => {
                // colorpicker is only activated when the settings popup is already open
                // so it takes it over
                info!("Message::ColorPickerOpen({kind:?}, {id:?})");
                match device {
                    DeviceKind::Cpu => {
                        self.colorpicker.activate(device, self.cpu.demo_graph());
                    }
                    DeviceKind::CpuTemp => {
                        self.colorpicker.activate(device, self.cputemp.demo_graph());
                    }
                    DeviceKind::Memory => {
                        self.colorpicker.activate(device, self.memory.demo_graph());
                    }
                    DeviceKind::Network(variant) => {
                        let (network, _config) = network_select!(self, variant);
                        self.colorpicker.activate(device, network.demo_graph());
                    }
                    DeviceKind::Disks(variant) => {
                        let (disks, _) = disks_select!(self, variant);
                        self.colorpicker.activate(device, disks.demo_graph());
                    }
                    DeviceKind::Gpu | DeviceKind::Vram | DeviceKind::GpuTemp => {
                        if let Some(id) = id {
                            if let Some(gpu) = self.gpus.get(&id) {
                                self.colorpicker.activate(device, gpu.demo_graph(device));
                            } else {
                                error!("no config for selected GPU {id}");
                            }
                        } else {
                            error!("Id is None");
                        }
                    }
                }
                self.colorpicker.set_color_variant(ColorVariant::Background);
            }

            Message::ColorPickerClose(save, maybe_gpu_id) => {
                info!("Message::ColorPickerClose({save},{maybe_gpu_id:?})");
                if save {
                    let cols = *self.colorpicker.colors();
                    self.save_colors(&cols, self.colorpicker.device(), maybe_gpu_id);
                    self.save_config();
                }
                self.colorpicker.deactivate();
            }

            Message::ColorPickerDefaults => {
                info!("Message::ColorPickerDefaults()");
                self.colorpicker.default_colors();
            }

            Message::ColorPickerAccent => {
                info!("Message::ColorPickerAccent()");
                if let Some(theme) = self.core.applet.theme() {
                    let srgba = cosmic::cosmic_theme::palette::Srgba::from_color(
                        theme.cosmic().accent_color().color,
                    );
                    self.colorpicker.update_color(srgba.opaque().into());
                }
            }

            Message::ColorPickerSliderRedChanged(val) => {
                let mut col = self.colorpicker.sliders();
                col.red = val;
                self.colorpicker.update_color(col);
            }

            Message::ColorPickerSliderGreenChanged(val) => {
                let mut col = self.colorpicker.sliders();
                col.green = val;
                self.colorpicker.update_color(col);
            }

            Message::ColorPickerSliderBlueChanged(val) => {
                let mut col = self.colorpicker.sliders();
                col.blue = val;
                self.colorpicker.update_color(col);
            }

            Message::ColorPickerSliderAlphaChanged(val) => {
                let mut col = self.colorpicker.sliders();
                col.alpha = val;
                self.colorpicker.update_color(col);
            }

            Message::ColorPickerSelectVariant(variant) => {
                self.colorpicker.set_color_variant(variant);
            }

            Message::ToggleNetBytes(toggle) => {
                info!("Message::ToggleNetBytes({toggle})");
                self.config.network1.show_bytes = toggle;
                self.config.network2.show_bytes = toggle;
                self.save_config();
            }

            Message::ToggleNetCombined(toggle) => {
                info!("Message::ToggleNetCombined({toggle})");
                if toggle.is_true() {
                    self.config.network1.variant = NetworkVariant::Combined;
                } else {
                    self.config.network1.variant = NetworkVariant::Download;
                }
                self.config.network2.variant = NetworkVariant::Upload;
                self.save_config();
            }

            Message::ToggleDisksCombined(toggle) => {
                info!("Message::ToggleDisksCombined({toggle})");
                if toggle.is_true() {
                    self.config.disks1.variant = DisksVariant::Combined;
                } else {
                    self.config.disks1.variant = DisksVariant::Write;
                }
                self.config.disks2.variant = DisksVariant::Read;
                self.save_config();
            }

            Message::ToggleDisksChart(variant, toggled) => {
                info!("Message::ToggleDiskChart({variant:?})");
                let (_, config) = disks_select!(self, variant);
                config.show_chart(toggled);
                self.save_config();
            }

            Message::ToggleDisksLabel(variant, toggled) => {
                info!("Message::ToggleDiskLabel({variant:?})");
                let (_, config) = disks_select!(self, variant);
                config.show_label(toggled);
                self.save_config();
            }

            Message::ToggleAdaptiveNet(variant, toggle) => {
                info!("Message::ToggleAdaptiveNet({variant:?}, {toggle:?})");
                let (_network, config) = network_select!(self, variant);
                config.adaptive = toggle;
                self.save_config();
            }

            Message::NetworkSelectUnit(variant, unit) => {
                let (_, config) = network_select!(self, variant);
                config.unit = Some(unit);
                self.save_config();
            }

            Message::SelectGraphType(dev, kind) => {
                info!("Message::SelectGraphType({dev:?})");
                match dev {
                    DeviceKind::Cpu => {
                        self.cpu.set_graph_kind(kind);
                        self.config.cpu.chart = kind;
                    }
                    DeviceKind::CpuTemp => {
                        self.cputemp.set_graph_kind(kind);
                        self.config.cputemp.chart = kind;
                    }
                    DeviceKind::Memory => {
                        self.memory.set_graph_kind(kind);
                        self.config.memory.chart = kind;
                    }
                    _ => error!("Message::SelectGraphType unsupported kind/device combination."), // Disks and Network don't have graph selection
                }
                self.save_config();
            }

            Message::TextInputBandwidthChanged(variant, string) => {
                let value = if string.is_empty() {
                    Some(0)
                } else {
                    string.parse::<u64>().ok()
                };

                if let Some(val) = value {
                    let (_, config) = network_select!(self, variant);
                    config.bandwidth = val;
                }
                self.save_config();
            }

            Message::Tick => {
                self.refresh_stats();
            }

            Message::SlowTimer => {
                if self.is_laptop {
                    let current_on_ac = self.is_on_ac().unwrap_or(true);
                    if self.on_ac != current_on_ac {
                        self.on_ac = current_on_ac;

                        for (id, gpu) in &mut self.gpus {
                            if let Some(c) = self.config.gpus.get(id)
                                && c.pause_on_battery
                            {
                                if current_on_ac {
                                    info!("Changed to AC, restart polling");
                                    gpu.restart(); // on AC, start polling
                                } else {
                                    info!("Changed to DC, stop polling");
                                    gpu.stop(); // on battery, stop polling
                                }
                            }
                        }
                    }
                }
            }

            Message::ToggleCpuChart(toggled) => {
                info!("Message::ToggleCpuChart({toggled:?})");
                self.config.cpu.show_chart(toggled);
                self.save_config();
            }

            Message::ToggleCpuTempChart(toggled) => {
                info!("Message::ToggleCpuTempChart({toggled:?})");
                self.config.cputemp.show_chart(toggled);
                self.save_config();
            }

            Message::ToggleCpuNoDecimals(toggle) => {
                info!("Message::ToggleCpuNoDecimals({toggle:?})");
                self.config.cpu.no_decimals = toggle;
                self.save_config();
            }

            Message::SelectCpuTempUnit(unit) => {
                info!("Message::SelectCpuTempUnit({unit:?})");
                self.config.cputemp.unit = unit;
                self.save_config();
            }

            Message::CpuTempMinTempChanged(temp) => {
                info!("Message::CpuTempMinTempChanged({temp})");
                if temp >= 0.0 && temp < 100.0 {
                    self.config.cputemp.min_temp = temp;
                    self.save_config();
                }
            }

            Message::CpuBarSizeChanged(width) => {
                info!("Message::CpuBarSizeChanged({width})");
                self.config.cpu.bar_width = width;
                self.save_config();
            }

            Message::CpuNarrowBarSpacing(enable) => {
                if enable {
                    self.config.cpu.bar_spacing = 0;
                } else {
                    self.config.cpu.bar_spacing = 1;
                }
                self.save_config();
            }

            Message::ToggleMemoryChart(toggled) => {
                info!("Message::ToggleMemoryChart({toggled:?})");
                self.config.memory.show_chart(toggled);
                self.save_config();
            }

            Message::ToggleNetChart(variant, toggled) => {
                info!("Message::ToggleNetChart({toggled:?})");
                let (_, config) = network_select!(self, variant);
                config.show_chart(toggled);
                self.save_config();
            }

            Message::ToggleCpuLabel(toggled) => {
                info!("Message::ToggleCpuLabel({toggled:?})");
                self.config.cpu.show_label(toggled);
                self.save_config();
            }

            Message::ToggleCpuTempLabel(toggled) => {
                info!("Message::ToggleCpuTempLabel({toggled:?})");
                self.config.cputemp.show_label(toggled);
                self.save_config();
            }

            Message::ToggleMemoryLabel(toggled) => {
                info!("Message::ToggleMemoryLabel({toggled:?})");
                self.config.memory.show_label(toggled);
                self.save_config();
            }

            Message::ToggleMemoryPercentage(toggled) => {
                info!("Message::ToggleMemoryPercentage({toggled:?})");
                self.config.memory.percentage = toggled;
                self.save_config();
            }

            Message::ToggleMemoryAllocated(toggled) => {
                info!("Message::ToggleMemoryAllocated({toggled:?})");
                self.config.memory.show_allocated = toggled;
                self.save_config();
            }

            Message::ToggleNetLabel(variant, toggled) => {
                info!("Message::ToggleNetLabel({toggled:?})");
                let (_, config) = network_select!(self, variant);
                config.show_label(toggled);
                self.save_config();
            }

            Message::ConfigChanged(config) => {
                info!("Message::ConfigChanged()");
                self.config_changed(&config);
            }

            Message::ColorTextInputRedChanged(value) => {
                let mut col = self.colorpicker.sliders();
                Minimon::set_color(&value, &mut col.red);
                self.colorpicker.update_color(col);
            }

            Message::ColorTextInputGreenChanged(value) => {
                let mut col = self.colorpicker.sliders();
                Minimon::set_color(&value, &mut col.green);
                self.colorpicker.update_color(col);
            }

            Message::ColorTextInputBlueChanged(value) => {
                let mut col = self.colorpicker.sliders();
                Minimon::set_color(&value, &mut col.blue);
                self.colorpicker.update_color(col);
            }

            Message::ColorTextInputAlphaChanged(value) => {
                let mut col = self.colorpicker.sliders();
                Minimon::set_color(&value, &mut col.alpha);
                self.colorpicker.update_color(col);
            }

            Message::LaunchSystemMonitor(desktop_app) => {
                info!("Message::LaunchSystemMonitor() {}", desktop_app.name);
                system_monitors::launch_desktop_app(desktop_app);
            }

            Message::RefreshRateChanged(rate) => {
                info!("Message::RefreshRateChanged({rate:?})");
                self.config.refresh_rate = (rate * 1000.0) as u32;
                self.save_config();
            }

            Message::LabelSizeChanged(size) => {
                info!("Message::LabelSizeChanged({size:?})");
                self.config.label_size_default = size;
                self.save_config();
            }

            Message::ToggleMonospaceLabels(toggle) => {
                info!("Message::Monospacelabels({toggle:?})");
                self.config.monospace_labels = toggle;
                self.save_config();
            }

            Message::PanelSpacing(spacing) => {
                info!("Message::PanelSpacing({spacing})");
                self.config.panel_spacing = spacing;
                self.save_config();
            }

            Message::ToggleSymbols(toggle) => {
                info!("Message::ToggleSymbols({toggle:?})");
                self.config.symbols = toggle;
                self.save_config();
            }

            Message::Settings(setting) => {
                info!("Message::Settings({setting:?})");
                self.settings_page = setting;
            }
            Message::SysmonSelect(idx) => {
                let name: Option<String> = SYSMON_NAMES.get(idx).map(|s| s.to_string());
                info!("Message::SysmonSelect({idx})->{name:?}");
                self.config.sysmon = name;
                self.save_config();
            }
            Message::GpuToggleChart(id, device, toggled) => {
                self.update_gpu_config(
                    &id,
                    "GpuToggleChart",
                    device,
                    |config, device| match device {
                        DeviceKind::Gpu => config.usage.show_chart(toggled),
                        DeviceKind::Vram => config.vram.show_chart(toggled),
                        DeviceKind::GpuTemp => config.temp.show_chart(toggled),
                        _ => error!("GpuToggleChart: wrong kind {device:?}"),
                    },
                );
            }

            Message::GpuToggleLabel(id, device, toggled) => {
                self.update_gpu_config(
                    &id,
                    "GpuToggleLabel",
                    device,
                    |config, device| match device {
                        DeviceKind::Gpu => config.usage.show_label(toggled),
                        DeviceKind::Vram => config.vram.show_label(toggled),
                        DeviceKind::GpuTemp => config.temp.show_label(toggled),
                        _ => error!("GpuToggleLabel: wrong kind {device:?}"),
                    },
                );
            }

            Message::SelectGpuTempUnit(id, unit) => {
                info!("Message::SelectCpuTempUnit({unit:?})");
                if let Some(c) = self.config.gpus.get_mut(&id) {
                    c.temp.unit = unit;
                    self.save_config();
                } else {
                    error!("GpuToggleStackLabels: wrong id {id:?}");
                }
                self.save_config();
            }

            Message::GpuTempMinTempChanged(id, temp) => {
                info!("Message::GpuTempMinTempChanged({id:?}, {temp})");
                if temp >= 0.0 && temp < 100.0 {
                    if let Some(c) = self.config.gpus.get_mut(&id) {
                        c.temp.min_temp = temp;
                        self.save_config();
                    } else {
                        error!("GpuTempMinTempChanged: wrong id {id:?}");
                    }
                }
            }

            Message::GpuToggleStackLabels(id, toggled) => {
                info!("Message::GpuToggleStackLabels({id:?}, {toggled:?})");
                if let Some(c) = self.config.gpus.get_mut(&id) {
                    c.stack_labels = toggled;
                    self.save_config();
                } else {
                    error!("GpuToggleStackLabels: wrong id {id:?}");
                }
            }

            Message::GpuSelectGraphType(id, device, kind) => {
                info!("Message::GpuSelectGraphType({id:?}, {device:?}, {kind:?})");
                self.update_gpu_config(&id, "GpuSelectGraphType", device, |config, device| {
                    match device {
                        DeviceKind::Gpu => config.usage.chart = kind,
                        DeviceKind::Vram => config.vram.chart = kind,
                        DeviceKind::GpuTemp => config.temp.chart = kind,
                        _ => error!("GpuSelectGraphType: wrong kind {device:?}"),
                    }
                });
                if let Some(gpu) = self.gpus.get_mut(&id) {
                    match device {
                        DeviceKind::Gpu => gpu.gpu.set_graph_kind(kind),
                        DeviceKind::Vram => gpu.vram.set_graph_kind(kind),
                        DeviceKind::GpuTemp => gpu.temp.set_graph_kind(kind),
                        _ => error!("GpuSelectGraphType: wrong kind {device:?}"),
                    }
                }
            }
            Message::ToggleDisableOnBattery(id, toggled) => {
                info!("Message::ToggleDisableOnBattery({id:?}, {toggled:?})");
                if let Some(c) = self.config.gpus.get_mut(&id) {
                    c.pause_on_battery = toggled;
                    self.save_config();
                } else {
                    error!("ToggleDisableOnBattery: wrong id {id:?}");
                }
            }
            Message::ChangeContentOrder(order_change) => {
                if order_change.new_index == order_change.current_index
                    || order_change.new_index >= self.config.content_order.order.len()
                {
                    return Task::none();
                }

                self.config
                    .content_order
                    .order
                    .swap(order_change.current_index, order_change.new_index);
                self.save_config();
            }
            Message::Tip => {
                Self::open_tipping_page_in_browser();
            }
        }
        Task::none()
    }
}

impl Minimon {
    fn config_changed(&mut self, config: &MinimonConfig) {
        self.config = config.clone();
        let rr = self.config.refresh_rate;
        self.refresh_rate.store(rr, atomic::Ordering::Relaxed);
        self.cpu.update_config(&config.cpu, rr);
        self.cputemp.update_config(&config.cputemp, rr);
        self.memory.update_config(&config.memory, rr);
        self.network1.update_config(&config.network1, rr);
        self.network2.update_config(&config.network2, rr);
        self.disks1.update_config(&config.disks1, rr);
        self.disks2.update_config(&config.disks2, rr);
        self.sync_gpu_configs();

        // Track whether anything is visible on the panel, or just the app-icon
        {
            self.data_is_visible = false;
            for gpu in self.gpus.values() {
                if let Some(g) = self.config.gpus.get(&gpu.id())
                    && g.is_visible()
                {
                    self.data_is_visible = true;
                    break;
                }
            }

            if self.config.cpu.visible()
                || self.config.cputemp.visible()
                || self.config.memory.visible()
                || self.config.network1.visible()
                || (self.config.network1.variant != NetworkVariant::Combined
                    && self.config.network2.visible())
                || self.config.disks1.visible()
                || (self.config.disks1.variant != DisksVariant::Combined
                    && self.config.disks2.visible())
            {
                self.data_is_visible = true;
            }
        }
        self.calculate_max_label_widths();
    }

    pub fn sub_page_header<'a, Message: 'static + Clone>(
        sub_page: Option<&'a str>,
        parent_page: &'a str,
        on_press: Message,
    ) -> Element<'a, Message> {
        let previous_button = widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
            .extra_small()
            .padding(0)
            .label(parent_page)
            .spacing(4)
            .class(widget::button::ButtonClass::Link)
            .on_press(on_press);

        if let Some(p) = sub_page {
            let sub_page_header = widget::row::with_capacity(2).push(text::title3(p));

            widget::column::with_capacity(2)
                .push(previous_button)
                .push(sub_page_header)
                .spacing(6)
                .width(iced::Length::Shrink)
                .into()
        } else {
            widget::column::with_capacity(2)
                .push(previous_button)
                .spacing(6)
                .width(iced::Length::Shrink)
                .into()
        }
    }

    pub fn go_next_with_item<'a, Msg: Clone + 'static>(
        description: &'a str,
        item: impl Into<cosmic::Element<'a, Msg>>,
        msg_opt: impl Into<Option<Msg>> + Clone,
    ) -> cosmic::Element<'a, Msg> {
        settings::item_row(vec![
            widget::text::body(description)
                .wrapping(iced::core::text::Wrapping::Word)
                .into(),
            widget::horizontal_space().into(),
            widget::row::with_capacity(2)
                .push(item)
                .push(widget::icon::from_name("go-next-symbolic").size(16).icon())
                .align_y(Alignment::Center)
                .spacing(cosmic::theme::spacing().space_s)
                .into(),
        ])
        .apply(widget::container)
        .class(cosmic::theme::Container::List)
        .apply(widget::button::custom)
        .padding(0)
        .class(cosmic::theme::Button::Transparent)
        .on_press_maybe(msg_opt.into())
        .into()
    }

    fn general_settings_ui(&'_ self) -> Element<'_, crate::app::Message> {
        let refresh_rate = f64::from(self.config.refresh_rate) / 1000.0;

        let heart = widget::button::custom(Element::from(row!(
            widget::text(fl!("tip")),
            widget::svg(widget::svg::Handle::from_memory(HEART.as_bytes()))
                .width(15)
                .height(15)
        )));
        let version_row = row!(
            text::heading(format!(
                "Minimon version {} for COSMIC.",
                env!("CARGO_PKG_VERSION")
            )),
            horizontal_space(),
            heart.on_press(Message::Tip)
        );
        // Create settings rows
        let refresh_row = settings::item(
            fl!("refresh-rate"),
            spin_button(
                format!("{refresh_rate:.2}"),
                refresh_rate,
                0.250,
                0.250,
                15.00,
                Message::RefreshRateChanged,
            ),
        );

        let label_row = settings::item(
            fl!("change-label-size"),
            spin_button(
                self.config.label_size_default.to_string(),
                self.config.label_size_default,
                1,
                5,
                20,
                Message::LabelSizeChanged,
            ),
        );

        let mono_row = settings::item(
            fl!("settings-monospace_font"),
            row!(
                widget::checkbox("", self.config.monospace_labels)
                    .on_toggle(Message::ToggleMonospaceLabels)
            ),
        );

        let symbol_row = settings::item(
            fl!("enable-symbols"),
            widget::toggler(self.config.symbols).on_toggle(Message::ToggleSymbols),
        );

        let spacing_row = settings::item(
            fl!("settings-panel-spacing"),
            widget::row::with_children(vec![
                text::body(fl!("settings-small")).into(),
                widget::slider(1..=6, self.config.panel_spacing, Message::PanelSpacing)
                    .width(100)
                    .into(),
                text::body(fl!("settings-large")).into(),
            ])
            .align_y(Alignment::Center)
            .spacing(8),
        );

        let idx = self
            .config
            .sysmon
            .as_ref()
            .and_then(|n| SYSMON_NAMES.iter().position(|&app_name| app_name == n));

        let sysmon_row = settings::item(
            fl!("choose-sysmon"),
            row!(widget::dropdown(&*SYSMON_NAMES, idx, Message::SysmonSelect).width(220)),
        );

        let content_items = Column::from_vec({
            let mut children = Vec::new();

            for (index, content) in self.config.content_order.order.iter().enumerate() {
                let item = match content {
                    ContentType::CpuUsage => text(fl!("settings-cpu")),
                    ContentType::CpuTemp => {
                        if !self.cputemp.is_found() {
                            continue;
                        }
                        text(fl!("settings-cpu-temperature"))
                    }
                    ContentType::MemoryUsage => text(fl!("settings-memory")),
                    ContentType::NetworkUsage => text(fl!("settings-network")),
                    ContentType::DiskUsage => text(fl!("settings-disks")),
                    ContentType::GpuInfo => {
                        if self.gpus.is_empty() {
                            continue;
                        }
                        text(fl!("settings-gpu"))
                    }
                };

                let item_row = row!(
                    row!(
                        button::icon(widget::icon::from_name("pan-up-symbolic").size(5)).on_press(
                            Message::ChangeContentOrder(ContentOrderChange {
                                current_index: index,
                                new_index: index.saturating_sub(1)
                            })
                        ),
                        button::icon(widget::icon::from_name("pan-down-symbolic").size(5))
                            .on_press(Message::ChangeContentOrder(ContentOrderChange {
                                current_index: index,
                                new_index: index.saturating_add(1)
                            })),
                    ),
                    item
                )
                .spacing(cosmic::theme::spacing().space_xxs)
                .align_y(Alignment::Center);

                children.push(item_row.into())
            }

            children
        })
        .spacing(cosmic::theme::spacing().space_s);

        let content_order = row!(
            text(fl!("content-order")),
            horizontal_space(),
            content_items
        );

        // Combine rows into a column with spacing
        column!(
            version_row,
            refresh_row,
            label_row,
            mono_row,
            symbol_row,
            spacing_row,
            sysmon_row,
            content_order
        )
        .spacing(10)
        .into()
    }

    fn push_symbolic_icon(
        &self,
        elements: &mut VecDeque<Element<crate::app::Message>>,
        icon_name: &str,
        at_start: bool,
    ) {
        let size = self.core.applet.suggested_size(true);
        let icon = widget::icon::from_name(icon_name)
            .symbolic(true)
            .size(size.1)
            .into();

        if at_start {
            elements.push_front(icon);
        } else {
            elements.push_back(icon);
        }
    }

    fn cpu_panel_ui(&'_ self, horizontal: bool) -> VecDeque<Element<'_, crate::app::Message>> {
        let size = self.core.applet.suggested_size(false);

        let mut elements: VecDeque<Element<Message>> = VecDeque::new();

        // Handle the symbols button if needed
        if self.config.symbols
            && (self.config.cpu.label_visible() || self.config.cpu.chart_visible())
        {
            self.push_symbolic_icon(&mut elements, CPU_ICON, false);
        }

        let cpu_usage = self.cpu.latest_sample();
        // Format CPU usage based on horizontal layout and sample value
        let formatted_cpu = if self.config.cpu.no_decimals {
            format!("{}%", cpu_usage.round())
        } else if cpu_usage < 10.0 && horizontal {
            format!("{:.2}%", (cpu_usage * 100.0).trunc() / 100.0)
        } else {
            format!("{:.1}%", (cpu_usage * 10.0).trunc() / 10.0)
        };

        // Add the CPU label if needed
        if self.config.cpu.label_visible() {
            elements.push_back(
                self.figure_label(formatted_cpu, self.label_cpu_width)
                    .into(),
            );
        }

        let width: u16 = if self.config.cpu.chart == ChartKind::StackedBars {
            StackedBarSvg::new(
                self.config.cpu.bar_width,
                size.0,
                self.config.cpu.bar_spacing,
            )
            .width(self.cpu.core_count())
        } else {
            size.1
        };

        if self.config.cpu.chart_visible() {
            elements.push_back(
                self.cpu
                    .chart(size.0, width)
                    .height(size.0)
                    .width(width)
                    .into(),
            );
        }

        elements
    }

    fn cpu_temp_panel_ui(
        &'_ self,
        _horizontal: bool,
    ) -> VecDeque<Element<'_, crate::app::Message>> {
        let size = self.core.applet.suggested_size(false);

        let mut elements: VecDeque<Element<Message>> = VecDeque::new();

        if self.cputemp.is_found() {
            // Handle the symbols button if needed
            if self.config.symbols
                && (self.config.cputemp.label_visible() || self.config.cputemp.chart_visible())
            {
                self.push_symbolic_icon(&mut elements, TEMP_ICON, false);
            }

            // Add the CPU label if needed
            if self.config.cputemp.label_visible() {
                elements.push_back(self.figure_label(self.cputemp.to_string(), None).into());
            }

            // Add the CPU chart if needed
            if self.config.cputemp.chart_visible() {
                elements.push_back(
                    self.cputemp
                        .chart(size.0, size.1)
                        .height(size.0)
                        .width(size.1)
                        .into(),
                );
            }
        }

        elements
    }

    fn memory_panel_ui(&'_ self, horizontal: bool) -> VecDeque<Element<'_, crate::app::Message>> {
        let size = self.core.applet.suggested_size(false);

        let mut elements: VecDeque<Element<Message>> = VecDeque::new();

        // Handle the symbols button if needed
        if self.config.symbols
            && (self.config.memory.label_visible() || self.config.memory.chart_visible())
        {
            self.push_symbolic_icon(&mut elements, RAM_ICON, false);
        }

        // Label section
        if self.config.memory.label_visible() {
            let formatted_mem = self.memory.to_string(!horizontal);
            elements.push_back(self.figure_label(formatted_mem, None).into());
        }

        // Chart section
        if self.config.memory.chart_visible() {
            elements.push_back(
                self.memory
                    .chart(size.0, size.1)
                    .height(size.0)
                    .width(size.1)
                    .into(),
            );
        }

        elements
    }

    fn network_panel_ui(&'_ self, horizontal: bool) -> VecDeque<Element<'_, crate::app::Message>> {
        let size = self.core.applet.suggested_size(false);

        let nw_combined = self.config.network1.variant == NetworkVariant::Combined;
        let sample_rate_ms = self.config.refresh_rate;
        let mut elements: VecDeque<Element<Message>> = VecDeque::new();

        let format_label = |text: String| self.figure_label(text, self.label_network_width);

        let unit_len = if horizontal {
            network::UnitVariant::Long
        } else {
            network::UnitVariant::Short
        };

        if self.config.network1.label_visible() {
            let mut network_labels = Vec::new();
            let mut dl_row = Vec::new();

            if horizontal {
                dl_row.push(self.figure_label("↓".to_owned(), None).into());
            }
            dl_row
                .push(format_label(self.network1.download_label(sample_rate_ms, unit_len)).into());

            if nw_combined {
                network_labels.push(widget::vertical_space().into());
            }

            network_labels.push(Row::from_vec(dl_row).into());

            if nw_combined {
                let mut ul_row = Vec::new();

                if horizontal {
                    ul_row.push(self.figure_label("↑".to_owned(), None).into());
                }
                ul_row.push(
                    format_label(self.network1.upload_label(sample_rate_ms, unit_len)).into(),
                );

                network_labels.push(Row::from_vec(ul_row).into());
                network_labels.push(widget::vertical_space().into());
            }

            elements.push_back(Column::from_vec(network_labels).into());
        }

        if self.config.network1.chart_visible() {
            elements.push_back(
                self.network1
                    .chart(size.0, size.1)
                    .height(size.0)
                    .width(size.1)
                    .into(),
            );
        }

        if self.config.network2.label_visible() && !nw_combined {
            let mut network_labels = Vec::new();

            let mut ul_row = Vec::new();

            if horizontal {
                ul_row.push(self.figure_label("↑".to_owned(), None).into());
            }
            ul_row.push(format_label(self.network2.upload_label(sample_rate_ms, unit_len)).into());

            network_labels.push(Row::from_vec(ul_row).into());

            elements.push_back(Column::from_vec(network_labels).into());
        }

        if self.config.network2.chart_visible() && !nw_combined {
            elements.push_back(
                self.network2
                    .chart(size.0, size.1)
                    .height(size.0)
                    .width(size.1)
                    .into(),
            );
        }

        if self.config.symbols && !elements.is_empty() {
            self.push_symbolic_icon(&mut elements, NETWORK_ICON, true);
        }

        elements
    }

    fn disks_panel_ui(&'_ self, horizontal: bool) -> VecDeque<Element<'_, crate::app::Message>> {
        let size = self.core.applet.suggested_size(false);

        let disks_combined = self.config.disks1.variant == DisksVariant::Combined;
        let sample_rate_ms = self.config.refresh_rate;
        let mut elements: VecDeque<Element<Message>> = VecDeque::new();

        let format_label = |text: String| self.figure_label(text, self.label_disks_width);

        let unit_len = if horizontal {
            disks::UnitVariant::Long
        } else {
            disks::UnitVariant::Short
        };

        if self.config.disks1.label_visible() {
            let mut disks_labels = Vec::new();

            let mut wr_row = Vec::new();
            if horizontal {
                wr_row.push(self.figure_label("w".to_owned(), self.label_w_width).into());
            }
            wr_row.push(format_label(self.disks1.write_label(sample_rate_ms, unit_len)).into());

            if disks_combined {
                disks_labels.push(widget::vertical_space().into());
            }

            disks_labels.push(Row::from_vec(wr_row).spacing(0).padding(0).into());

            if disks_combined {
                let mut rd_row = Vec::new();
                if horizontal {
                    rd_row.push(self.figure_label("r".to_owned(), self.label_w_width).into());
                }
                rd_row.push(format_label(self.disks1.read_label(sample_rate_ms, unit_len)).into());

                disks_labels.push(Row::from_vec(rd_row).spacing(0).padding(0).into());
                disks_labels.push(widget::vertical_space().into());
            }

            elements.push_back(Column::from_vec(disks_labels).into());
        }

        if self.config.disks1.chart_visible() {
            elements.push_back(
                self.disks1
                    .chart(size.0, size.1)
                    .height(size.0)
                    .width(size.1)
                    .into(),
            );
        }

        if self.config.disks2.label_visible() && !disks_combined {
            let mut disks_labels = Vec::new();

            let mut rd_row = Vec::new();
            if horizontal {
                rd_row.push(self.figure_label("r".to_owned(), self.label_w_width).into());
            }
            rd_row.push(format_label(self.disks2.read_label(sample_rate_ms, unit_len)).into());
            disks_labels.push(Row::from_vec(rd_row).spacing(0).padding(0).into());

            elements.push_back(Column::from_vec(disks_labels).into());
        }

        if self.config.disks2.chart_visible() && !disks_combined {
            elements.push_back(
                self.disks2
                    .chart(size.0, size.1)
                    .height(size.0)
                    .width(size.1)
                    .into(),
            );
        }

        if self.config.symbols && !elements.is_empty() {
            self.push_symbolic_icon(&mut elements, DISK_ICON, true);
        }

        elements
    }

    fn gpu_panel_ui<'a>(
        &'a self,
        gpu: &'a Gpu,
        horizontal: bool,
    ) -> VecDeque<Element<'a, crate::app::Message>> {
        let size = self.core.applet.suggested_size(false);

        let mut elements: VecDeque<Element<Message>> = VecDeque::new();

        if let Some(config) = self.config.gpus.get(&gpu.id()) {
            let formatted_gpu = gpu.gpu.to_string();
            let formatted_vram = gpu.vram.string(!horizontal);
            let stacked_labels =
                config.stack_labels && config.usage.label_visible() && config.vram.label_visible();

            if stacked_labels {
                let gpu_labels = vec![
                    widget::vertical_space().into(),
                    self.figure_label(formatted_gpu, self.label_gpu_width)
                        .into(),
                    self.figure_label(formatted_vram.clone(), None).into(),
                    widget::vertical_space().into(),
                ];
                elements.push_back(Column::from_vec(gpu_labels).into());
            } else if config.usage.label_visible() {
                elements.push_back(
                    self.figure_label(formatted_gpu, self.label_gpu_width)
                        .into(),
                );
            }

            if config.usage.chart_visible() {
                elements.push_back(gpu.gpu.chart().height(size.0).width(size.1).into());
            }
            if config.temp.label_visible() {
                elements.push_back(self.figure_label(gpu.temp.to_string(), None).into());
            }

            if config.temp.chart_visible() {
                elements.push_back(gpu.temp.chart().height(size.0).width(size.1).into());
            }

            if config.vram.label_visible() && !stacked_labels {
                elements.push_back(self.figure_label(formatted_vram, None).into());
            }

            if config.vram.chart_visible() {
                elements.push_back(gpu.vram.chart().height(size.0).width(size.1).into());
            }
        }

        if self.config.symbols && !elements.is_empty() {
            self.push_symbolic_icon(&mut elements, GPU_ICON, true);
        }

        elements
    }

    /// Set to 0 if empty, value if valid, but leave unchanged in value is not valid
    fn set_color(value: &str, color: &mut u8) {
        if value.is_empty() {
            *color = 0;
        } else if let Ok(num) = value.parse::<u8>() {
            *color = num;
        }
    }

    fn save_config(&self) {
        info!("save_config()");
        if let Ok(helper) = cosmic::cosmic_config::Config::new(
            match self.core.applet.panel_type {
                PanelType::Panel => APP_ID_PANEL,
                PanelType::Dock => APP_ID_DOCK,
                PanelType::Other(_) => APP_ID_OTHER,
            },
            MinimonConfig::VERSION,
        ) && let Err(err) = self.config.write_entry(&helper)
        {
            info!("Error writing config {err}");
        }
    }

    fn save_colors(&mut self, colors: &ChartColors, kind: DeviceKind, id: Option<String>) {
        match kind {
            DeviceKind::Cpu => {
                *self.config.cpu.colors_mut() = *colors;
            }
            DeviceKind::CpuTemp => {
                *self.config.cputemp.colors_mut() = *colors;
            }
            DeviceKind::Memory => {
                *self.config.memory.colors_mut() = *colors;
            }
            DeviceKind::Network(variant) => {
                let (_, config) = network_select!(self, variant);
                *config.colors_mut() = *colors;
            }
            DeviceKind::Disks(variant) => {
                let (_, config) = disks_select!(self, variant);
                *config.colors_mut() = *colors;
            }
            DeviceKind::Gpu => {
                if let Some(id) = id {
                    if let Some(config) = self.config.gpus.get_mut(&id) {
                        *config.usage.colors_mut() = *colors;
                    } else {
                        error!("No config for selected GPU {id}");
                    }
                }
            }
            DeviceKind::Vram => {
                if let Some(id) = id {
                    if let Some(config) = self.config.gpus.get_mut(&id) {
                        *config.vram.colors_mut() = *colors;
                    } else {
                        error!("No config for selected GPU {id}");
                    }
                }
            }
            DeviceKind::GpuTemp => {
                if let Some(id) = id {
                    if let Some(config) = self.config.gpus.get_mut(&id) {
                        *config.temp.colors_mut() = *colors;
                    } else {
                        error!("No config for selected GPU {id}");
                    }
                }
            }
        }
    }

    fn refresh_stats(&mut self) {
        // Update everything if popup open
        let all = self.popup.is_some();

        if all || self.config.cpu.visible() {
            self.cpu.update();
        }

        if all || self.config.cputemp.visible() {
            self.cputemp.update();
        }

        if all || self.config.memory.visible() {
            self.memory.update();
        }

        let combined_network = self.config.network1.variant == NetworkVariant::Combined;
        if all
            || (combined_network && self.config.network1.visible())
            || (!combined_network
                && (self.config.network1.visible() || self.config.network1.visible()))
        {
            self.network1.update();
            self.network2.update();
        }

        let combined_disks = self.config.disks1.variant == DisksVariant::Combined;

        if all
            || (combined_disks && self.config.disks1.visible())
            || (!combined_disks && (self.config.disks1.visible() || self.config.disks2.visible()))
        {
            self.disks1.update();
            self.disks2.update();
        }

        for gpu in &mut self.gpus.values_mut() {
            if let Some(g) = self.config.gpus.get(&gpu.id())
                && (all || g.is_visible())
            {
                if all && !gpu.is_active() {
                    gpu.restart();
                }
                gpu.update();
            }
        }
    }

    fn maybe_stop_gpus(&mut self) {
        if self.is_laptop && !self.on_ac {
            for (id, gpu) in &mut self.gpus {
                if let Some(c) = self.config.gpus.get(id)
                    && c.pause_on_battery
                {
                    info!("Changed to DC, stop polling");
                    gpu.stop(); // on battery, stop polling
                }
            }
        }
    }

    fn label_font_size(&self) -> u16 {
        match self.core.applet.size {
            Size::PanelSize(PanelSize::XL) => self.config.label_size_default + 5,
            Size::PanelSize(PanelSize::L) => self.config.label_size_default + 3,
            Size::PanelSize(PanelSize::M) => self.config.label_size_default + 2,
            Size::PanelSize(PanelSize::S) => self.config.label_size_default + 1,
            Size::PanelSize(PanelSize::XS) => self.config.label_size_default,
            _ => self.config.label_size_default,
        }
    }

    fn figure_label<'a>(
        &self,
        text: String,
        width: Option<f32>,
    ) -> widget::Text<'a, cosmic::Theme> {
        let size = self.label_font_size();

        if self.config.monospace_labels {
            widget::text(text).size(size).font(cosmic::font::mono()) // .font(cosmic::font::Font::with_name("Noto Mono"))
        } else if let Some(w) = width {
            widget::text(text)
                .size(size)
                .width(w)
                .wrapping(iced::core::text::Wrapping::None)
                .align_x(Horizontal::Center)
        } else {
            widget::text(text)
                .size(size)
                .wrapping(iced::core::text::Wrapping::None)
        }
    }

    // make sure our liust of detected GPUs and the config list are equal
    fn sync_gpu_configs(&mut self) {
        let config_gpus = &mut self.config.gpus;

        // Remove entries not present in detected GPUs
        config_gpus.retain(|id, _| self.gpus.contains_key(id));

        // Add missing GPU configs
        for id in self.gpus.keys() {
            config_gpus.entry(id.clone()).or_default();
        }

        for (id, gpu) in &mut self.gpus {
            if let Some(config) = config_gpus.get(id) {
                gpu.update_config(config, self.config.refresh_rate);
            }
        }
    }

    fn update_gpu_config<F>(&mut self, id: &str, action: &str, device: DeviceKind, update_fn: F)
    where
        F: FnOnce(&mut GpuConfig, DeviceKind),
    {
        info!("{action}({:?})", (id.to_string(), &device));
        if let Some(config) = self.config.gpus.get_mut(id) {
            update_fn(config, device);
            self.save_config();
        } else {
            error!("{action}: no config for selected GPU {id}");
        }
    }

    fn has_gpus(&self) -> bool {
        !self.gpus.is_empty()
    }

    fn is_on_ac(&self) -> Result<bool, Box<dyn std::error::Error>> {
        if self.is_laptop {
            // Connect to the system bus
            let connection = Connection::system()?;

            // Create a proxy to UPower service
            let proxy = zbus::blocking::Proxy::new(
                &connection,
                "org.freedesktop.UPower",
                "/org/freedesktop/UPower",
                "org.freedesktop.UPower",
            )?;

            // Get the list of power-related devices
            let devices: Vec<OwnedObjectPath> = proxy.call("EnumerateDevices", &())?;

            for device_path in devices {
                let device_proxy = zbus::blocking::Proxy::new(
                    &connection,
                    "org.freedesktop.UPower",
                    device_path.as_str(),
                    "org.freedesktop.UPower.Device",
                )?;

                // Get the Type property (1 = line power / AC)
                let kind: u32 = device_proxy.get_property("Type")?;
                if kind == 1 {
                    // Get the Online property
                    let online: bool = device_proxy.get_property("Online")?;
                    return Ok(online);
                }
            }
        }

        Ok(true)
    }

    fn is_laptop() -> bool {
        let power_supply_path = "/sys/class/power_supply";
        match fs::read_dir(power_supply_path) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().starts_with("BAT")),
            Err(e) => {
                info!("Could not read power supply info: {e}");
                false
            }
        }
    }

    fn measure_text_width(&mut self, text: &str, attrs: &Attrs) -> Option<f32> {
        let font_size = self.label_font_size();

        let metrics = Metrics::new(font_size.into(), font_size.into());
        // Create a buffer to shape the text
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced, None);

        // Get the width of the first layout line
        buffer
            .lines
            .first()
            .and_then(|line| line.layout_opt())
            .and_then(|layouts| layouts.first().map(|layout| layout.w.ceil() + 2.0))
    }

    fn calculate_max_label_widths(&mut self) {
        // Yes there are two different Family types
        // font::default() returns one and Attrs takes another
        use cosmic::iced::font::{Family as IcedFamily, Style as IcedStyle, Weight as IcedWeight};
        use iced::advanced::graphics::text::cosmic_text::{
            Family as CosmicTextFamily, Style as TextStyle, Weight as TextWeight,
        };

        if let Some(font) = self.interface_font.clone().map(Into::<iced::Font>::into) {
            let family = match font.family {
                IcedFamily::Monospace => CosmicTextFamily::Monospace,
                IcedFamily::Serif => CosmicTextFamily::Serif,
                IcedFamily::SansSerif => CosmicTextFamily::SansSerif,
                IcedFamily::Name(name) => CosmicTextFamily::Name(name),
                IcedFamily::Cursive => CosmicTextFamily::Cursive,
                IcedFamily::Fantasy => CosmicTextFamily::Fantasy,
            };

            let weight = match font.weight {
                IcedWeight::Thin => TextWeight::THIN,
                IcedWeight::ExtraLight => TextWeight::EXTRA_LIGHT,
                IcedWeight::Light => TextWeight::LIGHT,
                IcedWeight::Normal => TextWeight::NORMAL,
                IcedWeight::Medium => TextWeight::MEDIUM,
                IcedWeight::Bold => TextWeight::BOLD,
                IcedWeight::ExtraBold => TextWeight::EXTRA_BOLD,
                IcedWeight::Black => TextWeight::BLACK,
                IcedWeight::Semibold => TextWeight::SEMIBOLD,
            };

            let style = match font.style {
                IcedStyle::Normal => TextStyle::Normal,
                IcedStyle::Italic => TextStyle::Italic,
                IcedStyle::Oblique => TextStyle::Oblique,
            };

            let attrs = Attrs::new().family(family).weight(weight).style(style);

            let is_horizontal = self.core.applet.is_horizontal();

            self.label_cpu_width = self.measure_text_width("8.88%", &attrs);
            self.label_gpu_width = self.label_cpu_width;

            self.label_network_width = match (self.config.network1.show_bytes, is_horizontal) {
                (false, false) => self.measure_text_width("8.88M", &attrs),
                (false, true) => self.measure_text_width("8.88 Mbps", &attrs),
                (true, false) => self.measure_text_width("8.88M", &attrs),
                (true, true) => self.measure_text_width("8.88 MB/s", &attrs),
            };

            self.label_disks_width = if is_horizontal {
                self.measure_text_width("8.88 MB/s", &attrs)
            } else {
                self.measure_text_width("8.88M", &attrs)
            };

            self.label_w_width = self.measure_text_width("W ", &attrs);
        }
    }

    fn open_tipping_page_in_browser() {
        let url = "https://ko-fi.com/hyperchaotic";
        let in_flatpak = std::env::var("FLATPAK_ID").is_ok();

        let result = if in_flatpak {
            // Use flatpak-spawn to run xdg-open on the host
            std::process::Command::new("flatpak-spawn")
                .args(["--host", "xdg-open", url])
                .spawn()
        } else {
            // Native: directly call xdg-open
            std::process::Command::new("xdg-open").arg(url).spawn()
        };

        if let Err(e) = result {
            error!("Failed to launch browser: {e:?}");
        }
    }
}

const HEART: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" fill="none" stroke="red" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" class="icon icon-tabler icons-tabler-outline icon-tabler-heart">
  <path stroke="none" d="M0 0h24v24H0z"/>
  <path d="m20.288 12.653-8.28 8.269-8.278-8.27a5.52 5.566 0 1 1 8.279-7.308 5.52 5.566 0 1 1 8.279 7.315" style="stroke-width:2.21706"/>
</svg>"#;
