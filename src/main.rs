// SPDX-License-Identifier: GPL-3.0-only

use app::Minimon;

mod app;
mod barchart;
mod colorpicker;
mod config;
mod i18n;
mod sensors;
mod svg_graph;
mod system_monitors;

#[cfg(feature = "lyon_charts")]
mod charts;

use chrono::Local;
use log::info;
use std::io;

fn setup_logger() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    {
        // Debug builds: use fern with stdout
        fern::Dispatch::new()
            .level(log::LevelFilter::Warn)
            .level_for("cosmic_applet_minimon", log::LevelFilter::Debug)
            .format(|out, message, record| {
                out.finish(format_args!(
                    "{} [{}] {}",
                    Local::now().format("%H:%M:%S"),
                    record.level(),
                    message
                ));
            })
            .chain(io::stdout())
            .apply()?;
    }

    // In release builds we log to the systemd journal with fern/stdout fallback
    // To retrieve logs use "journalctl SYSLOG_IDENTIFIER=cosmic-applet-minimon"
    #[cfg(not(debug_assertions))]
    {
        let dispatch = fern::Dispatch::new()
            .level(log::LevelFilter::Warn)
            .level_for("cosmic_applet_minimon", log::LevelFilter::Debug);

        // Try to use systemd journal first
        match systemd_journal_logger::JournalLog::new() {
            Ok(journal_logger) => {
                let journal_logger = journal_logger.with_extra_fields(vec![
                    ("VERSION", env!("CARGO_PKG_VERSION")),
                    ("APPLET", "cosmic_applet_minimon"),
                ]);

                dispatch
                    .chain(Box::new(journal_logger) as Box<dyn log::Log>)
                    .apply()?;
            }
            Err(_) => {
                // Fallback to same fern logging as debug builds
                fern::Dispatch::new()
                    .level(log::LevelFilter::Warn)
                    .level_for("cosmic_applet_minimon", log::LevelFilter::Debug)
                    .format(|out, message, record| {
                        out.finish(format_args!(
                            "{} [{}] {}",
                            Local::now().format("%H:%M:%S"),
                            record.level(),
                            message
                        ));
                    })
                    .chain(io::stdout())
                    .apply()?;
            }
        }
    }

    Ok(())
}

fn main() -> cosmic::iced::Result {
    setup_logger().expect("Failed to initialize logger");

    #[cfg(not(debug_assertions))]
    println!(
        "In Release builds use 'journalctl SYSLOG_IDENTIFIER=cosmic-applet-minimon' to see logs"
    );

    info!("Application started");

    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);
    cosmic::applet::run::<Minimon>(())
}
