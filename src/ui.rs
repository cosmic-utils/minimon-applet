//! Shared building blocks for the settings popup.
//!
//! The popup follows the COSMIC design language: every page is a stack of
//! `settings::section()` lists, sub pages are reached through `go_next` rows and
//! left again through a back link that stays above the scroll area, and sensors
//! with several readings separate those readings with a tab bar rather than
//! stacking them on one long page.

use std::borrow::Cow;

use cosmic::Element;
use cosmic::cosmic_theme::palette::Srgba;
use cosmic::iced::advanced::text::Wrapping;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, list, settings, text};

use crate::app::Message;
use crate::config::{ChartColors, ChartKind};
use crate::fl;

/// Edge length of the chart preview shown in a sensor page header.
pub const PREVIEW_SIZE: u16 = 48;

/// Widest a preview may get before it starts crowding out the title and the
/// reading it sits next to. Only the stacked bar chart grows past `PREVIEW_SIZE`.
pub const PREVIEW_MAX_WIDTH: u16 = PREVIEW_SIZE * 3;

/// Size of the swatch opening the color picker.
const SWATCH_SIZE: (u16, u16) = (48, 24);

/// Width given to the dropdowns inside settings rows.
const DROPDOWN_WIDTH: u16 = 130;

/// The link back to the parent page, drawn above the scroll area of every sub
/// page so it stays in reach while the page scrolls.
pub fn back_button<'a>(parent: &'a str, on_press: Message) -> Element<'a, Message> {
    widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
        .extra_small()
        .padding(0)
        .label(parent)
        .spacing(4)
        .class(widget::button::ButtonClass::Link)
        .on_press(on_press)
        .into()
}

/// A settings row pairing a label with the control it operates.
///
/// [`settings::item`] lays the label out first and leaves the control whatever
/// room is left, so a translation longer than the English one has its control
/// cut off at the edge of the popup. Here the control is measured first and the
/// label wraps into the space that remains.
pub fn control_row<'a>(
    label: impl Into<Cow<'a, str>> + 'a,
    control: impl Into<Element<'a, Message>> + 'a,
) -> Element<'a, Message> {
    settings::item_row(vec![
        text::body(label)
            .wrapping(Wrapping::Word)
            .width(Length::Fill)
            .into(),
        control.into(),
    ])
    .into()
}

/// Title row of a sensor page: the sensor name, its current reading(s) and a
/// preview of the chart as it is drawn on the panel.
pub fn sensor_header<'a>(
    title: impl Into<Cow<'a, str>> + 'a,
    values: impl IntoIterator<Item = String>,
    preview: Element<'a, Message>,
) -> Element<'a, Message> {
    let values = values
        .into_iter()
        .map(|value| text::body(value).into())
        .collect::<Vec<_>>();

    widget::row::with_capacity(4)
        .push(text::title3(title).wrapping(Wrapping::Word))
        .push(widget::space::horizontal())
        .push(widget::column::with_children(values).align_x(Alignment::End))
        .push(preview)
        .align_y(Alignment::Center)
        .spacing(cosmic::theme::spacing().space_s)
        .into()
}

/// A list row holding a title and a `go-next` chevron, used to navigate into a
/// sub page.
pub fn go_next_row<'a>(
    description: impl Into<Cow<'a, str>> + 'a,
    on_press: Message,
) -> list::ListButton<'a, Message> {
    action_row(description, go_next_icon(), on_press)
}

/// A [`go_next_row`] that also shows the sensor's current reading.
///
/// The reading is the one part of the row that may take the room the other two
/// leave over: the chevron has a fixed size and the title is what the row is
/// looked up by, while a reading is several short groups that wrap onto a
/// second line without becoming harder to read.
pub fn go_next_value_row<'a>(
    description: impl Into<Cow<'a, str>> + 'a,
    value: impl Into<Cow<'a, str>> + 'a,
    on_press: Message,
) -> list::ListButton<'a, Message> {
    list::button(settings::item_row(vec![
        text::body(description).wrapping(Wrapping::Word).into(),
        text::body(value)
            .wrapping(Wrapping::Word)
            .align_x(Alignment::End)
            .width(Length::Fill)
            .into(),
        go_next_icon(),
    ]))
    .on_press(on_press)
}

/// The heart on the row linking to the developer's tip page.
pub fn tip_icon<'a>() -> Element<'a, Message> {
    widget::icon::from_svg_bytes(&include_bytes!("../res/icons/heart.svg")[..])
        .symbolic(true)
        .icon()
        .size(16)
        .into()
}

fn go_next_icon<'a>() -> Element<'a, Message> {
    widget::icon::from_name("go-next-symbolic")
        .size(16)
        .icon()
        .into()
}

/// A list row acting as a button, with a trailing widget hinting at what
/// activating it does.
///
/// The hint is an icon of a fixed size, so it is laid out first and the title
/// wraps into the room that is left, rather than a long translation pushing the
/// icon off the edge of the popup.
pub fn action_row<'a>(
    description: impl Into<Cow<'a, str>> + 'a,
    trailing: impl Into<Element<'a, Message>> + 'a,
    on_press: Message,
) -> list::ListButton<'a, Message> {
    list::button(control_row(description, trailing)).on_press(on_press)
}

/// The `Chart type` row, letting the user pick how the sensor is drawn.
pub fn chart_type_row<'a>(
    options: &'a [&'static str],
    selected: Option<usize>,
    on_select: impl Fn(usize) -> Message + Send + Sync + 'static,
) -> Element<'a, Message> {
    control_row(
        fl!("chart-type"),
        widget::dropdown(options, selected, on_select).width(DROPDOWN_WIDTH),
    )
}

/// The `Chart color` row, opening the color picker for the sensor.
pub fn chart_color_row<'a>(color: Srgba<u8>, on_press: Message) -> Element<'a, Message> {
    control_row(fl!("chart-color"), color_swatch(color, on_press))
}

/// The color a chart of `kind` is best represented by in a [`chart_color_row`].
///
/// Charts draw their samples with `graph1`, except the heat chart, which uses a
/// fixed orange to red gradient and only lets the background and the frame be
/// changed. Previewing `graph1` there would show a color the chart never draws
/// and the color picker cannot reach.
pub fn chart_swatch(colors: &ChartColors, kind: ChartKind) -> Srgba<u8> {
    match kind {
        ChartKind::Heat => colors.background,
        _ => colors.graph1,
    }
}

/// A filled rectangle acting as a button, previewing a chart color.
fn color_swatch<'a>(color: Srgba<u8>, on_press: Message) -> Element<'a, Message> {
    let color = cosmic::iced::Color::from_rgba8(
        color.red,
        color.green,
        color.blue,
        f32::from(color.alpha) / 255.0,
    );

    // The stock swatch is a small square; a wider one reads better next to the
    // chart it stands for.
    widget::color_picker::color_button(Some(on_press), Some(color), Length::Fill)
        .width(SWATCH_SIZE.0)
        .height(SWATCH_SIZE.1)
        .into()
}

/// A dropdown for picking the unit temperatures are displayed in.
pub fn temperature_unit_row<'a>(
    options: &'a [&'static str],
    selected: Option<usize>,
    on_select: impl Fn(usize) -> Message + Send + Sync + 'static,
) -> Element<'a, Message> {
    control_row(
        fl!("temperature-unit"),
        widget::dropdown(options, selected, on_select).width(DROPDOWN_WIDTH),
    )
}

/// A spin button for the temperature the chart starts scaling from.
pub fn min_temperature_row<'a>(
    min_temp: f64,
    on_change: impl Fn(f64) -> Message + 'static,
) -> Element<'a, Message> {
    // Older configs hold a fractional temperature, but the button steps in whole
    // degrees. Rounding what it is given keeps the label in step with the value;
    // showing `42.5` as `42` and `43.5` as `44` reads as a button that is stuck.
    let min_temp = min_temp.round();

    control_row(
        fl!("min-temperature"),
        widget::spin_button(
            format!("{min_temp:.0}"),
            min_temp,
            1.0,
            0.0,
            99.0,
            on_change,
        ),
    )
}
