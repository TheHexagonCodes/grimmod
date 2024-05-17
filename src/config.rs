use once_cell::sync::Lazy;

pub static CONFIG: Lazy<Config> = Lazy::new(Config::load);

#[derive(Clone, serde::Deserialize)]
pub struct Config {
    #[serde(default = "default_true")]
    pub mods: bool,
    #[serde(default = "Renderer::new")]
    pub renderer: Renderer,
    #[serde(default = "Display::new")]
    pub display: Display,
    #[serde(default = "Logging::new")]
    pub logging: Logging,
}

impl Config {
    pub fn new() -> Config {
        Config {
            mods: true,
            renderer: Renderer::new(),
            display: Display::new(),
            logging: Logging::new(),
        }
    }

    pub fn get() -> Config {
        CONFIG.clone()
    }

    pub fn load() -> Config {
        Config::try_load().unwrap_or_else(Config::new)
    }

    pub fn try_load() -> Option<Config> {
        let contents = std::fs::read_to_string("grimmod.toml").ok()?;
        toml::from_str(&contents).ok()?
    }
}

#[derive(Clone, serde::Deserialize)]
pub struct Display {
    #[serde(default = "default_true")]
    pub hdpi_fix: bool,
    #[serde(default = "default_true")]
    pub vsync: bool,
}

impl Display {
    pub fn new() -> Display {
        Display {
            hdpi_fix: true,
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

impl Renderer {
    pub fn new() -> Renderer {
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
}

impl Logging {
    pub fn new() -> Logging {
        Logging {
            enabled: true,
            debug: false,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}
