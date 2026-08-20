use cosmic::applet::cosmic_panel_config::PanelSize;
use cosmic::applet::{PanelType, Size};
use cosmic::config::FontConfig;
use cosmic::cosmic_config::CosmicConfigEntry;
use cosmic::cosmic_theme::palette::bool_mask::BoolMask;
use cosmic::cosmic_theme::palette::{FromColor, WithAlpha};
use cosmic::iced::advanced::graphics::text::cosmic_text::{Buffer, FontSystem, Metrics, Shaping};
use cosmic::iced::alignment::Horizontal::{self};
use cosmic::iced::program::graphics::text::cosmic_text::Attrs;

use std::collections::{BTreeMap, VecDeque};
use std::{fs, time};

use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::{self, Subscription};
use cosmic::iced::{Limits, Padding};
use cosmic::widget::about::About;
use cosmic::widget::segmented_button;
use cosmic::widget::{Column, Row, container, settings, spin_button, text};
use cosmic::{Apply, Element};
use cosmic::{widget, widget::autosize};

use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{self, AtomicU32};

use cosmic::{applet::cosmic_panel_config::PanelAnchor, iced::Alignment};

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
use crate::sensors::gpu::GpuType;
use crate::sensors::gpus::{Gpu, Gpus};
use crate::sensors::memory::Memory;
use crate::sensors::network::{self, Network};
use crate::sensors::{Sensor, TempUnit};
use crate::system_monitors;
use crate::ui;
use crate::{config::MinimonConfig, fl};

use cosmic::widget::Id as WId;

const NVIDIA_REDETECT_ATTEMPTS: u8 = 5;

static AUTOSIZE_MAIN_ID: LazyLock<WId> = std::sync::LazyLock::new(|| WId::new("autosize-main"));

const ICON: &str = "io.github.cosmic_utils.minimon-applet";
const CPU_ICON: &str = "io.github.cosmic_utils.minimon-applet-cpu";
const TEMP_ICON: &str = "io.github.cosmic_utils.minimon-applet-temperature";
const RAM_ICON: &str = "io.github.cosmic_utils.minimon-applet-ram";
const GPU_ICON: &str = "io.github.cosmic_utils.minimon-applet-gpu";
const NETWORK_ICON: &str = "io.github.cosmic_utils.minimon-applet-network";
const DISK_ICON: &str = "io.github.cosmic_utils.minimon-applet-harddisk";

const DEFAULT_MONITOR: &str = "COSMIC System Monitor";

const LICENSE: &str = "GPL-3.0-only";

const REPOSITORY_URL: &str = "https://github.com/cosmic-utils/minimon-applet";
const TIP_URL: &str = "https://ko-fi.com/hyperchaotic";
const LICENSE_URL: &str = "https://www.gnu.org/licenses/gpl-3.0.html";

pub static SETTINGS_CPU_CHOICE: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-cpu").leak());
pub static SETTINGS_MEMORY_CHOICE: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-memory").leak());
pub static SETTINGS_NETWORK_CHOICE: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-network").leak());
pub static SETTINGS_DISKS_CHOICE: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-disks").leak());
pub static SETTINGS_GPU_CHOICE: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-gpu").leak());
pub static SETTINGS_ABOUT_CHOICE: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-about").leak());

pub static SETTINGS_GENERAL_HEADING: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-subpage-general").leak());
pub static SETTINGS_BACK: LazyLock<&'static str> =
    LazyLock::new(|| fl!("settings-subpage-back").leak());

pub static SETTINGS_TIP: LazyLock<&'static str> = LazyLock::new(|| fl!("tip").leak());

pub static ABOUT_LINKS_MAIN: LazyLock<&'static str> = LazyLock::new(|| fl!("links-main").leak());
pub static ABOUT_LINKS_ISSUES: LazyLock<&'static str> =
    LazyLock::new(|| fl!("links-issues").leak());

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsVariant {
    General,
    Cpu,
    Memory,
    Network,
    Disks,
    Gpu(String),
    About,
}

/// One reading of a sensor that has more than one, selected through the tab bar
/// at the top of the sensor's settings page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    CpuLoad,
    CpuTemp,
    NetworkDownload,
    NetworkUpload,
    DisksWrite,
    DisksRead,
    GpuLoad,
    GpuTemp,
    GpuVram,
}

pub struct Minimon {
    /// Application state which is managed by the COSMIC runtime.
    core: Core,

    cpu: Cpu,
    cputemp: CpuTemp,
    memory: Memory,
    network1: Network,
    network2: Network,
    disks1: Disks,
    disks2: Disks,
    gpus: Gpus,

    /// As the Nvidia runtime may be slow to load we trach number of retries
    nvidia_redetect_attempts: u8,

    /// The popup id.
    popup: Option<Id>,

    /// Size of the panel window, used to centre the popup below the applet.
    panel_size: Option<iced::Size>,

    /// Current settings sub page
    settings_page: Option<SettingsVariant>,

    /// Readings of the current settings sub page, empty if it only has one
    settings_tabs: segmented_button::Model<segmented_button::SingleSelect>,

    /// Static information shown on the about page
    about: About,

    /// The color picker dialog
    colorpicker: ColorPicker,

    /// Settings stored on disk, including refresh rate, colors, etc.
    config: MinimonConfig,

    /// tick can be 250, 500 or 1000, depending on refresh rate modolu tick
    refresh_rate: Arc<AtomicU32>,

    // On AC or battery?
    is_laptop: bool,
    on_ac: bool,

    // Tracks whether any chart or value is showing on the panel
    data_is_visible: bool,

    // Used to measure value width, have to be cached because slow to load
    font_system: FontSystem,

    interface_font: Option<FontConfig>,

    // Pre-calc the max width of labels to avoid panel wobble
    value_cpu_width: Option<f32>,
    value_gpu_width: Option<f32>,
    value_network_width: Option<f32>,
    value_disks_width: Option<f32>,
    value_w_width: Option<f32>,
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
    ToggleNetValue(NetworkVariant, bool),
    ToggleNetLabel(NetworkVariant, bool),
    ToggleNetIcon(NetworkVariant, bool),
    ToggleAdaptiveNet(NetworkVariant, bool),
    NetworkSelectUnit(NetworkVariant, usize),
    TextInputBandwidthChanged(NetworkVariant, String),

    ToggleDisksCombined(bool),
    ToggleDisksChart(DisksVariant, bool),
    ToggleDisksValue(DisksVariant, bool),
    ToggleDisksLabel(DisksVariant, bool),
    ToggleDisksIcon(DisksVariant, bool),

    SelectGraphType(DeviceKind, ChartKind),
    Tick,
    SlowTimer,
    PopupClosed(Id),

    ToggleCpuChart(bool),
    ToggleCpuValue(bool),
    ToggleCpuLabel(bool),
    ToggleCpuIcon(bool),
    ToggleCpuTempChart(bool),
    ToggleCpuTempValue(bool),
    ToggleCpuTempLabel(bool),
    ToggleCpuTempIcon(bool),
    ToggleCpuNoDecimals(bool),
    CpuBarSizeChanged(u16),
    CpuNarrowBarSpacing(bool),
    ToggleMemoryChart(bool),
    ToggleMemoryValue(bool),
    ToggleMemoryLabel(bool),
    ToggleMemoryIcon(bool),
    ToggleMemoryPercentage(bool),
    ToggleMemoryAllocated(bool),
    ConfigChanged(Box<MinimonConfig>),
    ThemeChanged(Box<cosmic::config::CosmicTk>),
    LaunchSystemMonitor(&'static system_monitors::DesktopApp),
    RefreshRateChanged(f64),
    ValueSizeChanged(u16),
    ToggleMonospaceValues(bool),
    PanelSpacing(u16),
    SelectCpuTempUnit(TempUnit),
    CpuTempMinTempChanged(f64),

    Settings(Option<SettingsVariant>),
    SettingsTabSelected(segmented_button::Entity),
    LaunchWebbrowser(String),

    GpuToggleChart(String, DeviceKind, bool),
    GpuToggleValue(String, DeviceKind, bool),
    GpuToggleLabel(String, bool),
    GpuToggleIcon(String, bool),
    GpuToggleStackValues(String, bool),
    GpuSelectGraphType(String, DeviceKind, ChartKind),
    SelectGpuTempUnit(String, TempUnit),
    GpuTempMinTempChanged(String, f64),
    ToggleDisableOnBattery(String, bool),
    SysmonSelect(usize),

    ChangeContentOrder(ContentOrderChange),
}

/// The settings popup is built from columns of `settings::section()` lists.
type SettingsColumn<'a> = Column<'a, Message, cosmic::Theme, cosmic::Renderer>;

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

        LazyLock::force(&SYSMON_LIST);

        let gpus = Gpus::new(is_laptop);

        let is_horizontal = core.applet.is_horizontal();

        let mut app = Minimon {
            core,
            cpu: Cpu::new(is_horizontal),
            cputemp: CpuTemp::default(),
            memory: Memory::default(),
            network1: Network::default(),
            network2: Network::default(),
            disks1: Disks::default(),
            disks2: Disks::default(),
            gpus,
            nvidia_redetect_attempts: 0,
            popup: None,
            panel_size: None,
            settings_page: None,
            settings_tabs: segmented_button::Model::default(),
            about: Minimon::about(),
            colorpicker: ColorPicker::default(),
            config: MinimonConfig::default(),
            refresh_rate: Arc::new(AtomicU32::new(1000)),
            is_laptop,
            on_ac: true,
            data_is_visible: false,
            font_system: FontSystem::new(),
            interface_font: None,
            value_cpu_width: None,
            value_gpu_width: None,
            value_network_width: None,
            value_disks_width: None,
            value_w_width: None,
        };

        let config: MinimonConfig =
            cosmic::cosmic_config::Config::new(Self::APP_ID, MinimonConfig::VERSION)
                .map(|context| match CosmicConfigEntry::get_entry(&context) {
                    Ok(config) => config,
                    Err((errors, config)) => {
                        for e in errors {
                            log::warn!("Config issue: {:?}", e);
                        }
                        config
                    }
                })
                .unwrap_or_default();
        app.config_changed(&config);

        (app, Task::none())
    }

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
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

    fn on_window_resize(&mut self, id: Id, width: f32, height: f32) {
        if self.core.main_window_id() == Some(id) {
            self.panel_size = Some(iced::Size::new(width, height));
        }
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

        // Build the full list of panel elements
        let mut elements: Vec<Element<Message>> = Vec::new();

        // If the applet is not visible, return an icon button to toggle the popup
        if !self.data_is_visible {
            elements.extend(self.simple_ui());
        } else {
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

        // Overview or one of the settings sub pages
        } else {
            let spacing = cosmic::theme::spacing();

            let padding = if self.core.is_condensed() {
                spacing.space_s
            } else {
                spacing.space_m
            };

            let page = match &self.settings_page {
                None => self.overview_page(get_sysmon(&self.config.sysmon)),
                Some(SettingsVariant::General) => self.general_settings_page(),
                Some(SettingsVariant::Cpu) => self.cpu_settings_page(),
                Some(SettingsVariant::Memory) => self.memory_settings_page(),
                Some(SettingsVariant::Network) => self.network_settings_page(),
                Some(SettingsVariant::Disks) => self.disks_settings_page(),
                Some(SettingsVariant::Gpu(id)) => self.gpu_settings_page(id),
                Some(SettingsVariant::About) => self.about_settings_page(),
            }
            .spacing(spacing.space_s);

            // A sub page keeps its back link above the scroll area, so the way
            // out stays in reach in a page longer than the popup.
            let content: Element<'_, Message> = if self.settings_page.is_some() {
                let back = ui::back_button(&SETTINGS_BACK, Message::Settings(None));

                widget::column::with_capacity(2)
                    .push(container(back).padding(Padding::from(padding).bottom(spacing.space_s)))
                    .push(
                        page.padding(Padding::from(padding).top(0))
                            .apply(cosmic::widget::scrollable),
                    )
                    .into()
            } else {
                page.padding(padding)
                    .apply(cosmic::widget::scrollable)
                    .into()
            };

            let limits = Limits::NONE
                .max_width(380.0)
                .min_width(360.0)
                .min_height(200.0)
                .max_height(600.0);

            self.core
                .applet
                .popup_container(content)
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
                    return cosmic::surface::surface_task(cosmic::surface::action::destroy_popup(
                        p,
                    ));
                } else {
                    return cosmic::surface::surface_task(cosmic::surface::action::app_popup(
                        |_| Default::default(),
                        |app: &mut Minimon| {
                            let new_id = Id::unique();
                            app.popup.replace(new_id);

                            let mut popup_settings = app.core.applet.get_popup_settings(
                                app.core.main_window_id().unwrap(),
                                new_id,
                                Some((1, 1)),
                                None,
                                None,
                            );
                            // `get_popup_settings` anchors to a single applet slot, but
                            // the panel window is as long as all the sensors drawn in it.
                            // Anchor to the whole window so the popup is centred on the
                            // applet.
                            if let Some(size) = app.panel_size {
                                let anchor = &mut popup_settings.positioner.anchor_rect;
                                if app.core.applet.is_horizontal() {
                                    anchor.width = anchor.width.max(size.width.round() as i32);
                                } else {
                                    anchor.height = anchor.height.max(size.height.round() as i32);
                                }
                            }
                            popup_settings
                        },
                        None,
                    ));
                }
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
                self.rebuild_settings_tabs();
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
                self.rebuild_settings_tabs();
            }

            Message::ToggleDisksChart(variant, toggled) => {
                info!("Message::ToggleDiskChart({variant:?})");
                let (_, config) = disks_select!(self, variant);
                config.show_chart(toggled);
                self.save_config();
            }

            Message::ToggleDisksValue(variant, toggled) => {
                info!("Message::ToggleDiskLabel({variant:?})");
                let (_, config) = disks_select!(self, variant);
                config.show_value(toggled);
                self.save_config();
            }

            Message::ToggleDisksLabel(variant, toggled) => {
                info!("Message::ToggleDisksLabel({variant:?})");
                let (_, config) = disks_select!(self, variant);
                config.show_label(toggled);
                self.save_config();
            }

            Message::ToggleDisksIcon(variant, toggled) => {
                info!("Message::ToggleDisksIcon({variant:?})");
                let (_, config) = disks_select!(self, variant);
                config.show_icon(toggled);
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

                        for (id, gpu) in self.gpus.iter_mut() {
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

            Message::ToggleCpuValue(toggled) => {
                info!("Message::ToggleCpuValue({toggled:?})");
                self.config.cpu.show_value(toggled);
                self.save_config();
            }

            Message::ToggleCpuLabel(toggled) => {
                info!("Message::ToggleCpuLabel({toggled:?})");
                self.config.cpu.show_label(toggled);
                self.save_config();
            }

            Message::ToggleCpuIcon(toggled) => {
                info!("Message::ToggleCpuIcon({toggled:?})");
                self.config.cpu.show_icon(toggled);
                self.save_config();
            }

            Message::ToggleCpuTempValue(toggled) => {
                info!("Message::ToggleCpuTempValue({toggled:?})");
                self.config.cputemp.show_value(toggled);
                self.save_config();
            }

            Message::ToggleCpuTempLabel(toggled) => {
                info!("Message::ToggleCpuTempLabel({toggled:?})");
                self.config.cputemp.show_label(toggled);
                self.save_config();
            }

            Message::ToggleCpuTempIcon(toggled) => {
                info!("Message::ToggleCpuTempIcon({toggled:?})");
                self.config.cputemp.show_icon(toggled);
                self.save_config();
            }

            Message::ToggleMemoryValue(toggled) => {
                info!("Message::ToggleMemoryValue({toggled:?})");
                self.config.memory.show_value(toggled);
                self.save_config();
            }

            Message::ToggleMemoryLabel(toggled) => {
                info!("Message::ToggleMemoryLabel({toggled:?})");
                self.config.memory.show_label(toggled);
                self.save_config();
            }

            Message::ToggleMemoryIcon(toggled) => {
                info!("Message::ToggleMemoryIcon({toggled:?})");
                self.config.memory.show_icon(toggled);
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

            Message::ToggleNetValue(variant, toggled) => {
                info!("Message::ToggleNetValue({toggled:?})");
                let (_, config) = network_select!(self, variant);
                config.show_value(toggled);
                self.save_config();
            }

            Message::ToggleNetLabel(variant, toggled) => {
                info!("Message::ToggleNetLabel({toggled:?})");
                let (_, config) = network_select!(self, variant);
                config.show_label(toggled);
                self.save_config();
            }

            Message::ToggleNetIcon(variant, toggled) => {
                info!("Message::ToggleNetIcon({toggled:?})");
                let (_, config) = network_select!(self, variant);
                config.show_icon(toggled);
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

            Message::ValueSizeChanged(size) => {
                info!("Message::ValueSizeChanged({size:?})");
                self.config.value_size_default = size;
                self.save_config();
            }

            Message::ToggleMonospaceValues(toggle) => {
                info!("Message::Monospacelabels({toggle:?})");
                self.config.monospace_values = toggle;
                self.save_config();
            }

            Message::PanelSpacing(spacing) => {
                info!("Message::PanelSpacing({spacing})");
                self.config.panel_spacing = spacing;
                self.save_config();
            }

            Message::Settings(setting) => {
                info!("Message::Settings({setting:?})");
                self.settings_page = setting;
                self.rebuild_settings_tabs();
            }

            Message::SettingsTabSelected(entity) => {
                self.settings_tabs.activate(entity);
            }

            Message::LaunchWebbrowser(url) => {
                info!("Message::LaunchWebbrowser({url})");
                Minimon::launch_webbrowser(&url);
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

            Message::GpuToggleValue(id, device, toggled) => {
                self.update_gpu_config(
                    &id,
                    "GpuToggleLabel",
                    device,
                    |config, device| match device {
                        DeviceKind::Gpu => config.usage.show_value(toggled),
                        DeviceKind::Vram => config.vram.show_value(toggled),
                        DeviceKind::GpuTemp => config.temp.show_value(toggled),
                        _ => error!("GpuToggleLabel: wrong kind {device:?}"),
                    },
                );
            }

            Message::GpuToggleLabel(id, toggled) => {
                info!("Message::GpuToggleLabel({id:?}, {toggled:?})");
                if let Some(c) = self.config.gpus.get_mut(&id) {
                    c.usage.show_label(toggled);
                    self.save_config();
                } else {
                    error!("GpuToggleLabel: wrong id {id:?}");
                }
            }

            Message::GpuToggleIcon(id, toggled) => {
                info!("Message::GpuToggleIcon({id:?}, {toggled:?})");
                if let Some(c) = self.config.gpus.get_mut(&id) {
                    c.usage.show_icon(toggled);
                    self.save_config();
                } else {
                    error!("GpuToggleIcon: wrong id {id:?}");
                }
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

            Message::GpuToggleStackValues(id, toggled) => {
                info!("Message::GpuToggleStackValues({id:?}, {toggled:?})");
                if let Some(c) = self.config.gpus.get_mut(&id) {
                    c.stack_values = toggled;
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
                // Both indices are baked into the message when the row is drawn,
                // so a shorter order arriving from the config watcher in between
                // would make the swap panic.
                let len = self.config.content_order.order.len();
                if order_change.new_index == order_change.current_index
                    || order_change.new_index >= len
                    || order_change.current_index >= len
                {
                    return Task::none();
                }

                self.config
                    .content_order
                    .order
                    .swap(order_change.current_index, order_change.new_index);
                self.save_config();
            }
        }
        Task::none()
    }
}

impl Minimon {
    fn config_changed(&mut self, config: &MinimonConfig) {
        info!("Updating state with configuration data");
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
        self.rebuild_settings_tabs();

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

    /// Static information shown on the about page.
    fn about() -> About {
        About::default()
            .name("Minimon")
            .icon(widget::icon::from_name(ICON).handle())
            .version(env!("CARGO_PKG_VERSION"))
            .author("Hyperchaotic")
            //.developers([("Hyperchaotic", "hyperchaotic@gmail.com")])
            .links([
                (*ABOUT_LINKS_MAIN, REPOSITORY_URL),
                (*ABOUT_LINKS_ISSUES, TIP_URL),
            ])
            .license(LICENSE)
            .license_url(LICENSE_URL)
            .comments(fl!("app-description"))
    }

    /// Rebuilds the tab bar for the page currently open. Sensors with a single
    /// reading end up with an empty model and no tab bar at all.
    fn rebuild_settings_tabs(&mut self) {
        let previous = self.settings_tab();
        let mut tabs = segmented_button::Model::builder();

        match &self.settings_page {
            Some(SettingsVariant::Cpu) if self.cputemp.is_found() => {
                tabs = tabs
                    .insert(|tab| {
                        tab.text(fl!("tab-load"))
                            .data(SettingsTab::CpuLoad)
                            .activate()
                    })
                    .insert(|tab| tab.text(fl!("tab-temperature")).data(SettingsTab::CpuTemp));
            }
            Some(SettingsVariant::Network)
                if self.config.network1.variant != NetworkVariant::Combined =>
            {
                tabs = tabs
                    .insert(|tab| {
                        tab.text(fl!("tab-download"))
                            .data(SettingsTab::NetworkDownload)
                            .activate()
                    })
                    .insert(|tab| tab.text(fl!("tab-upload")).data(SettingsTab::NetworkUpload));
            }
            Some(SettingsVariant::Disks)
                if self.config.disks1.variant != DisksVariant::Combined =>
            {
                tabs = tabs
                    .insert(|tab| {
                        tab.text(fl!("tab-write"))
                            .data(SettingsTab::DisksWrite)
                            .activate()
                    })
                    .insert(|tab| tab.text(fl!("tab-read")).data(SettingsTab::DisksRead));
            }
            Some(SettingsVariant::Gpu(_)) => {
                tabs = tabs
                    .insert(|tab| {
                        tab.text(fl!("tab-gpu-load"))
                            .data(SettingsTab::GpuLoad)
                            .activate()
                    })
                    .insert(|tab| tab.text(fl!("tab-temperature")).data(SettingsTab::GpuTemp))
                    .insert(|tab| tab.text(fl!("tab-vram-load")).data(SettingsTab::GpuVram));
            }
            _ => {}
        }

        self.settings_tabs = tabs.build();

        // Stay on the reading the user was looking at, as long as it still has
        // a tab of its own.
        if let Some(previous) = previous {
            let restored = self
                .settings_tabs
                .iter()
                .find(|&tab| self.settings_tabs.data::<SettingsTab>(tab) == Some(&previous));

            if let Some(tab) = restored {
                self.settings_tabs.activate(tab);
            }
        }
    }

    /// Name of a GPU as it appears in the settings. Product names are long
    /// enough to break the layout, so they are only shown on the GPU's own
    /// page, and a number tells several of them apart.
    fn gpu_label(&self, id: &str) -> String {
        if self.gpus.len() < 2 {
            return (*SETTINGS_GPU_CHOICE).to_owned();
        }

        let index = self.gpus.iter().position(|(key, _)| key == id).unwrap_or(0);
        format!("{} {}", *SETTINGS_GPU_CHOICE, index + 1)
    }

    /// The reading the open sensor page is currently showing settings for.
    fn settings_tab(&self) -> Option<SettingsTab> {
        self.settings_tabs.active_data::<SettingsTab>().copied()
    }

    /// Scales a chart down to the preview shown in a sensor page header.
    fn chart_preview(
        chart: widget::Container<'_, Message, cosmic::Theme, cosmic::Renderer>,
    ) -> Element<'_, Message> {
        chart
            .width(ui::PREVIEW_SIZE)
            .height(ui::PREVIEW_SIZE)
            .into()
    }

    /// Frame shared by every sensor page: header, tab bar and the settings
    /// sections of the currently selected reading. The back link is drawn by
    /// [`Minimon::view_window`], above the scroll area.
    fn sensor_page<'a>(
        &'a self,
        title: impl Into<std::borrow::Cow<'a, str>> + 'a,
        subtitle: Option<String>,
        values: Vec<String>,
        preview: Element<'a, Message>,
        sections: Vec<Element<'a, Message>>,
    ) -> SettingsColumn<'a> {
        let mut content = Column::new()
            .push(ui::sensor_header(title, values, preview))
            .push_maybe(subtitle.map(text::caption));

        if self.settings_tabs.len() > 1 {
            content = content.push(
                widget::tab_bar::horizontal(&self.settings_tabs)
                    .on_activate(Message::SettingsTabSelected),
            );
        }

        for section in sections {
            content = content.push(section);
        }

        content
    }

    /// The page opened first: everything that can be configured, one row each.
    fn overview_page(
        &self,
        sysmon: Option<&'static system_monitors::DesktopApp>,
    ) -> SettingsColumn<'_> {
        let mut content = Column::new();

        if let Some(sysmon) = sysmon {
            let label = fl!("settings-launch", name = sysmon.name.as_str());
            content = content.push(settings::section().add(ui::action_row(
                label,
                widget::button::link::icon().icon(),
                Message::LaunchSystemMonitor(sysmon),
            )));
        }

        let sample_rate_ms = self.config.refresh_rate;

        let cpu = if self.cputemp.is_found() {
            format!("{} | {}", self.cpu, self.cputemp)
        } else {
            self.cpu.to_string()
        };

        let memory = if self.config.memory.show_allocated {
            format!(
                "{} / {:.1} GB / {:.1} GB",
                self.memory.to_string(false),
                self.memory.latest_sample_allocated(),
                self.memory.total()
            )
        } else {
            format!(
                "{} / {:.1} GB",
                self.memory.to_string(false),
                self.memory.total()
            )
        };

        let network = format!(
            "↓ {} | ↑ {}",
            self.network1
                .download_label(sample_rate_ms, network::UnitVariant::Long),
            self.network1
                .upload_label(sample_rate_ms, network::UnitVariant::Long)
        );

        let disks = format!(
            "W {} | R {}",
            self.disks1
                .write_label(sample_rate_ms, disks::UnitVariant::Long),
            self.disks1
                .read_label(sample_rate_ms, disks::UnitVariant::Long)
        );

        let mut sensors = settings::section()
            .add(ui::go_next_row(
                *SETTINGS_GENERAL_HEADING,
                Message::Settings(Some(SettingsVariant::General)),
            ))
            .add(ui::go_next_value_row(
                *SETTINGS_CPU_CHOICE,
                cpu,
                Message::Settings(Some(SettingsVariant::Cpu)),
            ))
            .add(ui::go_next_value_row(
                *SETTINGS_MEMORY_CHOICE,
                memory,
                Message::Settings(Some(SettingsVariant::Memory)),
            ))
            .add(ui::go_next_value_row(
                *SETTINGS_NETWORK_CHOICE,
                network,
                Message::Settings(Some(SettingsVariant::Network)),
            ))
            .add(ui::go_next_value_row(
                *SETTINGS_DISKS_CHOICE,
                disks,
                Message::Settings(Some(SettingsVariant::Disks)),
            ));

        for (id, gpu) in self.gpus.iter() {
            let info = format!(
                "{} {} / {:.1} GB | {}",
                gpu.gpu,
                gpu.vram.string(false),
                gpu.vram.total(),
                gpu.temp
            );
            sensors = sensors.add(ui::go_next_value_row(
                self.gpu_label(id),
                info,
                Message::Settings(Some(SettingsVariant::Gpu(id.clone()))),
            ));
        }

        content.push(sensors).push(
            settings::section()
                .add(ui::go_next_row(
                    *SETTINGS_ABOUT_CHOICE,
                    Message::Settings(Some(SettingsVariant::About)),
                ))
                .add(ui::action_row(
                    *SETTINGS_TIP,
                    ui::tip_icon(),
                    Message::LaunchWebbrowser(TIP_URL.to_owned()),
                )),
        )
    }

    /// Settings that are not tied to a single sensor.
    fn general_settings_page(&self) -> SettingsColumn<'_> {
        let refresh_rate = f64::from(self.config.refresh_rate) / 1000.0;

        let sysmon_index = self
            .config
            .sysmon
            .as_ref()
            .and_then(|name| SYSMON_NAMES.iter().position(|&app| app == name));

        // The config is a file the user can edit, and a spacing outside the
        // range the panel knows falls back to a default, so the row shows a
        // value the button can step through either way.
        let panel_spacing = self.config.panel_spacing.clamp(1, 6);

        let general = settings::section()
            .add(ui::control_row(
                fl!("refresh-rate"),
                spin_button(
                    format!("{refresh_rate:.2}"),
                    refresh_rate,
                    0.250,
                    0.250,
                    15.00,
                    Message::RefreshRateChanged,
                ),
            ))
            .add(ui::control_row(
                fl!("change-value-size"),
                spin_button(
                    self.config.value_size_default.to_string(),
                    self.config.value_size_default,
                    1,
                    5,
                    20,
                    Message::ValueSizeChanged,
                ),
            ))
            .add(
                settings::item::builder(fl!("settings-monospace_font"))
                    .toggler(self.config.monospace_values, Message::ToggleMonospaceValues),
            )
            .add(ui::control_row(
                fl!("settings-panel-spacing"),
                spin_button(
                    panel_spacing.to_string(),
                    panel_spacing,
                    1,
                    1,
                    6,
                    Message::PanelSpacing,
                ),
            ))
            .add(ui::control_row(
                fl!("choose-sysmon"),
                widget::dropdown(&*SYSMON_NAMES, sysmon_index, Message::SysmonSelect).width(180),
            ));

        let mut order = settings::section().title(fl!("content-order"));

        // Entries for hardware that is not present are left out, so the arrows
        // have to swap with the neighbouring *visible* entry rather than with
        // whatever happens to sit next in the list.
        let visible: Vec<(usize, String)> = self
            .config
            .content_order
            .order
            .iter()
            .enumerate()
            .filter_map(|(index, content)| {
                let label = match content {
                    ContentType::CpuUsage => fl!("settings-cpu"),
                    ContentType::CpuTemp if self.cputemp.is_found() => {
                        fl!("settings-cpu-temperature")
                    }
                    ContentType::MemoryUsage => fl!("settings-memory"),
                    ContentType::NetworkUsage => fl!("settings-network"),
                    ContentType::DiskUsage => fl!("settings-disks"),
                    ContentType::GpuInfo if self.has_gpus() => fl!("settings-gpu"),
                    ContentType::CpuTemp | ContentType::GpuInfo => return None,
                };
                Some((index, label))
            })
            .collect();

        for (position, (index, label)) in visible.iter().enumerate() {
            let swap_with = |other: &(usize, String)| {
                Message::ChangeContentOrder(ContentOrderChange {
                    current_index: *index,
                    new_index: other.0,
                })
            };
            let move_up = position.checked_sub(1).map(|p| swap_with(&visible[p]));
            let move_down = visible.get(position + 1).map(swap_with);

            order = order.add(
                widget::row::with_capacity(3)
                    .push(
                        widget::button::icon(widget::icon::from_name("pan-up-symbolic"))
                            .on_press_maybe(move_up),
                    )
                    .push(
                        widget::button::icon(widget::icon::from_name("pan-down-symbolic"))
                            .on_press_maybe(move_down),
                    )
                    .push(text::body(label.clone()))
                    .align_y(Alignment::Center)
                    .spacing(cosmic::theme::spacing().space_xxs),
            );
        }

        Column::new()
            .push(text::title3(*SETTINGS_GENERAL_HEADING))
            .push(general)
            .push(order)
    }

    fn cpu_settings_page(&self) -> SettingsColumn<'_> {
        if self.settings_tab() == Some(SettingsTab::CpuTemp) {
            return self.sensor_page(
                *SETTINGS_CPU_CHOICE,
                self.cpu.name(),
                vec![self.cputemp.to_string()],
                Minimon::chart_preview(self.cputemp.chart(ui::PREVIEW_SIZE, ui::PREVIEW_SIZE)),
                vec![self.cputemp.settings_ui()],
            );
        }

        // Stacked bars are as wide as the machine has cores, so the preview
        // cannot be squeezed into a square like the other chart kinds. It still
        // has to share the header row with the title and the reading, so a
        // many-core machine gets a scaled down preview rather than a clipped one.
        let preview = if self.cpu.graph_kind() == ChartKind::StackedBars {
            let natural = StackedBarSvg::new(
                self.config.cpu.bar_width,
                ui::PREVIEW_SIZE,
                self.config.cpu.bar_spacing,
            )
            .width(self.cpu.core_count());
            let width = natural.min(ui::PREVIEW_MAX_WIDTH);
            self.cpu
                .chart(ui::PREVIEW_SIZE, natural)
                .width(width)
                .height(ui::PREVIEW_SIZE)
                .into()
        } else {
            Minimon::chart_preview(self.cpu.chart(ui::PREVIEW_SIZE, ui::PREVIEW_SIZE))
        };

        self.sensor_page(
            *SETTINGS_CPU_CHOICE,
            self.cpu.name(),
            vec![self.cpu.to_string()],
            preview,
            vec![self.cpu.settings_ui()],
        )
    }

    fn memory_settings_page(&self) -> SettingsColumn<'_> {
        let mut values = vec![self.memory.to_string(false)];
        if self.config.memory.show_allocated {
            values.push(format!("{:.1} GB", self.memory.latest_sample_allocated()));
        }

        self.sensor_page(
            *SETTINGS_MEMORY_CHOICE,
            None,
            values,
            Minimon::chart_preview(self.memory.chart(ui::PREVIEW_SIZE, ui::PREVIEW_SIZE)),
            vec![self.memory.settings_ui()],
        )
    }

    fn network_settings_page(&self) -> SettingsColumn<'_> {
        let sample_rate_ms = self.config.refresh_rate;
        let combined = self.config.network1.variant == NetworkVariant::Combined;
        let upload = self.settings_tab() == Some(SettingsTab::NetworkUpload);

        let network = if upload {
            &self.network2
        } else {
            &self.network1
        };

        let mut values = Vec::with_capacity(2);
        if combined || !upload {
            values.push(format!(
                "↓ {}",
                self.network1
                    .download_label(sample_rate_ms, network::UnitVariant::Long)
            ));
        }
        if combined || upload {
            values.push(format!(
                "↑ {}",
                network.upload_label(sample_rate_ms, network::UnitVariant::Long)
            ));
        }

        // The panel draws one label and one icon for the whole sensor, both read
        // off the first config, so they belong here rather than in a tab.
        let variant = self.config.network1.variant;
        let device = settings::section()
            .add(
                settings::item::builder(fl!("enable-label"))
                    .toggler(self.config.network1.label_visible(), move |t| {
                        Message::ToggleNetLabel(variant, t)
                    }),
            )
            .add(
                settings::item::builder(fl!("enable-icon"))
                    .toggler(self.config.network1.icon_visible(), move |t| {
                        Message::ToggleNetIcon(variant, t)
                    }),
            )
            .add(
                settings::item::builder(fl!("net-use-bytes"))
                    .toggler(self.config.network1.show_bytes, Message::ToggleNetBytes),
            )
            .add(
                settings::item::builder(fl!("enable-net-combined"))
                    .toggler(combined, Message::ToggleNetCombined),
            );

        self.sensor_page(
            *SETTINGS_NETWORK_CHOICE,
            None,
            values,
            Minimon::chart_preview(network.chart(ui::PREVIEW_SIZE, ui::PREVIEW_SIZE)),
            vec![network.settings_ui(), device.into()],
        )
    }

    fn disks_settings_page(&self) -> SettingsColumn<'_> {
        let sample_rate_ms = self.config.refresh_rate;
        let combined = self.config.disks1.variant == DisksVariant::Combined;
        let read = self.settings_tab() == Some(SettingsTab::DisksRead);

        let disk = if read { &self.disks2 } else { &self.disks1 };

        let mut values = Vec::with_capacity(2);
        if combined || !read {
            values.push(format!(
                "W {}",
                self.disks1
                    .write_label(sample_rate_ms, disks::UnitVariant::Long)
            ));
        }
        if combined || read {
            values.push(format!(
                "R {}",
                disk.read_label(sample_rate_ms, disks::UnitVariant::Long)
            ));
        }

        // The panel draws one label and one icon for the whole sensor, both read
        // off the first config, so they belong here rather than in a tab.
        let variant = self.config.disks1.variant;
        let device = settings::section()
            .add(
                settings::item::builder(fl!("enable-label"))
                    .toggler(self.config.disks1.label_visible(), move |t| {
                        Message::ToggleDisksLabel(variant, t)
                    }),
            )
            .add(
                settings::item::builder(fl!("enable-icon"))
                    .toggler(self.config.disks1.icon_visible(), move |t| {
                        Message::ToggleDisksIcon(variant, t)
                    }),
            )
            .add(
                settings::item::builder(fl!("enable-disks-combined"))
                    .toggler(combined, Message::ToggleDisksCombined),
            );

        self.sensor_page(
            *SETTINGS_DISKS_CHOICE,
            None,
            values,
            Minimon::chart_preview(disk.chart(ui::PREVIEW_SIZE, ui::PREVIEW_SIZE)),
            vec![disk.settings_ui(), device.into()],
        )
    }

    fn gpu_settings_page(&self, id: &str) -> SettingsColumn<'_> {
        let (Some(gpu), Some(config)) = (self.gpus.get(id), self.config.gpus.get(id)) else {
            error!("SettingsVariant::Gpu: Not found {id}");
            return Column::new();
        };

        let (value, preview, section) = match self.settings_tab() {
            Some(SettingsTab::GpuTemp) => (
                gpu.temp.to_string(),
                Minimon::chart_preview(gpu.temp.chart()),
                gpu.settings_temp_ui(&config.temp),
            ),
            Some(SettingsTab::GpuVram) => (
                gpu.vram.string(false),
                Minimon::chart_preview(gpu.vram.chart()),
                gpu.settings_vram_ui(&config.vram),
            ),
            _ => (
                gpu.gpu.to_string(),
                Minimon::chart_preview(gpu.gpu.chart()),
                gpu.settings_usage_ui(&config.usage),
            ),
        };

        self.sensor_page(
            self.gpu_label(id),
            Some(gpu.name()),
            vec![value],
            preview,
            vec![section, gpu.settings_device_ui(config)],
        )
    }

    fn about_settings_page(&self) -> SettingsColumn<'_> {
        Column::new().push(widget::about(&self.about, |url| {
            Message::LaunchWebbrowser(url.to_owned())
        }))
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

    fn push_text_label(&self, elements: &mut VecDeque<Element<crate::app::Message>>, label: &str) {
        let size = self.config.value_size_default;
        elements.push_back(widget::text::body(label.to_string()).size(size).into());
    }

    fn simple_ui(&'_ self) -> VecDeque<Element<'_, crate::app::Message>> {
        let mut elements: VecDeque<Element<Message>> = VecDeque::new();
        elements.push_front(
            self.core
                .applet
                .icon_button(ICON)
                .on_press(Message::TogglePopup)
                .into(),
        );
        elements
    }

    fn cpu_panel_ui(&'_ self, horizontal: bool) -> VecDeque<Element<'_, crate::app::Message>> {
        let size = self.core.applet.suggested_size(false);

        let mut elements: VecDeque<Element<Message>> = VecDeque::new();

        let cpu_has_content = self.config.cpu.value_visible() || self.config.cpu.chart_visible();

        if self.config.cpu.icon_visible() && cpu_has_content {
            self.push_symbolic_icon(&mut elements, CPU_ICON, false);
        }

        if self.config.cpu.label_visible() && cpu_has_content {
            self.push_text_label(&mut elements, &fl!("label-cpu"));
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

        if self.config.cpu.value_visible() {
            elements.push_back(
                self.figure_value(formatted_cpu, self.value_cpu_width)
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
            let cputemp_has_content =
                self.config.cputemp.value_visible() || self.config.cputemp.chart_visible();

            if self.config.cputemp.icon_visible() && cputemp_has_content {
                self.push_symbolic_icon(&mut elements, TEMP_ICON, false);
            }

            if self.config.cputemp.label_visible() && cputemp_has_content {
                self.push_text_label(&mut elements, &fl!("label-cpu-temp"));
            }

            if self.config.cputemp.value_visible() {
                elements.push_back(self.figure_value(self.cputemp.to_string(), None).into());
            }

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

        let memory_has_content =
            self.config.memory.value_visible() || self.config.memory.chart_visible();

        if self.config.memory.icon_visible() && memory_has_content {
            self.push_symbolic_icon(&mut elements, RAM_ICON, false);
        }

        if self.config.memory.label_visible() && memory_has_content {
            self.push_text_label(&mut elements, &fl!("label-memory"));
        }

        if self.config.memory.value_visible() {
            let formatted_mem = self.memory.to_string(!horizontal);
            elements.push_back(self.figure_value(formatted_mem, None).into());
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

        let format_value = |text: String| self.figure_value(text, self.value_network_width);

        let unit_len = if horizontal {
            network::UnitVariant::Long
        } else {
            network::UnitVariant::Short
        };

        let network_has_content = self.config.network1.value_visible()
            || self.config.network1.chart_visible()
            || (!nw_combined
                && (self.config.network2.value_visible() || self.config.network2.chart_visible()));

        if self.config.network1.label_visible() && network_has_content {
            self.push_text_label(&mut elements, &fl!("label-network"));
        }

        if self.config.network1.value_visible() {
            let mut network_values = Vec::new();
            let mut dl_row = Vec::new();

            if horizontal {
                dl_row.push(self.figure_value("↓".to_owned(), None).into());
            }
            dl_row.push(
                format_value(
                    self.network1
                        .download_label(sample_rate_ms, unit_len)
                        .clone(),
                )
                .into(),
            );

            if nw_combined {
                network_values.push(widget::space::vertical().into());
            }

            network_values.push(Row::from_vec(dl_row).into());

            if nw_combined {
                let mut ul_row = Vec::new();

                if horizontal {
                    ul_row.push(self.figure_value("↑".to_owned(), None).into());
                }
                ul_row.push(
                    format_value(self.network1.upload_label(sample_rate_ms, unit_len)).into(),
                );

                network_values.push(Row::from_vec(ul_row).into());
                network_values.push(widget::space::vertical().into());
            }

            elements.push_back(Column::from_vec(network_values).into());
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

        if self.config.network2.value_visible() && !nw_combined {
            let mut network_values = Vec::new();

            let mut ul_row = Vec::new();

            if horizontal {
                ul_row.push(self.figure_value("↑".to_owned(), None).into());
            }
            ul_row.push(format_value(self.network2.upload_label(sample_rate_ms, unit_len)).into());

            network_values.push(Row::from_vec(ul_row).into());

            elements.push_back(Column::from_vec(network_values).into());
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

        if self.config.network1.icon_visible() && network_has_content {
            self.push_symbolic_icon(&mut elements, NETWORK_ICON, true);
        }

        elements
    }

    fn disks_panel_ui(&'_ self, horizontal: bool) -> VecDeque<Element<'_, crate::app::Message>> {
        let size = self.core.applet.suggested_size(false);

        let disks_combined = self.config.disks1.variant == DisksVariant::Combined;
        let sample_rate_ms = self.config.refresh_rate;
        let mut elements: VecDeque<Element<Message>> = VecDeque::new();

        let format_value = |text: String| self.figure_value(text, self.value_disks_width);

        let unit_len = if horizontal {
            disks::UnitVariant::Long
        } else {
            disks::UnitVariant::Short
        };

        let disks_has_content = self.config.disks1.value_visible()
            || self.config.disks1.chart_visible()
            || (!disks_combined
                && (self.config.disks2.value_visible() || self.config.disks2.chart_visible()));

        if self.config.disks1.label_visible() && disks_has_content {
            self.push_text_label(&mut elements, &fl!("label-disks"));
        }

        if self.config.disks1.value_visible() {
            let mut disks_values = Vec::new();

            let mut wr_row = Vec::new();
            if horizontal {
                wr_row.push(self.figure_value("w".to_owned(), self.value_w_width).into());
            }
            wr_row.push(format_value(self.disks1.write_label(sample_rate_ms, unit_len)).into());

            if disks_combined {
                disks_values.push(widget::space::vertical().into());
            }

            disks_values.push(Row::from_vec(wr_row).spacing(0).padding(0).into());

            if disks_combined {
                let mut rd_row = Vec::new();
                if horizontal {
                    rd_row.push(self.figure_value("r".to_owned(), self.value_w_width).into());
                }
                rd_row.push(format_value(self.disks1.read_label(sample_rate_ms, unit_len)).into());

                disks_values.push(Row::from_vec(rd_row).spacing(0).padding(0).into());
                disks_values.push(widget::space::vertical().into());
            }

            elements.push_back(Column::from_vec(disks_values).into());
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

        if self.config.disks2.value_visible() && !disks_combined {
            let mut disks_values = Vec::new();

            let mut rd_row = Vec::new();
            if horizontal {
                rd_row.push(self.figure_value("r".to_owned(), self.value_w_width).into());
            }
            rd_row.push(format_value(self.disks2.read_label(sample_rate_ms, unit_len)).into());
            disks_values.push(Row::from_vec(rd_row).spacing(0).padding(0).into());

            elements.push_back(Column::from_vec(disks_values).into());
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

        if self.config.disks1.icon_visible() && disks_has_content {
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
            let gpu_has_content = config.usage.value_visible()
                || config.usage.chart_visible()
                || config.temp.value_visible()
                || config.temp.chart_visible()
                || config.vram.value_visible()
                || config.vram.chart_visible();

            if config.usage.label_visible() && gpu_has_content {
                self.push_text_label(&mut elements, &fl!("label-gpu"));
            }

            let formatted_gpu = gpu.gpu.to_string();
            let formatted_vram = gpu.vram.string(!horizontal);
            let stacked_values =
                config.stack_values && config.usage.value_visible() && config.vram.value_visible();

            if stacked_values {
                let gpu_values = vec![
                    widget::space::vertical().into(),
                    self.figure_value(formatted_gpu, self.value_gpu_width)
                        .into(),
                    self.figure_value(formatted_vram.clone(), None).into(),
                    widget::space::vertical().into(),
                ];
                elements.push_back(Column::from_vec(gpu_values).into());
            } else if config.usage.value_visible() {
                elements.push_back(
                    self.figure_value(formatted_gpu, self.value_gpu_width)
                        .into(),
                );
            }

            if config.usage.chart_visible() {
                elements.push_back(gpu.gpu.chart().height(size.0).width(size.1).into());
            }
            if config.temp.value_visible() {
                elements.push_back(self.figure_value(gpu.temp.to_string(), None).into());
            }

            if config.temp.chart_visible() {
                elements.push_back(gpu.temp.chart().height(size.0).width(size.1).into());
            }

            if config.vram.value_visible() && !stacked_values {
                elements.push_back(self.figure_value(formatted_vram, None).into());
            }

            if config.vram.chart_visible() {
                elements.push_back(gpu.vram.chart().height(size.0).width(size.1).into());
            }
        }

        if let Some(config) = self.config.gpus.get(&gpu.id()) {
            let gpu_has_content = config.usage.value_visible()
                || config.usage.chart_visible()
                || config.temp.value_visible()
                || config.temp.chart_visible()
                || config.vram.value_visible()
                || config.vram.chart_visible();

            if config.usage.icon_visible() && gpu_has_content {
                self.push_symbolic_icon(&mut elements, GPU_ICON, true);
            }
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
        // Redetect Nvidia GPUs if none found.
        // Retry NVIDIA_REDETECT_ATTEMPTS times because Flatpak/NVML startup
        // can race session initialization.
        if !self.gpus.has_type(GpuType::Nvidia)
            && self.nvidia_redetect_attempts < NVIDIA_REDETECT_ATTEMPTS
        {
            self.nvidia_redetect_attempts += 1;

            info!(
                "No Nvidia GPU detected, retry attempt {}",
                self.nvidia_redetect_attempts
            );

            self.gpus.redetect(GpuType::Nvidia, self.is_laptop);

            // Sync configs in case a new GPU appeared
            self.sync_gpu_configs();
        }

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
            for (id, gpu) in self.gpus.iter_mut() {
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
            Size::PanelSize(PanelSize::XL) => self.config.value_size_default + 5,
            Size::PanelSize(PanelSize::L) => self.config.value_size_default + 3,
            Size::PanelSize(PanelSize::M) => self.config.value_size_default + 2,
            Size::PanelSize(PanelSize::S) => self.config.value_size_default + 1,
            Size::PanelSize(PanelSize::XS) => self.config.value_size_default,
            _ => self.config.value_size_default,
        }
    }

    fn figure_value<'a>(
        &self,
        text: String,
        width: Option<f32>,
    ) -> widget::Text<'a, cosmic::Theme> {
        let size = self.label_font_size();

        if self.config.monospace_values {
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

    fn sync_gpu_configs(&mut self) {
        let config_gpus = &mut self.config.gpus;

        // Remove entries not present in detected GPUs
        config_gpus.retain(|id, _| self.gpus.get(id).is_some());

        // Add missing GPU configs
        for (id, _) in self.gpus.iter() {
            config_gpus.entry(id.clone()).or_default();
        }

        // Sync runtime config into GPU objects
        for (id, gpu) in self.gpus.iter_mut() {
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
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_text(text, attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        buffer.layout_runs().next().map(|run| run.line_w.ceil())
    }

    fn calculate_max_label_widths(&mut self) {
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

            self.value_cpu_width = self.measure_text_width("8.88%", &attrs);
            self.value_gpu_width = self.value_cpu_width;

            self.value_network_width = match (self.config.network1.show_bytes, is_horizontal) {
                (false, false) => self.measure_text_width("8.88M", &attrs),
                (false, true) => self.measure_text_width("8.88 Mbps", &attrs),
                (true, false) => self.measure_text_width("8.88M", &attrs),
                (true, true) => self.measure_text_width("8.88 MB/s", &attrs),
            };

            self.value_disks_width = if is_horizontal {
                self.measure_text_width("8.88 MB/s", &attrs)
            } else {
                self.measure_text_width("8.88M", &attrs)
            };

            self.value_w_width = self.measure_text_width("W ", &attrs);
        }
    }

    /// Hands a link to the desktop, from inside the sandbox if there is one.
    fn launch_webbrowser(url: &str) {
        // The about page offers an address for every contributor, even the ones
        // that did not leave one behind.
        if url.is_empty() || url == "mailto:" {
            return;
        }

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
