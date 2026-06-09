use std::collections::HashMap;
use std::fmt::Write;

use crate::{sensors::cpu::CpuLoad, svg_graph::SvgColors};

#[derive(Debug, Clone, Copy)]
pub struct StackedBarSvg {
    core_width: u16,
    core_height: u16,
    spacing: u16,
    padding: u16,
}

impl Default for StackedBarSvg {
    fn default() -> Self {
        StackedBarSvg {
            core_width: 4,
            core_height: 22, // image height will be core_height+(2*padding)
            spacing: 1,
            padding: 1,
        }
    }
}

impl StackedBarSvg {
    pub fn new(bar_width: u16, chart_height: u16, spacing: u16) -> Self {
        let padding = 1;
        Self {
            core_width: bar_width,
            core_height: chart_height - (padding * 2),
            spacing,
            padding,
        }
    }

    pub fn svg(&self, cores: &HashMap<usize, CpuLoad>, colors: &SvgColors) -> String {
        // Calculate total width based on number of cores
        // Formula: (num_cores * core_width) + ((num_cores - 1) * spacing) + (2 * padding)
        let total_width = self.width(cores.len());
        let total_height = self.height();

        let mut svg = String::new();

        // SVG header with COSMIC-friendly dark theme - width adapts to core count
        let _ = writeln!(
            svg,
            r#"<svg width="{total_width}" height="{total_height}" viewBox="0 0 {total_width} {total_height}" xmlns="http://www.w3.org/2000/svg">"#
        );

        // CSS styles with configurable colors
        let _ = writeln!(
            svg,
            r#"  <defs>
    <style>
      .background {{ fill: {}; stroke: {}; stroke-width: 1; }}
      .user-load {{ fill: {}; }}
      .system-load {{ fill: {}; }}
      .separator {{ fill: {}; }}
    </style>
    <clipPath id="rounded-clip">
    <rect x="0" y="0" width="{total_width}" height="{total_height}" rx="4.5" ry="4.5"/>
  </clipPath>
  </defs>"#,
            colors.background, colors.frame, colors.graph1, colors.graph2, colors.background,
        );

        // Background with adaptive width. The rect is inset by 0.5 SVG units so the
        // 1px stroke is centered on (0.5, 0.5)..(W-0.5, H-0.5), keeping the entire
        // stroke inside the viewBox. Otherwise the outer half of the stroke would
        // render beyond the SVG bounds and paint over the surrounding panel button.
        let bg_w = f32::from(total_width) - 1.0;
        let bg_h = f32::from(total_height) - 1.0;
        let _ = writeln!(
            svg,
            r#"  <g clip-path="url(#rounded-clip)"><rect class="background" x="0.5" y="0.5" width="{bg_w}" height="{bg_h}" rx="4" ry="4"/>"#
        );

        for i in 0..cores.len() {
            if let Some(core) = cores.get(&i) {
                let x_offset = self.padding + (i as u16 * (self.core_width + self.spacing));
                self.generate_core_bar(&mut svg, x_offset, core, i);
                /*
                            // Add 1px separator after each bar (except the last one)
                            if i < cores.len() - 1 {
                                let separator_x = x_offset + self.core_width;
                                writeln!(
                                    svg,
                                    r#"  <rect class="separator" x="{}" y="{}" width="1" height="{}"/>"#,
                                    separator_x, self.padding, self.core_height
                                )
                                .unwrap();
                            }
                */
            }
        }

        let _ = writeln!(svg, "</g></svg>");
        svg
    }

    fn generate_core_bar(
        &self,
        svg: &mut String,
        x_offset: u16,
        core: &CpuLoad,
        _core_index: usize,
    ) {
        let available_height = self.core_height as f64;

        // Calculate heights (clamp to 0-100%)
        let user_percent = core.user_pct.clamp(0.0, 100.0);
        let system_percent = core.system_pct.clamp(0.0, 100.0);
        let total_percent = (user_percent + system_percent).min(100.0);

        let user_height = (available_height * user_percent / 100.0) as u16;
        let system_height = (available_height * system_percent / 100.0) as u16;

        // Calculate positions (bars grow upward from bottom)
        let user_y = self.padding + self.core_height - user_height;
        let system_y = if total_percent <= 100.0 {
            user_y.saturating_sub(system_height)
        } else {
            // If total > 100%, prioritize system time visibility
            self.padding
        };

        // Generate user load bar (bottom)
        if user_height > 0 {
            let _ = writeln!(
                svg,
                r#"  <rect class="user-load" x="{}" y="{}" width="{}" height="{}"/>"#,
                x_offset, user_y, self.core_width, user_height
            );
        }

        // Generate system load bar (top)
        if system_height > 0 {
            let _ = writeln!(
                svg,
                r#"  <rect class="system-load" x="{}" y="{}" width="{}" height="{}"/>"#,
                x_offset, system_y, self.core_width, system_height
            );
        }
    }
}

impl StackedBarSvg {
    // Calculate the total width needed for a given number of cores
    pub fn width(&self, core_count: usize) -> u16 {
        if core_count == 0 {
            (self.padding * 2) + self.core_width // Minimum width
        } else {
            (core_count as u16 * self.core_width)
                + ((core_count.saturating_sub(1)) as u16 * self.spacing)
                + (self.padding * 2)
        }
    }

    pub fn height(&self) -> u16 {
        self.core_height + (self.padding * 2)
    }
}
