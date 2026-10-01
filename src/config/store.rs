//! Loading and debounced saving of `%APPDATA%\SideGlow\config.toml`.

use super::Config;
use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const FILE_NAME: &str = "config.toml";
const SAVE_DELAY: Duration = Duration::from_millis(500);

pub fn app_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("SideGlow"))
}

pub struct ConfigStore {
    path: Option<PathBuf>,
    saved: Config,
    dirty_since: Option<Instant>,
}

impl ConfigStore {
    /// Loads the config, falling back to defaults when the file is missing or broken.
    pub fn load() -> (Self, Config) {
        let path = app_dir().map(|dir| dir.join(FILE_NAME));
        let config = path.as_deref().map(load_from).unwrap_or_default();
        let store = Self {
            path,
            saved: config.clone(),
            dirty_since: None,
        };
        (store, config)
    }

    /// Saves `config` once it has stayed unchanged for a short while, so dragging a
    /// slider doesn't hit the disk on every frame. Returns when to call again.
    pub fn tick(&mut self, config: &Config) -> Option<Duration> {
        if *config == self.saved {
            self.dirty_since = None;
            return None;
        }
        let since = *self.dirty_since.get_or_insert_with(Instant::now);
        let elapsed = since.elapsed();
        if elapsed < SAVE_DELAY {
            return Some(SAVE_DELAY - elapsed);
        }
        self.flush(config);
        None
    }

    pub fn flush(&mut self, config: &Config) {
        if *config == self.saved {
            return;
        }
        if let Some(path) = &self.path
            && let Err(err) = save_to(path, config)
        {
            log::error!("failed to save config: {err:#}");
        }
        self.saved = config.clone();
        self.dirty_since = None;
    }
}

fn load_from(path: &Path) -> Config {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Config::default(),
        Err(err) => {
            log::error!("failed to read {}: {err}", path.display());
            return Config::default();
        }
    };
    match parse(&text) {
        Ok(config) => config,
        Err(err) => {
            let backup = path.with_extension("toml.bak");
            log::error!(
                "invalid config {}: {err}; moving it to {}",
                path.display(),
                backup.display()
            );
            let _ = std::fs::rename(path, backup);
            Config::default()
        }
    }
}

fn parse(text: &str) -> Result<Config, toml::de::Error> {
    let mut config: Config = toml::from_str(text)?;
    config.sanitize();
    Ok(config)
}

fn save_to(path: &Path, config: &Config) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let text = toml::to_string_pretty(config)?;
    // Write-then-rename so a crash mid-write never leaves a truncated config behind.
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Edge, GlowMode, MonitorId, OutputConfig};

    #[test]
    fn round_trip() {
        let mut config = Config::default();
        config.look.brightness = 1.5;
        config.outputs.push(OutputConfig {
            source: MonitorId {
                device_path: r"\\?\DISPLAY#AAA".into(),
                device_name: r"\\.\DISPLAY1".into(),
                name: "Main".into(),
            },
            source_edge: Edge::Top,
            mode: GlowMode::Dedicated,
            glow_spread: Some(0.3),
            ..Default::default()
        });
        let text = toml::to_string_pretty(&config).unwrap();
        assert!(text.contains("glow_spread = 0.08\n"), "{text}");
        assert!(text.contains("glow_spread = 0.3\n"), "{text}");
        assert_eq!(parse(&text).unwrap(), config);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let config = parse("[look]\nbrightness = 0.5\n").unwrap();
        assert_eq!(config.look.brightness, 0.5);
        assert_eq!(
            config.look.smoothing_ms,
            Config::default().look.smoothing_ms
        );
        assert_eq!(config.capture, Config::default().capture);
    }

    #[test]
    fn empty_file_is_default() {
        assert_eq!(parse("").unwrap(), Config::default());
    }
}
