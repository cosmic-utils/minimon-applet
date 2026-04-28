use cosmic::iced::Background;
use cosmic::iced::alignment::Horizontal;
use cosmic::iced::{
    Alignment,
    widget::{column, row},
};
use cosmic::{Element, cosmic_theme::palette::Srgba};
use std::rc::Rc;

use cosmic::{
    iced::{
        Color, Length, Radians,
        gradient::{ColorStop, Linear},
    },
    theme,
    widget::{
        self,
        slider::{self, HandleShape},
    },
};
use theme::iced::Slider;

use crate::app::Message;
use crate::config::{ChartColors, ChartKind, ColorVariant, DeviceKind};
use crate::fl;
use log::info;

const RED_RECT: &str = "<svg width=\"50\" height=\"50\" xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"50\" height=\"50\" x=\"0\" y=\"0\" rx=\"15\" ry=\"15\" fill=\"red\" /></svg>";
const GREEN_RECT: &str = "<svg width=\"50\" height=\"50\" xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"50\" height=\"50\" x=\"0\" y=\"0\" rx=\"15\" ry=\"15\" fill=\"green\" /></svg>";
const BLUE_RECT: &str = "<svg width=\"50\" height=\"50\" xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"50\" height=\"50\" x=\"0\" y=\"0\" rx=\"15\" ry=\"15\" fill=\"blue\" /></svg>";
const ALPHA_RECT: &str = "<svg width=\"50\" height=\"50\" xmlns=\"http://www.w3.org/2000/svg\">\
  <rect width=\"48\" height=\"48\" x=\"1\" y=\"1\" rx=\"15\" ry=\"15\" fill=\"none\" stroke=\"lightgrey\" stroke-width=\"1\"/>\
  <line x1=\"5\" y1=\"5\" x2=\"45\" y2=\"45\" stroke=\"red\" stroke-width=\"3\"/>\
  <line x1=\"5\" y1=\"45\" x2=\"45\" y2=\"5\" stroke=\"red\" stroke-width=\"3\"/>\
</svg>";

use std::sync::{LazyLock, Mutex};

pub static COLOR_STOPS_RED_LOW: LazyLock<Mutex<[ColorStop; 2]>> = LazyLock::new(|| {
    Mutex::new([
        ColorStop {
            offset: 0.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
    ])
});

pub static COLOR_STOPS_RED_HIGH: LazyLock<Mutex<[ColorStop; 2]>> = LazyLock::new(|| {
    Mutex::new([
        ColorStop {
            offset: 0.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgb(1.0, 0.0, 0.0),
        },
    ])
});

pub static COLOR_STOPS_GREEN_LOW: LazyLock<Mutex<[ColorStop; 2]>> = LazyLock::new(|| {
    Mutex::new([
        ColorStop {
            offset: 0.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
    ])
});

pub static COLOR_STOPS_GREEN_HIGH: LazyLock<Mutex<[ColorStop; 2]>> = LazyLock::new(|| {
    Mutex::new([
        ColorStop {
            offset: 0.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgb(0.0, 1.0, 0.0),
        },
    ])
});

pub static COLOR_STOPS_BLUE_LOW: LazyLock<Mutex<[ColorStop; 2]>> = LazyLock::new(|| {
    Mutex::new([
        ColorStop {
            offset: 0.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
    ])
});

pub static COLOR_STOPS_BLUE_HIGH: LazyLock<Mutex<[ColorStop; 2]>> = LazyLock::new(|| {
    Mutex::new([
        ColorStop {
            offset: 0.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgb(0.0, 0.0, 1.0),
        },
    ])
});

const ERROR: &str = "<svg width=\"800px\" height=\"800px\" viewBox=\"0 0 25 25\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">
<path d=\"M12.5 16V14.5M12.5 9V13M20.5 12.5C20.5 16.9183 16.9183 20.5 12.5 20.5C8.08172 20.5 4.5 16.9183 4.5 12.5C4.5 8.08172 8.08172 4.5 12.5 4.5C16.9183 4.5 20.5 8.08172 20.5 12.5Z\" stroke=\"red\" stroke-width=\"1.2\"/>
</svg>";

pub trait DemoGraph {
    fn demo(&self) -> String;
    fn kind(&self) -> ChartKind;
    fn colors(&self) -> &ChartColors;
    fn set_colors(&mut self, colors: &ChartColors);
    fn color_choices(&self) -> Vec<(&'static str, ColorVariant)>;
    fn id(&self) -> Option<String>;
}

/// Data for managing the `ColorPicker` dialog
pub struct ColorPicker {
    demo_chart: Option<Box<dyn DemoGraph>>,
    device: DeviceKind,
    // Current field being adjusted background/text/etc.
    color_variant: ColorVariant,
    ///Current slider values
    slider_red_val: u8,
    slider_green_val: u8,
    slider_blue_val: u8,
    slider_alpha_val: u8,
}

impl Default for ColorPicker {
    fn default() -> Self {
        ColorPicker {
            demo_chart: None,
            device: DeviceKind::Cpu,
            color_variant: ColorVariant::Background,
            slider_red_val: 0,
            slider_green_val: 0,
            slider_blue_val: 0,
            slider_alpha_val: 0,
        }
    }
}

impl ColorPicker {
    pub fn device(&self) -> DeviceKind {
        self.device
    }

    pub fn active(&self) -> bool {
        self.demo_chart.is_some()
    }

    pub fn activate(&mut self, device: DeviceKind, demo_chart: Box<dyn DemoGraph>) {
        info!("colorpicker::activate({device:?})");
        self.device = device;
        self.color_variant = ColorVariant::Background;
        self.demo_chart = Some(demo_chart);
    }

    pub fn deactivate(&mut self) {
        self.demo_chart = None;
    }

    // This function is largely borrowed from the PixelDoted color picker:
    // https://github.com/PixelDoted/cosmic-ext-color-picker
    fn color_slider<'b, Message>(
        value: u8,
        on_change: impl Fn(u8) -> Message + 'b,
        color_stops_low: &'static Mutex<[ColorStop; 2]>,
        color_stops_high: &'static Mutex<[ColorStop; 2]>,
    ) -> cosmic::Element<'b, Message>
    where
        Message: Clone + 'b,
    {
        widget::slider(0..=255, value, on_change)
            .width(Length::Fixed(220.0))
            .step(1)
            .class(Slider::Custom {
                active: Rc::new(|t| {
                    let cosmic = t.cosmic();
                    let mut a =
                        slider::Catalog::style(t, &Slider::default(), slider::Status::Active);

                    a.rail.backgrounds = (
                        Background::Gradient(cosmic::iced::Gradient::Linear(
                            Linear::new(Radians(90.0))
                                .add_stops(color_stops_low.lock().unwrap().iter().copied()),
                        )),
                        Background::Gradient(cosmic::iced::Gradient::Linear(
                            Linear::new(Radians(90.0))
                                .add_stops(color_stops_high.lock().unwrap().iter().copied()),
                        )),
                    );
                    a.rail.width = 8.0;
                    a.handle.background = Color::TRANSPARENT.into();
                    a.handle.shape = HandleShape::Circle { radius: 8.0 };
                    a.handle.border_color = cosmic.palette.neutral_10.into();
                    a.handle.border_width = 4.0;
                    a
                }),
                hovered: Rc::new(|t| {
                    let cosmic = t.cosmic();
                    let mut a =
                        slider::Catalog::style(t, &Slider::default(), slider::Status::Hovered);

                    a.rail.backgrounds = (
                        Background::Gradient(cosmic::iced::Gradient::Linear(
                            Linear::new(Radians(90.0))
                                .add_stops(color_stops_low.lock().unwrap().iter().copied()),
                        )),
                        Background::Gradient(cosmic::iced::Gradient::Linear(
                            Linear::new(Radians(90.0))
                                .add_stops(color_stops_high.lock().unwrap().iter().copied()),
                        )),
                    );
                    a.rail.width = 8.0;
                    a.handle.background = Color::TRANSPARENT.into();
                    a.handle.shape = HandleShape::Circle { radius: 8.0 };
                    a.handle.border_color = cosmic.palette.neutral_10.into();
                    a.handle.border_width = 4.0;
                    a
                }),
                dragging: Rc::new(|t| {
                    let cosmic = t.cosmic();
                    let mut a =
                        slider::Catalog::style(t, &Slider::default(), slider::Status::Dragged);

                    a.rail.backgrounds = (
                        Background::Gradient(cosmic::iced::Gradient::Linear(
                            Linear::new(Radians(90.0))
                                .add_stops(color_stops_low.lock().unwrap().iter().copied()),
                        )),
                        Background::Gradient(cosmic::iced::Gradient::Linear(
                            Linear::new(Radians(90.0))
                                .add_stops(color_stops_high.lock().unwrap().iter().copied()),
                        )),
                    );
                    a.rail.width = 8.0;
                    a.handle.background = Color::TRANSPARENT.into();
                    a.handle.shape = HandleShape::Circle { radius: 8.0 };
                    a.handle.border_color = cosmic.palette.neutral_10.into();
                    a.handle.border_width = 4.0;
                    a
                }),
            })
            .into()
    }

    pub fn sliders(&self) -> Srgba<u8> {
        Srgba::from_components((
            self.slider_red_val,
            self.slider_green_val,
            self.slider_blue_val,
            self.slider_alpha_val,
        ))
    }

    pub fn demo(&self) -> String {
        if let Some(d) = self.demo_chart.as_ref() {
            let demo = d.demo();
            return demo;
        }
        ERROR.to_string()
    }

    pub fn update_color(&mut self, color: Srgba<u8>) {
        self.slider_red_val = color.red;
        self.slider_green_val = color.green;
        self.slider_blue_val = color.blue;
        self.slider_alpha_val = color.alpha;

        if let Some(dmo) = self.demo_chart.as_mut() {
            let mut col = *dmo.colors();
            col.set_color(color, self.color_variant);
            dmo.set_colors(&col);

            // Set the shading for sliders, this is required to be static lifetime
            COLOR_STOPS_RED_LOW.lock().unwrap()[1].color =
                Color::from_rgb(f32::from(color.red) / f32::from(u8::MAX), 0.0, 0.0);
            COLOR_STOPS_RED_HIGH.lock().unwrap()[0].color =
                Color::from_rgb(f32::from(color.red) / f32::from(u8::MAX), 0.0, 0.0);

            COLOR_STOPS_GREEN_LOW.lock().unwrap()[1].color =
                Color::from_rgb(0.0, f32::from(color.green) / f32::from(u8::MAX), 0.0);
            COLOR_STOPS_GREEN_HIGH.lock().unwrap()[0].color =
                Color::from_rgb(0.0, f32::from(color.green) / f32::from(u8::MAX), 0.0);

            COLOR_STOPS_BLUE_LOW.lock().unwrap()[1].color =
                Color::from_rgb(0.0, 0.0, f32::from(color.blue) / f32::from(u8::MAX));
            COLOR_STOPS_BLUE_HIGH.lock().unwrap()[0].color =
                Color::from_rgb(0.0, 0.0, f32::from(color.blue) / f32::from(u8::MAX));
        } else {
            log::error!("Colorpicker::update_color() No demo graph object!");
        }
    }

    pub fn default_colors(&mut self) {
        if let Some(dmo) = self.demo_chart.as_mut() {
            let colors = ChartColors::new(self.device, dmo.kind());
            dmo.set_colors(&colors);
            self.update_color(colors.get_color(self.color_variant));
        } else {
            log::error!("Colorpicker::default_colors() No demo graph object!");
        }
    }

    pub fn color_variant(&self) -> ColorVariant {
        self.color_variant
    }

    pub fn set_color_variant(&mut self, variant: ColorVariant) {
        if let Some(dmo) = self.demo_chart.as_mut() {
            self.color_variant = variant;
            let color = dmo.colors().get_color(variant);
            self.update_color(color);
        } else {
            log::error!("Colorpicker::set_color_variant() No demo graph object!");
        }
    }

    pub fn colors(&self) -> &ChartColors {
        if let Some(dmo) = self.demo_chart.as_ref() {
            dmo.colors()
        } else {
            log::error!("Colorpicker::set_color_variant() No demo graph object!");
            panic!();
        }
    }

    pub fn view_colorpicker(&'_ self) -> Element<'_, crate::app::Message> {
        let color = self.sliders();
        let title = format!("{} {}", self.device, fl!("colorpicker-colors"));

        if let Some(dmo) = self.demo_chart.as_ref() {
            let mut children1 = Vec::new();
            let mut children2 = Vec::new();

            for (s, c) in dmo.color_choices() {
                let wgt = Element::from(widget::radio(
                    s,
                    c,
                    if self.color_variant() == c {
                        Some(c)
                    } else {
                        None
                    },
                    Message::ColorPickerSelectVariant,
                ));
                if children1.len() < 6 {
                    children1.push(widget::space::horizontal().width(20).into());
                    children1.push(wgt);
                } else {
                    children2.push(widget::space::horizontal().width(20).into());
                    children2.push(wgt);
                }
            }

            let fields = column!(
                cosmic::widget::row::with_children(children1),
                cosmic::widget::row::with_children(children2)
            );

            let c = widget::list_column()
                .add(
                    widget::text::title2(title)
                        .width(Length::Fill)
                        .align_x(Horizontal::Center),
                )
                .add(
                    widget::svg(widget::svg::Handle::from_memory(self.demo().into_bytes()))
                        .width(Length::Fill)
                        .height(100),
                )
                .add(column!(
                    Element::from(
                        row!(
                            widget::space::horizontal(),
                            widget::svg(widget::svg::Handle::from_memory(RED_RECT.as_bytes()))
                                .height(20),
                            widget::space::horizontal(),
                            ColorPicker::color_slider(
                                color.red,
                                Message::ColorPickerSliderRedChanged,
                                &COLOR_STOPS_RED_LOW,
                                &COLOR_STOPS_RED_HIGH
                            ),
                            widget::space::horizontal(),
                            widget::text_input("", color.red.to_string())
                                .width(50)
                                .on_input(Message::ColorTextInputRedChanged),
                            widget::space::horizontal(),
                        )
                        .align_y(Alignment::Center)
                    ),
                    Element::from(
                        row!(
                            widget::space::horizontal(),
                            widget::svg(widget::svg::Handle::from_memory(GREEN_RECT.as_bytes()))
                                .height(20),
                            widget::space::horizontal(),
                            ColorPicker::color_slider(
                                color.green,
                                Message::ColorPickerSliderGreenChanged,
                                &COLOR_STOPS_GREEN_LOW,
                                &COLOR_STOPS_GREEN_HIGH
                            ),
                            widget::space::horizontal(),
                            widget::text_input("", color.green.to_string())
                                .width(50)
                                .on_input(Message::ColorTextInputGreenChanged),
                            widget::space::horizontal(),
                        )
                        .align_y(Alignment::Center)
                    ),
                    Element::from(
                        row!(
                            widget::space::horizontal(),
                            widget::svg(widget::svg::Handle::from_memory(BLUE_RECT.as_bytes()))
                                .height(20),
                            widget::space::horizontal(),
                            ColorPicker::color_slider(
                                color.blue,
                                Message::ColorPickerSliderBlueChanged,
                                &COLOR_STOPS_BLUE_LOW,
                                &COLOR_STOPS_BLUE_HIGH
                            ),
                            widget::space::horizontal(),
                            widget::text_input("", color.blue.to_string())
                                .width(50)
                                .on_input(Message::ColorTextInputBlueChanged),
                            widget::space::horizontal(),
                        )
                        .align_y(Alignment::Center)
                    ),
                    Element::from(
                        row!(
                            widget::space::horizontal(),
                            widget::svg(widget::svg::Handle::from_memory(ALPHA_RECT.as_bytes()))
                                .height(20),
                            widget::space::horizontal(),
                            widget::slider(
                                0..=255,
                                color.alpha,
                                Message::ColorPickerSliderAlphaChanged
                            )
                            .width(Length::Fixed(220.0))
                            .step(1),
                            widget::space::horizontal(),
                            widget::text_input("", color.alpha.to_string())
                                .width(50)
                                .on_input(Message::ColorTextInputAlphaChanged),
                            widget::space::horizontal(),
                        )
                        .align_y(Alignment::Center)
                    ),
                ))
                .add(fields)
                .add(
                    row!(
                        widget::button::standard(fl!("colorpicker-defaults"))
                            .on_press(Message::ColorPickerDefaults),
                        widget::button::standard(fl!("colorpicker-accent"))
                            .on_press(Message::ColorPickerAccent),
                        row!(
                            widget::space::horizontal(),
                            widget::button::destructive(fl!("colorpicker-cancel"))
                                .on_press(Message::ColorPickerClose(false, dmo.id())),
                            widget::button::suggested(fl!("colorpicker-save"))
                                .on_press(Message::ColorPickerClose(true, dmo.id()))
                        )
                        .width(Length::Fill)
                        .spacing(5)
                        .align_y(Alignment::End)
                    )
                    .padding(5)
                    .spacing(5)
                    .width(Length::Fill),
                );

            c.into()
        } else {
            widget::button::destructive(fl!("colorpicker-cancel"))
                .on_press(Message::ColorPickerClose(false, None))
                .into()
        }
    }
}
