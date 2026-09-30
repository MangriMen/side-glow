//! Minimal logger: stderr in debug builds, `%APPDATA%\SideGlow\sideglow.log` in release
//! builds (which have no console).

use log::{Level, LevelFilter, Log, Metadata, Record};
use parking_lot::Mutex;
use std::io::Write;

struct Logger {
    file: Option<Mutex<std::fs::File>>,
    level: LevelFilter,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
            // Our own messages, plus warnings from dependencies.
            && (metadata.target().starts_with(env!("CARGO_CRATE_NAME"))
                || metadata.level() <= Level::Warn)
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "[{:<5} {}] {}\n",
            record.level(),
            record.target(),
            record.args()
        );
        match &self.file {
            Some(file) => {
                let _ = file.lock().write_all(line.as_bytes());
            }
            None => eprint!("{line}"),
        }
    }

    fn flush(&self) {
        if let Some(file) = &self.file {
            let _ = file.lock().flush();
        }
    }
}

pub fn init() {
    let level = match std::env::var("SIDEGLOW_LOG").as_deref() {
        Ok("trace") => LevelFilter::Trace,
        Ok("debug") => LevelFilter::Debug,
        Ok("warn") => LevelFilter::Warn,
        Ok("error") => LevelFilter::Error,
        _ => LevelFilter::Info,
    };
    let file = if cfg!(debug_assertions) {
        None
    } else {
        crate::config::store::app_dir().and_then(|dir| {
            std::fs::create_dir_all(&dir).ok()?;
            std::fs::File::create(dir.join("sideglow.log")).ok()
        })
    };
    let logger = Logger {
        file: file.map(Mutex::new),
        level,
    };
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(level);
    }
}
