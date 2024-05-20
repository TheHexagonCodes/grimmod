use glob::glob;
use semver::{Version, VersionReq};
use std::ffi::{c_char, c_int, c_void};
use std::fs::File;
use std::io::Read;
use windows::Win32::Foundation::BOOL;
use windows::Win32::Graphics::Gdi::GetMonitorInfoW;
use windows::Win32::Graphics::Gdi::MonitorFromWindow;
use windows::Win32::Graphics::Gdi::{MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::WindowsAndMessaging::SetProcessDPIAware;

use crate::debug;
use crate::raw::{grim, sdl};

const VERSION: Version = Version::new(1, 0, 0);

#[derive(serde::Deserialize)]
pub struct ModInfo {
    pub name: String,
    pub version: Version,
    pub author: String,
    pub contact: String,
    pub homepage: String,
    pub description: String,
    pub grimmod_version: VersionReq,
}

pub fn validate_mods() {
    let Ok(mod_infos) = glob("./Mods/*/info.json") else {
        return;
    };
    for info_path in mod_infos {
        let mut contents = String::new();
        let info: Option<ModInfo> = info_path
            .ok()
            .and_then(|path| File::open(path).ok())
            .and_then(|mut file| file.read_to_string(&mut contents).ok())
            .and_then(|_| serde_json::from_str(&contents).ok());

        if let Some(info) = info {
            if info.grimmod_version.matches(&VERSION) {
                debug::info(format!(
                    "Mod validated: {} {} (by {} at {})",
                    info.name, info.version, info.author, info.homepage
                ));
            } else {
                debug::error(format!(
                    "Mod failed validation: {} {} was made for grimmod {} but {} found.",
                    info.name, info.version, info.grimmod_version, VERSION
                ));
                debug::error("Disable it if issues arise");
            }
        }
    }
}

/// Get the game's screen's size and position
pub fn screen_bounds() -> Option<sdl::Rect> {
    let mut window_info: sdl::SysWminfo = Default::default();
    let mut monitor_info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    unsafe {
        let window = grim::GAME_WINDOW.get();
        if sdl::get_window_wminfo(window as *mut _, &mut window_info) == BOOL(0) {
            return None;
        }
        let hmonitor = MonitorFromWindow(window_info.window, MONITOR_DEFAULTTONEAREST);
        GetMonitorInfoW(hmonitor, &mut monitor_info);
    };

    let x = monitor_info.rcMonitor.left;
    let y = monitor_info.rcMonitor.top;
    let width = monitor_info.rcMonitor.right - x;
    let height = monitor_info.rcMonitor.bottom - y;

    Some(sdl::Rect {
        x: 0,
        y: 0,
        w: width,
        h: height,
    })
}

/// SDL function for controlling the swap interval (vsync)
///
/// This is a overload for a native function that will be hooked
pub extern "C" fn sdl_gl_set_swap_interval(_interval: c_int) -> c_int {
    sdl::set_swap_interval(1)
}

pub extern "C" fn sdl_create_window(
    title: *const c_char,
    x: c_int,
    y: c_int,
    w: c_int,
    h: c_int,
    flags: u32,
) -> *mut c_void {
    unsafe {
        SetProcessDPIAware();
        sdl::create_window(title, x, y, w, h, flags | sdl::WINDOW_ALLOW_HIGHDPI)
    }
}

pub extern "C" fn sdl_get_display_bounds(_display_index: c_int, rect: *mut sdl::Rect) -> c_int {
    let Some(screen_bounds) = screen_bounds() else {
        return -1;
    };
    unsafe {
        (*rect) = screen_bounds;
    }
    0
}

pub extern "C" fn sdl_get_current_display_mode(
    display_index: c_int,
    mode: *mut sdl::DisplayMode,
) -> c_int {
    unsafe {
        let result = sdl::get_current_display_mode(display_index, mode);
        if result == 0 {
            let Some(screen_bounds) = screen_bounds() else {
                return -1;
            };
            (*mode).width = screen_bounds.w;
            (*mode).height = screen_bounds.h;
        }
        result
    }
}

pub extern "C" fn draw_software_scene(
    draw: *const grim::Draw,
    software_surface: *const grim::Surface,
    transition: f32,
) {
    unsafe {
        let value = if transition == 1.0 {
            1.0
        } else {
            grim::RENDERING_MODE.get()
        };
        grim::draw_software_scene(draw, software_surface, value);
    }
}
