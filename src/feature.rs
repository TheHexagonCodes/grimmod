use crate::config::Config;
use crate::raw::{gl, grim};
use crate::renderer::graphics;
use crate::{file, misc};

/// Overload native IO functions to load modded files
pub fn mods() {
    if !Config::get().mods {
        return;
    }

    unsafe {
        grim::open_file.hook(file::open as grim::OpenFile);
        grim::close_file.hook(file::close as grim::CloseFile);
        grim::read_file.hook(file::read as grim::ReadFile);
    }
}

/// Upgrade image loading and display pipeline to enable HD 32bit assets
pub fn hq_assets() {
    if !Config::get().mods || !Config::get().renderer.hq_assets {
        return;
    }

    unsafe {
        grim::open_bm_image.hook(graphics::open_bm_image as grim::OpenBmImage);
        grim::manage_resource.hook(graphics::manage_resource as grim::ManageResource);
        grim::copy_image.hook(graphics::copy_image as grim::CopyImage);
        grim::decompress_image.hook(graphics::decompress_image as grim::DecompressImage);

        grim::bind_image_surface.hook(graphics::bind_image_surface as grim::BindImageSurface);
        grim::surface_upload.hook(graphics::surface_upload as grim::SurfaceUpload);
        grim::setup_draw.hook(graphics::setup_draw as grim::SetupDraw);
        gl::delete_textures
            .hook(graphics::delete_textures as gl::DeleteTextures)
            .ok();

        if Config::get().renderer.video_cutouts {
            grim::init_gfx.hook(graphics::init_gfx as grim::InitGfx);
            grim::draw_indexed_primitives
                .hook(graphics::draw_indexed_primitives as grim::DrawIndexedPrimitives);
        }
    };
}

/// Change the original/remaster renderer toggle to a binary rather than smooth transition
pub fn quick_toggle() {
    if !Config::get().renderer.quick_toggle {
        return;
    }

    unsafe {
        grim::draw_software_scene.hook(misc::draw_software_scene as grim::DrawSoftwareScene);
    }
}

/// Force VSync to be always on
pub fn vsync() {
    if !Config::get().display.vsync {
        return;
    }

    unsafe {
        gl::sdl_set_swap_interval.hook(misc::sdl_gl_set_swap_interval as gl::SdlSetSwapInterval);
    }
}

/// Render game at native resolution even on HDPI screens
pub fn hdpi_fix() {
    if !Config::get().display.hdpi_fix {
        return;
    }

    unsafe {
        gl::sdl_create_window.hook(misc::sdl_create_window as gl::SdlCreateWindow);
        gl::sdl_get_display_bounds.hook(misc::sdl_get_display_bounds as gl::SdlGetDisplayBounds);
        gl::sdl_get_current_display_mode
            .hook(misc::sdl_get_current_display_mode as gl::SdlGetCurrentDisplayMode);
    }
}
