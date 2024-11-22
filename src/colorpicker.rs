use cosmic::iced::alignment::Horizontal;
use cosmic::iced::Background;
use cosmic::iced::{
    widget::{column, row},
    Alignment,
};
use cosmic::{cosmic_theme::palette::Srgb, Element};
use std::ops::RangeInclusive;
use std::rc::Rc;

use cosmic::{
    iced::{
        gradient::{ColorStop, Linear},
        Color, Length, Radians,
    },
    theme,
    widget::{
        self,
        slider::{self, HandleShape},
    },
};
use theme::iced::Slider;

use crate::app::Message;
use crate::config::{SvgColorVariant, SvgColors, SvgDevKind, SvgGraphKind};

const RED_RECT: &str = "<svg width=\"50\" height=\"50\" xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"50\" height=\"50\" x=\"0\" y=\"0\" rx=\"15\" ry=\"15\" fill=\"red\" /></svg>";
const GREEN_RECT: &str = "<svg width=\"50\" height=\"50\" xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"50\" height=\"50\" x=\"0\" y=\"0\" rx=\"15\" ry=\"15\" fill=\"green\" /></svg>";
const BLUE_RECT: &str = "<svg width=\"50\" height=\"50\" xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"50\" height=\"50\" x=\"0\" y=\"0\" rx=\"15\" ry=\"15\" fill=\"blue\" /></svg>";

use lazy_static::lazy_static;
use std::sync::Mutex;

lazy_static! {
    static ref COLOR_STOPS_RED_LOW: Mutex<[ColorStop; 2]> = Mutex::new([
        ColorStop {
            offset: 0.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
    ]);
    static ref COLOR_STOPS_RED_HIGH: Mutex<[ColorStop; 2]> = Mutex::new([
        ColorStop {
            offset: 0.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgb(1.0, 0.0, 0.0),
        },
    ]);
    static ref COLOR_STOPS_GREEN_LOW: Mutex<[ColorStop; 2]> = Mutex::new([
        ColorStop {
            offset: 0.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
    ]);
    static ref COLOR_STOPS_GREEN_HIGH: Mutex<[ColorStop; 2]> = Mutex::new([
        ColorStop {
            offset: 0.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgb(0.0, 1.0, 0.0),
        },
    ]);
    static ref COLOR_STOPS_BLUE_LOW: Mutex<[ColorStop; 2]> = Mutex::new([
        ColorStop {
            offset: 0.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
    ]);
    static ref COLOR_STOPS_BLUE_HIGH: Mutex<[ColorStop; 2]> = Mutex::new([
        ColorStop {
            offset: 0.0,
            color: Color::from_rgb(0.0, 0.0, 0.0),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgb(0.0, 0.0, 1.0),
        },
    ]);
}

const ERROR: &str = "<svg width=\"800px\" height=\"800px\" viewBox=\"0 0 25 25\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">
<path d=\"M12.5 16V14.5M12.5 9V13M20.5 12.5C20.5 16.9183 16.9183 20.5 12.5 20.5C8.08172 20.5 4.5 16.9183 4.5 12.5C4.5 8.08172 8.08172 4.5 12.5 4.5C16.9183 4.5 20.5 8.08172 20.5 12.5Z\" stroke=\"red\" stroke-width=\"1.2\"/>
</svg>";

pub trait DemoSvg {
    fn svg_demo(&self) -> String;
    fn svg_colors(&self) -> SvgColors;
    fn svg_set_colors(&mut self, colors: SvgColors);
    fn svg_color_choices(&self) -> Vec<(&'static str, SvgColorVariant)>;
}

/// Data for managing the `ColorPicker` dialog
pub struct ColorPicker {
    demo_svg: Option<Box<dyn DemoSvg>>,
    kind: SvgDevKind,
    // Current field being adjusted background/text/etc.
    color_variant: SvgColorVariant,
    ///Current slider values
    slider_red_val: u8,
    slider_green_val: u8,
    slider_blue_val: u8,
}

impl ColorPicker {
    pub fn new() -> Self {
        ColorPicker {
            demo_svg: None,
            kind: SvgDevKind::Cpu(SvgGraphKind::Ring),
            color_variant: SvgColorVariant::Color1,
            slider_red_val: 0,
            slider_green_val: 0,
            slider_blue_val: 0,
        }
    }

    pub fn kind(&self) -> SvgDevKind {
        self.kind
    }

    pub fn active(&self) -> bool {
        self.demo_svg.is_some()
    }

    pub fn activate(&mut self, kind: SvgDevKind, demo_svg: Box<dyn DemoSvg>) {
        self.kind = kind;
        self.color_variant = SvgColorVariant::Color1;
        self.demo_svg = Some(demo_svg);
    }

    pub fn deactivate(&mut self) {
        self.demo_svg = None;
    }

    fn color_slider<'b, Message>(
        range: RangeInclusive<u8>,
        value: u8,
        on_change: impl Fn(u8) -> Message + 'b,
        color_stops_low: &'static Mutex<[ColorStop; 2]>,
        color_stops_high: &'static Mutex<[ColorStop; 2]>,
    ) -> cosmic::Element<'b, Message>
    where
        Message: Clone + 'b,
    {
        widget::slider(range, value, on_change)
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

    pub fn sliders(&self) -> Srgb<u8> {
        Srgb::from_components((
            self.slider_red_val,
            self.slider_green_val,
            self.slider_blue_val,
        ))
    }

    pub fn svg_demo(&self) -> String {
        if let Some(d) = self.demo_svg.as_ref() {
            return d.svg_demo();
        }
        ERROR.to_string()
    }

    pub fn set_sliders(&mut self, color: Srgb<u8>) {
        self.slider_red_val = color.red;
        self.slider_green_val = color.green;
        self.slider_blue_val = color.blue;
        let dmo = self.demo_svg.as_mut().expect("ERROR: No demo svg!");
        let mut col = dmo.svg_colors();
        col.set_color(color, self.color_variant);
        dmo.svg_set_colors(col);

        // Set the shading for sliders, this is required to be static lifetime
        COLOR_STOPS_RED_LOW.lock().unwrap()[1].color =
            Color::from_rgb(color.red as f32 / u8::MAX as f32, 0.0, 0.0);
        COLOR_STOPS_RED_HIGH.lock().unwrap()[0].color =
            Color::from_rgb(color.red as f32 / u8::MAX as f32, 0.0, 0.0);

        COLOR_STOPS_GREEN_LOW.lock().unwrap()[1].color =
            Color::from_rgb(0.0, color.green as f32 / u8::MAX as f32, 0.0);
        COLOR_STOPS_GREEN_HIGH.lock().unwrap()[0].color =
            Color::from_rgb(0.0, color.green as f32 / u8::MAX as f32, 0.0);

        COLOR_STOPS_BLUE_LOW.lock().unwrap()[1].color =
            Color::from_rgb(0.0, 0.0, color.blue as f32 / u8::MAX as f32);
        COLOR_STOPS_BLUE_HIGH.lock().unwrap()[0].color =
            Color::from_rgb(0.0, 0.0, color.blue as f32 / u8::MAX as f32);
    }

    pub fn set_colors(&mut self, colors: SvgColors) {
        let dmo = self.demo_svg.as_mut().expect("ERROR: No demo svg!");
        dmo.svg_set_colors(colors);
        self.set_sliders(colors.get_color(self.color_variant));
    }

    pub fn variant(&self) -> SvgColorVariant {
        self.color_variant
    }

    pub fn set_variant(&mut self, variant: SvgColorVariant) {
        let dmo = self.demo_svg.as_mut().expect("ERROR: No demo svg!");
        self.color_variant = variant;
        let color = dmo.svg_colors().get_color(variant);
        self.set_sliders(color);
    }

    pub fn colors(&self) -> SvgColors {
        let dmo = self.demo_svg.as_ref().expect("ERROR: No demo svg!");
        dmo.svg_colors()
    }

    pub fn view_colorpicker(&self) -> Element<crate::app::Message> {
        let color = self.sliders();
        let title = format!("{} colors", self.kind);

        let mut children = Vec::new();

        let dmo = self.demo_svg.as_ref().expect("ERROR: No demo svg!");
        for (s, c) in dmo.svg_color_choices() {
            children.push(Element::from(widget::radio(
                s,
                c,
                if self.variant() == c { Some(c) } else { None },
                Message::ColorPickerSelectVariant,
            )));
        }

        let fields = cosmic::widget::row::with_children(children);

        let c = widget::list_column()
            .padding(0)
            .spacing(0)
            .add(
                widget::text::title2(title)
                    .width(Length::Fill)
                    .align_x(Horizontal::Center),
            )
            .add(
                widget::svg(widget::svg::Handle::from_memory(
                    self.svg_demo().into_bytes(),
                ))
                .width(Length::Fill)
                .height(100),
            )
            .add(column!(
                Element::from(
                    row!(
                        widget::horizontal_space(),
                        widget::svg(widget::svg::Handle::from_memory(RED_RECT.as_bytes()))
                            .height(20),
                        widget::horizontal_space(),
                        ColorPicker::color_slider(
                            0..=255,
                            color.red,
                            Message::ColorPickerSliderRedChanged,
                            &COLOR_STOPS_RED_LOW,
                            &COLOR_STOPS_RED_HIGH
                        ),
                        widget::horizontal_space(),
                        widget::text_input("", color.red.to_string())
                            .width(50)
                            .on_input(Message::ColorTextInputRedChanged),
                        widget::horizontal_space(),
                    )
                    .align_y(Alignment::Center)
                ),
                Element::from(
                    row!(
                        widget::horizontal_space(),
                        widget::svg(widget::svg::Handle::from_memory(GREEN_RECT.as_bytes()))
                            .height(20),
                        widget::horizontal_space(),
                        ColorPicker::color_slider(
                            0..=255,
                            color.green,
                            Message::ColorPickerSliderGreenChanged,
                            &COLOR_STOPS_GREEN_LOW,
                            &COLOR_STOPS_GREEN_HIGH
                        ),
                        widget::horizontal_space(),
                        widget::text_input("", color.green.to_string())
                            .width(50)
                            .on_input(Message::ColorTextInputGreenChanged),
                        widget::horizontal_space(),
                    )
                    .align_y(Alignment::Center)
                ),
                Element::from(
                    row!(
                        widget::horizontal_space(),
                        widget::svg(widget::svg::Handle::from_memory(BLUE_RECT.as_bytes()))
                            .height(20),
                        widget::horizontal_space(),
                        ColorPicker::color_slider(
                            0..=255,
                            color.blue,
                            Message::ColorPickerSliderBlueChanged,
                            &COLOR_STOPS_BLUE_LOW,
                            &COLOR_STOPS_BLUE_HIGH
                        ),
                        widget::horizontal_space(),
                        widget::text_input("", color.blue.to_string())
                            .width(50)
                            .on_input(Message::ColorTextInputBlueChanged),
                        widget::horizontal_space(),
                    )
                    .align_y(Alignment::Center)
                ),
            ))
            .add(fields)
            .spacing(10)
            .add(
                row!(
                    widget::button::standard("Defaults").on_press(Message::ColorPickerDefaults),
                    row!(
                        widget::horizontal_space(),
                        widget::button::destructive("Cancel")
                            .on_press(Message::ColorPickerClose(false)),
                        widget::button::suggested("Save").on_press(Message::ColorPickerClose(true))
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
    }
}
