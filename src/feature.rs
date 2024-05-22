use crate::config::Config;
use crate::raw::memory::HookError;
use crate::raw::{gl, grim, sdl};
use crate::renderer::graphics;
use crate::{file, misc};

/// Overload native IO functions to load modded files
pub fn mods() -> Result<(), HookError> {
    if !Config::get().mods {
        return Ok(());
    }

    grim::open_file.hook(file::open as grim::OpenFile)?;
    grim::close_file.hook(file::close as grim::CloseFile)?;
    grim::read_file.hook(file::read as grim::ReadFile)?;

    Ok(())
}

/// Upgrade image loading and display pipeline to enable HD 32bit assets
pub fn hq_assets() -> Result<(), HookError> {
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

/// Some functions need to be hooked always
pub fn always_on() -> Result<(), HookError> {
    grim::render_scene.hook(graphics::render_scene as grim::RenderScene)?;

    Ok(())
}

/// Force VSync to be always on
pub fn vsync() -> Result<(), HookError> {
    if !Config::get().display.vsync {
        return Ok(());
    }

    sdl::set_swap_interval.hook(misc::sdl_gl_set_swap_interval as sdl::SetSwapInterval)?;

    Ok(())
}

/// Render game at native resolution even on HDPI screens
pub fn hdpi_fix() -> Result<(), HookError> {
    if !Config::get().display.hdpi_fix {
        return Ok(());
    }

    sdl::create_window.hook(misc::sdl_create_window as sdl::CreateWindow)?;
    sdl::get_display_bounds.hook(misc::sdl_get_display_bounds as sdl::GetDisplayBounds)?;
    sdl::get_current_display_mode
        .hook(misc::sdl_get_current_display_mode as sdl::GetCurrentDisplayMode)?;

    Ok(())
}
