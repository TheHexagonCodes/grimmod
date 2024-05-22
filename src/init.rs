use crate::raw::memory::{HookError, BASE_ADDRESS};
use crate::raw::{gl, grim, process, sdl};
use crate::renderer::{graphics, video_cutouts};
use crate::{debug, feature, misc};

pub fn main() {
    debug::info("GrimMod attached to GrimFandango.exe");

    if debug::verbose() {
        debug::info(format!("Base memory address found: 0x{:x}", *BASE_ADDRESS));
    }

    if let Err(err) = initiate_startup() {
        debug::error(format!("GrimMod startup failed: {}", err));
    }
}

fn initiate_startup() -> Result<(), String> {
    let (code_addr, code_size) = process::get_first_executable_memory_region()
        .ok_or_else(|| "Could not locate executable memory region".to_string())?;
    grim::find_fns(code_addr, code_size).string_err()?;
    // hook the application entry point for the next step of the startup
    let entry_addr = process::get_application_entry_addr()
        .ok_or_else(|| grim::entry.not_found())
        .string_err()?;
    grim::entry.bind(entry_addr).string_err()?;
    grim::entry.hook(application_entry).string_err()
}

fn startup() -> Result<(), String> {
    process::bind_get_proc_address().string_err()?;
    sdl::bind_static_fns().string_err()?;
    gl::bind_static_fns().string_err()?;
    gl::bind_glew_fns().string_err()?;
    init_features().string_err()?;

    grim::init_renderers.hook(init_renderers).string_err()?;

    Ok(())
}

/// Runs some graphics setup code required for GrimMod
///
/// Since this is executed after the window has been initialized, binding
/// OpenGL functions dynamically using (a dynamic) GetProcessAddress will allow
/// tools like RenderDoc to intercept the call
fn post_graphics_startup() -> Result<(), String> {
    gl::bind_dynamic_fns().string_err()?;
    gl::compressed_tex_image2d_arb
        .hook(graphics::compressed_tex_image2d)
        .string_err()?;

    video_cutouts::create_stencil_buffer();
    misc::validate_mods();

    Ok(())
}

pub fn init_features() -> Result<(), HookError> {
    feature::mods()?;
    feature::hq_assets()?;
    feature::always_on()?;
    feature::vsync()?;
    feature::hdpi_fix()?;

    Ok(())
}

/// Wraps the application entry to locate and bind now-loaded functions
extern "stdcall" fn application_entry() {
    match startup() {
        Ok(_) => debug::info("Successfully initiated GrimMod feature hooks"),
        Err(err) => debug::error(format!("GrimMod feature hooks failed to attach: {}", err)),
    };

    grim::entry();
}

/// Wraps the renderers init function to execute some code that needs
/// to run after gfx setup is done
pub extern "C" fn init_renderers() {
    if let Err(err) = post_graphics_startup() {
        debug::error(format!(
            "Loading auxiliary OpenGL functions failed: {}",
            err
        ));
    };

    grim::init_renderers();
}

pub trait StringError<A> {
    fn string_err(self) -> Result<A, String>;
}

impl<A, E: ToString> StringError<A> for Result<A, E> {
    fn string_err(self) -> Result<A, String> {
        self.map_err(|err| err.to_string())
    }
}
