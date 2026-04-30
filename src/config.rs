use std::sync::LazyLock;

static CONFIG: LazyLock<Config> = LazyLock::new(Config::load);

#[derive(Clone, serde::Deserialize)]
pub struct Config {
    #[serde(default = "default_true")]
    pub mods: bool,
    #[serde(default)]
    pub renderer: Renderer,
    #[serde(default)]
    pub display: Display,
    #[serde(default)]
    pub logging: Logging,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            mods: true,
            renderer: Renderer::default(),
            display: Display::default(),
            logging: Logging::default(),
        }
    }
}

impl Config {
    pub fn get() -> &'static Config {
        &CONFIG
    }

    fn load() -> Config {
        Config::try_load().unwrap_or_default()
    }

    fn try_load() -> Option<Config> {
        let contents = std::fs::read_to_string("grimmod.toml").ok()?;
        toml::from_str(&contents).ok()?
    }
}

#[derive(Clone, serde::Deserialize)]
pub struct Display {
    /// HDPI fix — Windows-only, ignored on Linux.
    #[cfg(target_os = "windows")]
    #[serde(default = "default_true")]
    pub hdpi_fix: bool,
    #[cfg(target_os = "linux")]
    #[serde(default = "default_false")]
    pub hdpi_fix: bool,
    #[serde(default = "default_true")]
    pub vsync: bool,
}

impl Default for Display {
    fn default() -> Display {
        Display {
            #[cfg(target_os = "windows")]
            hdpi_fix: true,
            #[cfg(target_os = "linux")]
            hdpi_fix: false,
            vsync: true,
        }
    }
}

#[derive(Clone, serde::Deserialize)]
pub struct Renderer {
    #[serde(default = "default_true")]
    pub hq_assets: bool,
    #[serde(default = "default_true")]
    pub quick_toggle: bool,
    #[serde(default = "default_true")]
    pub video_cutouts: bool,
}

impl Default for Renderer {
    fn default() -> Renderer {
        Renderer {
            hq_assets: true,
            quick_toggle: true,
            video_cutouts: true,
        }
    }
}

#[derive(Clone, serde::Deserialize)]
pub struct Logging {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_false")]
    pub debug: bool,
    /// Enable per-frame performance profiling output.
    #[serde(default = "default_false")]
    pub profile: bool,
}

impl Default for Logging {
    fn default() -> Logging {
        Logging {
            enabled: true,
            debug: false,
            profile: false,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}
