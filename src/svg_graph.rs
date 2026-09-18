use std::collections::VecDeque;

use cosmic::cosmic_theme::palette::Srgba;

use crate::config::ChartColors;

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgColors {
    pub background: String,
    pub frame: String,
    pub text: String,
    pub graph1: String,
    pub graph2: String,
    pub graph3: String,
}

impl From<ChartColors> for SvgColors {
    fn from(graph_colors: ChartColors) -> Self {
        fn to_hex(color: Srgba<u8>) -> String {
            format!(
                "#{:02X}{:02X}{:02X}{:02X}",
                color.red, color.green, color.blue, color.alpha
            )
        }

        SvgColors {
            background: to_hex(graph_colors.background),
            frame: to_hex(graph_colors.frame),
            text: to_hex(graph_colors.text),
            graph1: to_hex(graph_colors.graph1),
            graph2: to_hex(graph_colors.graph2),
            graph3: to_hex(graph_colors.graph3),
        }
    }
}

impl SvgColors {
    pub fn new(colors: &ChartColors) -> SvgColors {
        (*colors).into()
    }

    pub fn set_colors(&mut self, colors: &ChartColors) {
        *self = (*colors).into();
    }
}

fn clip_path_for_ram_fill(percentage: u8) -> String {
    fn clip_rect_svg(percentage: u8, cx: f64, cy: f64, r: f64) -> String {
        let pct = (percentage as f64).clamp(0.0, 100.0);

        // Rectangle that clips the green circle:
        let width = 2.0 * r;
        let height = (pct / 100.0) * (2.0 * r);
        let x = cx - r;
        let y = (cy + r) - height;

        // round a bit so the string is tidy
        let f = |v: f64| format!("{:.4}", v);

        format!(
            r#"<defs><clipPath id="bottom-half">
  <rect x="{x}" y="{y}" width="{w}" height="{h}" />
</clipPath></defs>"#,
            x = f(x),
            y = f(y),
            w = f(width),
            h = f(height),
        )
    }

    clip_rect_svg(percentage, 17.0, 17.0, 12.9155)
}

pub fn ring(value1: &str, percentage1: u8, percentage2: Option<u8>, color: &SvgColors) -> String {
    let mut svg = String::with_capacity(RINGSVG_LEN);
    svg.push_str(RINGSVG_1);
    svg.push_str(&color.background);
    svg.push_str(RINGSVG_1_1);
    svg.push_str(&color.graph2);
    svg.push_str(RINGSVG_2);
    svg.push_str(&color.graph1);
    svg.push_str(RINGSVG_3);
    svg.push_str(&percentage1.to_string());
    svg.push_str(RINGSVG_4);
    if let Some(pct2) = percentage2 {
        svg.push_str(&clip_path_for_ram_fill(pct2));
        svg.push_str(RINGSVG_4_3);
        svg.push_str(&color.background);
        svg.push_str(RINGSVG_4_4);
        svg.push_str(&color.graph3);
        svg.push_str(RINGSVG_4_5);
    }
    svg.push_str(RINGSVG_5);
    svg.push_str(&color.text);
    svg.push_str(RINGSVG_6);
    svg.push_str(value1);
    svg.push_str(RINGSVG_7);
    svg
}

//stroke, dashoffset,dasharray

pub fn line(samples: &VecDeque<f64>, max_y: f64, colors: &SvgColors) -> String {
    // Generate list of coordinates for line

    let scaling: f32 = 40.0 / max_y as f32;
    let est_len = samples.len() * 10; // Rough estimate: each pair + separator

    let indexed_string = samples.iter().enumerate().fold(
        String::with_capacity(est_len),
        |mut acc, (index, &value)| {
            let x = ((index * 2) + 1) as u32;
            let y = (41.0 - (scaling * value as f32)).round() as u32;
            if index > 0 {
                acc.push(' ');
            }
            let _ = write!(&mut acc, "{x},{y}");
            acc
        },
    );

    let mut svg = String::with_capacity(LINE_LEN);
    svg.push_str(LINESVG_1);
    svg.push_str(&colors.background);
    svg.push_str(LINESVG_2);
    svg.push_str(&colors.frame);
    svg.push_str(LINESVG_3);
    svg.push_str(LINESVG_4);
    svg.push_str(&colors.graph1[..colors.graph1.len() - 2]);
    svg.push_str(LINESVG_5);
    svg.push_str(&indexed_string);
    svg.push_str(LINESVG_6);
    svg.push_str(&colors.graph1);
    svg.push_str(LINESVG_7);
    svg.push_str(&indexed_string);
    svg.push_str(LINESVG_8);
    svg.push_str(LINESVG_FRAME_START);
    svg.push_str(&colors.frame);
    svg.push_str(LINESVG_FRAME_END);
    svg.push_str(LINESVG_9);

    svg
}

/// Three readings on a shared scale, with non-overlapping colored bands.
pub fn triple_line(
    samples: [&VecDeque<f64>; 3],
    max_y: f64,
    colors: &ChartColors,
    threshold: Option<f64>,
) -> String {
    let band_colors = [colors.graph1, colors.graph2, colors.graph3].map(band_fill_color);
    let colors = SvgColors::new(colors);
    let threshold = threshold.filter(|value| value.is_finite() && *value > 0.0);
    let max_y = max_y.max(threshold.unwrap_or(0.0) * 1.15).max(1.0);
    let mut svg = String::from(LINESVG_1);
    svg.push_str(&colors.background);
    svg.push_str(LINESVG_2);
    svg.push_str(&colors.frame);
    svg.push_str(LINESVG_3);

    let points = samples.map(|series| {
        let mut points = String::with_capacity(series.len() * 12);
        for (index, value) in series.iter().enumerate() {
            let x = index * 2 + 1;
            let y = 41.0 - 40.0 * value.clamp(0.0, max_y) / max_y;
            if index > 0 {
                points.push(' ');
            }
            let _ = write!(points, "{x},{y:.2}");
        }
        points
    });
    let graph_colors = [&colors.graph1, &colors.graph2, &colors.graph3];

    // Each band belongs to its upper boundary. Split at crossings so the
    // ownership follows the curves instead of depending on their drawing order.
    let mut bands: [String; 3] = std::array::from_fn(|_| String::new());
    for band in line_bands(samples, max_y) {
        let [(x0, y0), (x1, y1), (x2, y2), (x3, y3)] = band.points;
        let _ = write!(
            bands[band.series],
            "M{x0:.4},{y0:.4} L{x1:.4},{y1:.4} L{x2:.4},{y2:.4} L{x3:.4},{y3:.4} Z "
        );
    }
    for (path, fill) in bands.iter().zip(&band_colors) {
        if !path.is_empty() {
            let _ = write!(svg, r#"<path fill="{fill}" d="{path}"/>"#);
        }
    }
    for (points, color) in points.iter().zip(graph_colors) {
        if points.is_empty() {
            continue;
        }
        svg.push_str(LINESVG_4);
        svg.push_str(color);
        svg.push_str(LINESVG_5);
        svg.push_str(points);
        svg.push_str(r#""/>"#);
    }
    if let Some(value) = threshold {
        let y = 41.0 - 40.0 * value / max_y;
        // A dark outline keeps the neutral marker visible over any curve color.
        let _ = write!(
            svg,
            r##"<g fill="none" stroke-dasharray="2 1.5"><path d="M1,{y:.2} H41" stroke="#202020" stroke-width="1.3"/><path d="M1,{y:.2} H41" stroke="#EEEEEE" stroke-width="0.65"/></g>"##
        );
    }
    svg.push_str(LINESVG_FRAME_START);
    svg.push_str(&colors.frame);
    svg.push_str(LINESVG_FRAME_END);
    svg.push_str(LINESVG_9);
    svg
}

/// Opaque shading for a curve's band, computed before SVG encoding.
fn band_fill_color(color: Srgba<u8>) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        color.red / 2,
        color.green / 2,
        color.blue / 2
    )
}

#[derive(Debug)]
struct LineBand {
    series: usize,
    points: [(f64, f64); 4],
}

fn line_bands(samples: [&VecDeque<f64>; 3], max_y: f64) -> Vec<LineBand> {
    let mut bands = Vec::new();
    let count = samples.iter().map(|series| series.len()).min().unwrap_or(0);
    let y = |value: f64| 41.0 - 40.0 * value / max_y;
    for index in 0..count.saturating_sub(1) {
        let left = samples.map(|series| series[index].clamp(0.0, max_y));
        let right = samples.map(|series| series[index + 1].clamp(0.0, max_y));
        let at = |series: usize, t: f64| left[series] + (right[series] - left[series]) * t;
        let mut splits = vec![0.0, 1.0];
        for a in 0..3 {
            for b in a + 1..3 {
                let delta = (right[a] - left[a]) - (right[b] - left[b]);
                if delta != 0.0 {
                    let t = (left[b] - left[a]) / delta;
                    if t > 0.0 && t < 1.0 {
                        splits.push(t);
                    }
                }
            }
        }
        splits.sort_by(f64::total_cmp);
        splits.dedup();
        for interval in splits.windows(2) {
            let (start, end) = (interval[0], interval[1]);
            let middle = (start + end) / 2.0;
            let mut order = [0, 1, 2];
            order.sort_by(|a, b| at(*a, middle).total_cmp(&at(*b, middle)));
            let x0 = index as f64 * 2.0 + 1.0 + start * 2.0;
            let x1 = index as f64 * 2.0 + 1.0 + end * 2.0;
            let mut lower = (0.0, 0.0);
            for series in order {
                let upper = (at(series, start), at(series, end));
                if upper != lower {
                    bands.push(LineBand {
                        series,
                        points: [
                            (x0, y(lower.0)),
                            (x1, y(lower.1)),
                            (x1, y(upper.1)),
                            (x0, y(upper.0)),
                        ],
                    });
                }
                lower = upper;
            }
        }
    }
    bands
}

pub fn line_stacked(
    samples_used: &VecDeque<f64>,
    samples_allocated: &VecDeque<f64>,
    max_y: f64,
    colors: &SvgColors,
) -> String {
    let scaling: f32 = if max_y > 0.0 {
        40.0 / max_y as f32
    } else {
        0.0
    };

    let n = samples_used.len().min(samples_allocated.len());
    if n == 0 || scaling == 0.0 {
        return String::new();
    }

    // Build forward point lists, and keep used points for reverse walk
    let est_len = n * 10;
    let mut pts_used_fwd = String::with_capacity(est_len);
    let mut pts_alloc_fwd = String::with_capacity(est_len);
    let mut used_points: Vec<(u32, u32)> = Vec::with_capacity(n);

    for (index, (u, a)) in samples_used
        .iter()
        .zip(samples_allocated.iter())
        .enumerate()
    {
        let x = ((index * 2) + 1) as u32;

        let u_clamped = u.max(0.0).min(max_y);
        let a_clamped = a.max(0.0).min(max_y);
        // ensure top >= bottom for a valid ribbon
        let top = a_clamped.max(u_clamped);
        let bot = u_clamped;

        let y_used = (41.0 - (scaling * bot as f32)).round().clamp(1.0, 41.0) as u32;
        let y_alloc = (41.0 - (scaling * top as f32)).round().clamp(1.0, 41.0) as u32;

        if index > 0 {
            pts_used_fwd.push(' ');
            pts_alloc_fwd.push(' ');
        }
        let _ = write!(&mut pts_used_fwd, "{x},{y_used}");
        let _ = write!(&mut pts_alloc_fwd, "{x},{y_alloc}");
        used_points.push((x, y_used));
    }

    // Build the ribbon polygon: forward(alloc) + reverse(used)
    let mut poly_band = String::with_capacity(pts_alloc_fwd.len() + pts_used_fwd.len() + 8);
    poly_band.push_str(&pts_alloc_fwd);
    for (x, y) in used_points.iter().rev() {
        poly_band.push(' ');
        let _ = write!(&mut poly_band, "{x},{y}");
    }

    let mut svg = String::with_capacity(LINE_LEN);

    // Frame + background
    svg.push_str(LINESVG_1);
    svg.push_str(&colors.background);
    svg.push_str(LINESVG_2);
    svg.push_str(&colors.frame);
    svg.push_str(LINESVG_3);

    svg.push_str(LINESVG_4);
    svg.push_str(&colors.graph1[..colors.graph1.len() - 2]);
    svg.push_str(LINESVG_5);
    svg.push_str(&pts_used_fwd);

    svg.push_str(LINESVG_6);
    svg.push_str(&colors.graph1);
    svg.push_str(LINESVG_7);
    svg.push_str(&pts_used_fwd);
    svg.push_str(LINESVG_8);

    svg.push_str(r#"<polygon fill=""#);
    svg.push_str(&colors.graph3);
    svg.push_str(r#"" points=""#);
    svg.push_str(&poly_band);
    svg.push_str(r#""/>"#);

    svg.push_str(r#"<polyline fill="none" stroke=""#);
    svg.push_str(&colors.graph3[..colors.graph3.len() - 2]);
    svg.push_str(r#"" stroke-width="1" points=""#);
    svg.push_str(&pts_alloc_fwd);
    svg.push_str(r#""/>"#);
    svg.push_str(LINESVG_FRAME_START);
    svg.push_str(&colors.frame);
    svg.push_str(LINESVG_FRAME_END);

    svg.push_str(LINESVG_9);
    svg
}

pub fn double_line(
    samples: &VecDeque<u64>,
    samples2: &VecDeque<u64>,
    graph_samples: usize,
    colors: &SvgColors,
    max_y: Option<u64>,
) -> String {
    assert!(samples.len() == samples2.len());

    let len = samples.len();

    let start = len.saturating_sub(graph_samples);

    let max = max_y.unwrap_or_else(|| {
        let calculated_max = samples
            .iter()
            .chain(samples2.iter())
            .copied()
            .max()
            .unwrap_or(40);
        std::cmp::max(40, calculated_max) // Ensure min value is 40
    });

    // Generate list of coordinates for line
    let est_len = (samples.len() - start) * 10;
    let scaling: f64 = 40.0 / max as f64;
    let (indexed_string, indexed_string2) = samples
        .iter()
        .skip(start)
        .zip(samples2.iter().skip(start))
        .enumerate()
        .fold(
            (
                String::with_capacity(est_len),
                String::with_capacity(est_len),
            ),
            |(mut acc1, mut acc2), (index, (&value1, &value2))| {
                let x = ((index * 2) + 1) as u32;
                let y1 = (41.0 - (scaling * value1 as f64)).round() as u32;
                let y2 = (41.0 - (scaling * value2 as f64)).round() as u32;
                let _ = write!(&mut acc1, "{x},{y1} ");
                let _ = write!(&mut acc2, "{x},{y2} ");
                (acc1, acc2)
            },
        );

    let mut svg = String::with_capacity(DBLLINESVG_LEN);
    svg.push_str(DBLLINESVG_1);
    svg.push_str(&colors.background);
    svg.push_str(DBLLINESVG_2);
    svg.push_str(&colors.frame);
    svg.push_str(DBLLINESVG_3);

    //First graph and polygon
    svg.push_str(DBLLINESVG_4);
    svg.push_str(&colors.graph1[..colors.graph1.len() - 2]);
    svg.push_str(DBLLINESVG_5);
    svg.push_str(&indexed_string);
    svg.push_str(DBLLINESVG_6);
    svg.push_str(&colors.graph1);
    svg.push_str(DBLLINESVG_7);
    svg.push_str(&indexed_string);
    svg.push_str(DBLLINESVG_8);

    //Second graph and polygon
    svg.push_str(DBLLINESVG_4);
    svg.push_str(&colors.graph2[..colors.graph2.len() - 2]);
    svg.push_str(DBLLINESVG_5);
    svg.push_str(&indexed_string2);
    svg.push_str(DBLLINESVG_6);
    svg.push_str(&colors.graph2);
    svg.push_str(DBLLINESVG_7);
    svg.push_str(&indexed_string2);
    svg.push_str(DBLLINESVG_8);
    svg.push_str(LINESVG_FRAME_START);
    svg.push_str(&colors.frame);
    svg.push_str(LINESVG_FRAME_END);

    svg.push_str(DBLLINESVG_9);

    svg
}

pub fn line_adaptive(
    samples: &VecDeque<u64>,
    graph_samples: usize,
    colors: &SvgColors,
    max_y: Option<u64>,
) -> String {
    let len = samples.len();
    let start = len.saturating_sub(graph_samples);

    let max = max_y.unwrap_or_else(|| {
        let calculated_max = samples.iter().copied().max().unwrap_or(40);
        std::cmp::max(40, calculated_max) // Ensure min value is 40
    });

    // Generate list of coordinates for line
    let est_len = (samples.len() - start) * 10;
    let scaling: f64 = 40.0 / max as f64;
    let indexed_string = samples.iter().skip(start).enumerate().fold(
        String::with_capacity(est_len),
        |mut acc, (index, &value)| {
            let x = ((index * 2) + 1) as u32;
            let y = (41.0 - (scaling * value as f64)).round() as u32;
            let _ = write!(&mut acc, "{x},{y} ");
            acc
        },
    );

    let mut svg = String::with_capacity(DBLLINESVG_LEN);
    svg.push_str(DBLLINESVG_1);
    svg.push_str(&colors.background);
    svg.push_str(DBLLINESVG_2);
    svg.push_str(&colors.frame);
    svg.push_str(DBLLINESVG_3);

    //First graph and polygon
    svg.push_str(DBLLINESVG_4);
    svg.push_str(&colors.graph1[..colors.graph1.len() - 2]);
    svg.push_str(DBLLINESVG_5);
    svg.push_str(&indexed_string);
    svg.push_str(DBLLINESVG_6);
    svg.push_str(&colors.graph1);
    svg.push_str(DBLLINESVG_7);
    svg.push_str(&indexed_string);
    svg.push_str(DBLLINESVG_8);
    svg.push_str(LINESVG_FRAME_START);
    svg.push_str(&colors.frame);
    svg.push_str(LINESVG_FRAME_END);

    svg.push_str(DBLLINESVG_9);

    svg
}

pub fn heat(samples: &VecDeque<f64>, max_y: u64, colors: &SvgColors) -> String {
    // Generate list of coordinates for line

    let scaling: f32 = 40.0 / max_y as f32;
    let est_len = samples.len() * 10; // Rough estimate: each pair + separator

    let indexed_string = samples.iter().enumerate().fold(
        String::with_capacity(est_len),
        |mut acc, (index, &value)| {
            let x = ((index * 2) + 1) as u32;
            let y = (41.0 - (scaling * value as f32)).round() as u32;
            if index > 0 {
                acc.push(' ');
            }
            let _ = write!(&mut acc, "{x},{y}");
            acc
        },
    );

    let mut svg = String::with_capacity(LINE_LEN);
    svg.push_str(HEATSVG_1);
    svg.push_str(&colors.background);
    svg.push_str(HEATSVG_2);
    svg.push_str(&colors.frame);
    svg.push_str(HEATSVG_3);
    svg.push_str(&indexed_string);
    svg.push_str(HEATSVG_8);
    svg.push_str(&colors.frame);
    svg.push_str(HEATSVG_9);

    svg
}

const HEATSVG_1: &str = r#"<svg width="42" height="42" viewBox="0 0 42 42" xmlns="http://www.w3.org/2000/svg">
  <defs>
  <linearGradient id="temp-gradient" x1="0" y1="42" x2="0" y2="0" gradientUnits="userSpaceOnUse">
      <stop offset="0%" stop-color="orange"/>
      <stop offset="90%" stop-color="red"/>
    </linearGradient>
    <clipPath id="rounded-clip">
      <rect x="0" y="0" width="42" height="42" rx="7" ry="7"/>
    </clipPath>
  </defs>
  <g clip-path="url(#rounded-clip)">
    <rect x="0" y="0" rx="7" ry="7" width="42" height="42" fill=""#; // background color placeholder

const HEATSVG_2: &str = r#"" stroke=""#; // frame color placeholder
const HEATSVG_3: &str = r#""/><polygon fill="url(#temp-gradient)" points=""#;
const HEATSVG_8: &str = r#"  41,41 1,41"/><rect x="0" y="0" rx="7" ry="7" width="42" height="42" fill="rgba(0,0,0,0)" stroke=""#;
const HEATSVG_9: &str = r#""/></g></svg>"#;

const LINESVG_1: &str = r#"<svg width="42" height="42" viewBox="0 0 42 42" xmlns="http://www.w3.org/2000/svg">
<defs>
  <clipPath id="rounded-clip">
    <rect x="0" y="0" width="42" height="42" rx="7" ry="7"/>
  </clipPath>
</defs>
<g clip-path="url(#rounded-clip)">
  <rect x="0" y="0" rx="7" ry="7" width="42" height="42" fill=""#; // background color placeholder

const LINESVG_2: &str = r#"" stroke=""#; // frame color placeholder
const LINESVG_3: &str = r#""/>"#;
const LINESVG_4: &str = r#"<polyline fill="none" stroke=""#; // line color placeholder
const LINESVG_5: &str = r#"" stroke-width="1" points=""#;
const LINESVG_6: &str = r#""/><polygon fill=""#; // polygon color placeholder
const LINESVG_7: &str = r#"" points=""#;
const LINESVG_8: &str = r#"  41,41 1,41"/>"#;
const LINESVG_9: &str = r#"</g></svg>"#;

const LINESVG_FRAME_START: &str =
    r#"<rect x="0" y="0" rx="7" ry="7" width="42" height="42" fill="none" stroke=""#;
const LINESVG_FRAME_END: &str = r#""/>"#;

const LINE_LEN: usize = 640; // Just for preallocation
// Ring SVG
const RINGSVG_1: &str = r#"
<svg viewBox="0 0 34 34" xmlns="http://www.w3.org/2000/svg">
 <path
    d="M17 1.0845
      a 15.9155 15.9155 0 0 1 0 31.831
      a 15.9155 15.9155 0 0 1 0 -31.831"
      fill=""#;

const RINGSVG_1_1: &str = r#"" stroke=""#;

const RINGSVG_2: &str = r#""
stroke-width="2"
/>
<path
  d="M17 32.831
    a 15.9155 15.9155 0 0 1 0 -31.831
    a 15.9155 15.9155 0 0 1 0 31.831"
  fill="none"
  stroke=""#;

const RINGSVG_3: &str = r#""
  stroke-width="2"
  stroke-dasharray=""#;

const RINGSVG_4: &str = r#", 100"
/>"#;

const RINGSVG_4_3: &str = r#"
  <circle cx="17" cy="17" r="12.9155" fill=""#;

const RINGSVG_4_4: &str = r#"" />
  <circle cx="17" cy="17" r="12.9155" fill=""#;

const RINGSVG_4_5: &str = r#"" clip-path="url(#bottom-half)" />"#;

const RINGSVG_5: &str = r#"<style>
.percentage {
 fill: "#;

const RINGSVG_6: &str = r#";
  font-family: "Noto Sans", sans-serif;
  font-size: 1.2em;
  text-anchor: middle;
}
</style>
<text x="17" y="22.35" class="percentage">"#;

const RINGSVG_7: &str = r#"</text></svg>"#;

const RINGSVG_LEN: usize = 680; // For preallocation

// Double Line SVG
const DBLLINESVG_1: &str = r#"
<svg width="42" height="42" viewBox="0 0 42 42" xmlns="http://www.w3.org/2000/svg">
<defs>
  <clipPath id="rounded-clip">
    <rect x="0" y="0" width="42" height="42" rx="7" ry="7"/>
  </clipPath>
</defs>
<g clip-path="url(#rounded-clip)">
<rect x="0" y="0" width="42" height="42" rx="7" ry="7" fill=""#;

const DBLLINESVG_2: &str = r#"" stroke=""#;
const DBLLINESVG_3: &str = r#""/>
"#;

// Line
const DBLLINESVG_4: &str = r#"<polyline fill="none" stroke=""#;
const DBLLINESVG_5: &str = r#"" stroke-width="1" points=""#;

// Polygon
const DBLLINESVG_6: &str = r#""/>
<polygon fill=""#;
const DBLLINESVG_7: &str = r#"" points=""#;
const DBLLINESVG_8: &str = r#"  41,41 1,41"/>"#;

const DBLLINESVG_9: &str = r#"</g></svg>"#;

const DBLLINESVG_LEN: usize = 1000; // For preallocation

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_stays_visible_and_is_drawn_above_curves() {
        let colors = ChartColors::new(
            crate::config::DeviceKind::SystemLoad,
            crate::config::ChartKind::Line,
        );
        let series = VecDeque::from([0.0, 8.0]);
        for (maximum, expected_y) in [(1.0, "6.22"), (16.0, "21.00")] {
            let svg = triple_line([&series; 3], maximum, &colors, Some(8.0));
            assert!(svg.contains(&format!("M1,{expected_y} H41")));
            assert!(svg.rfind("<polyline").unwrap() < svg.find("stroke-dasharray").unwrap());
        }
        let svg = triple_line([&series; 3], 8.0, &colors, None);
        assert!(!svg.contains("stroke-dasharray"));
    }

    #[test]
    fn triple_line_keeps_strokes_and_uses_opaque_darker_bands() {
        let colors = ChartColors::new(
            crate::config::DeviceKind::SystemLoad,
            crate::config::ChartKind::Line,
        );
        let svg = triple_line(
            [
                &VecDeque::from([1.0, 1.0]),
                &VecDeque::from([4.0, 4.0]),
                &VecDeque::from([2.0, 2.0]),
            ],
            4.0,
            &colors,
            None,
        );
        assert_eq!(svg.matches("<polyline").count(), 3);
        assert_eq!(svg.matches("<path fill=").count(), 3);
        assert!(!svg.contains("fill-opacity"));
        assert!(svg.rfind("<path").unwrap() < svg.find("<polyline").unwrap());
        let svg_colors = SvgColors::new(&colors);
        for (color, stroke) in [colors.graph1, colors.graph2, colors.graph3]
            .into_iter()
            .zip([&svg_colors.graph1, &svg_colors.graph2, &svg_colors.graph3])
        {
            assert!(svg.contains(&format!(r#"<path fill="{}""#, band_fill_color(color))));
            assert!(svg.contains(&format!(r#"stroke="{stroke}""#)));
        }
    }

    #[test]
    fn fill_bands_follow_the_upper_curve_even_when_curves_cross() {
        let samples = [
            VecDeque::from([1.0, 4.0]),
            VecDeque::from([4.0, 1.0]),
            VecDeque::from([3.0, 3.0]),
        ];
        let bands = line_bands([&samples[0], &samples[1], &samples[2]], 4.0);
        // At each point, exactly the nearest curve above it owns the fill.
        for step in 1..40 {
            let t = step as f64 / 40.0;
            let x = 1.0 + 2.0 * t;
            let values = samples.each_ref().map(|s| s[0] + (s[1] - s[0]) * t);
            for level in 1..40 {
                let value = level as f64 / 10.0;
                if values.iter().any(|v| (v - value).abs() < 1e-8) {
                    continue;
                }
                let expected = (0..3)
                    .filter(|i| values[*i] > value)
                    .min_by(|a, b| values[*a].total_cmp(&values[*b]));
                let y = 41.0 - 10.0 * value;
                let owners: Vec<_> = bands
                    .iter()
                    .filter(|band| {
                        let [a, b, c, d] = band.points;
                        if x < a.0 || x >= b.0 {
                            return false;
                        }
                        let u = (x - a.0) / (b.0 - a.0);
                        let bottom = a.1 + (b.1 - a.1) * u;
                        let top = d.1 + (c.1 - d.1) * u;
                        y > top && y < bottom
                    })
                    .map(|band| band.series)
                    .collect();
                if let Some(expected) = expected {
                    assert_eq!(owners.len(), 1, "x={x}, load={value}");
                    // At an exact crossing either tied curve can own the band.
                    assert!(
                        (values[owners[0]] - values[expected]).abs() < 1e-8,
                        "wrong band color at x={x}, load={value}"
                    );
                } else {
                    assert!(
                        owners.is_empty(),
                        "fill above every curve at x={x}, load={value}"
                    );
                }
            }
        }
    }

    #[test]
    fn flat_bands_match_the_requested_blue_orange_green_order() {
        let bands = line_bands(
            [
                &VecDeque::from([1.0, 1.0]),
                &VecDeque::from([4.0, 4.0]),
                &VecDeque::from([2.0, 2.0]),
            ],
            4.0,
        );
        assert_eq!(
            bands.iter().map(|b| b.series).collect::<Vec<_>>(),
            [0, 2, 1]
        );
        assert_eq!(
            bands[0].points,
            [(1.0, 41.0), (3.0, 41.0), (3.0, 31.0), (1.0, 31.0)]
        );
        assert_eq!(
            bands[1].points,
            [(1.0, 31.0), (3.0, 31.0), (3.0, 21.0), (1.0, 21.0)]
        );
        assert_eq!(
            bands[2].points,
            [(1.0, 21.0), (3.0, 21.0), (3.0, 1.0), (1.0, 1.0)]
        );
    }
}
