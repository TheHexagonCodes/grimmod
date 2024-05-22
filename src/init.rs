use crate::{
    config::Config,
    debug, misc,
    raw::{
        gl, grim,
        memory::{HookError, BASE_ADDRESS},
        process, sdl,
    },
    renderer::{graphics, video_cutouts}, file,
};

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
    init_hooks().string_err()?;

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

pub fn init_hooks() -> Result<(), HookError> {
    always_on_hooks()?;
    mods_hooks()?;
    hq_assets_hooks()?;
    vsync_hooks()?;
    hdpi_fix_hooks()?;

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

/// Some functions need to be hooked always
pub fn always_on_hooks() -> Result<(), HookError> {
    grim::init_renderers.hook(init_renderers)?;
    grim::render_scene.hook(graphics::render_scene as grim::RenderScene)?;

    Ok(())
}

/// Overload native IO functions to load modded files
pub fn mods_hooks() -> Result<(), HookError> {
    if !Config::get().mods {
        return Ok(());
    }

    grim::open_file.hook(file::open as grim::OpenFile)?;
    grim::close_file.hook(file::close as grim::CloseFile)?;
    grim::read_file.hook(file::read as grim::ReadFile)?;

    Ok(())
}

/// Upgrade image loading and display pipeline to enable HD 32bit assets
pub fn hq_assets_hooks() -> Result<(), HookError> {
    if !Config::get().mods || !Config::get().renderer.hq_assets {
        return Ok(());
    }

    grim::open_bm_image.hook(graphics::open_bm_image as grim::OpenBmImage)?;
    grim::manage_resource.hook(graphics::manage_resource as grim::ManageResource)?;
    grim::copy_image.hook(graphics::copy_image as grim::CopyImage)?;
    grim::decompress_image.hook(graphics::decompress_image as grim::DecompressImage)?;
    grim::bind_image_surface.hook(graphics::bind_image_surface as grim::BindImageSurface)?;
    grim::surface_upload.hook(graphics::surface_upload as grim::SurfaceUpload)?;
    grim::setup_draw.hook(graphics::setup_draw as grim::SetupDraw)?;
    gl::delete_textures.hook(graphics::delete_textures as gl::DeleteTextures)?;

    if Config::get().renderer.video_cutouts {
        grim::draw_indexed_primitives
            .hook(graphics::draw_indexed_primitives as grim::DrawIndexedPrimitives)?;
    }

    Ok(())
}

/// Force VSync to be always on
pub fn vsync_hooks() -> Result<(), HookError> {
    if !Config::get().display.vsync {
        return Ok(());
    }

    sdl::set_swap_interval.hook(misc::sdl_gl_set_swap_interval as sdl::SetSwapInterval)?;

    Ok(())
}
/// Render game at native resolution even on HDPI screens
pub fn hdpi_fix_hooks() -> Result<(), HookError> {
    if !Config::get().display.hdpi_fix {
        return Ok(());
    }

    sdl::create_window.hook(misc::sdl_create_window as sdl::CreateWindow)?;
    sdl::get_display_bounds.hook(misc::sdl_get_display_bounds as sdl::GetDisplayBounds)?;
    sdl::get_current_display_mode
        .hook(misc::sdl_get_current_display_mode as sdl::GetCurrentDisplayMode)?;

    Ok(())
}

pub trait StringError<A> {
    fn string_err(self) -> Result<A, String>;
}

impl<A, E: ToString> StringError<A> for Result<A, E> {
    fn string_err(self) -> Result<A, String> {
        self.map_err(|err| err.to_string())
    }
}
