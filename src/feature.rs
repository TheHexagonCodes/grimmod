use crate::bridge;
use crate::config::Config;
use crate::file;
use crate::gl;
use crate::grim;
use crate::misc;

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
    if !Config::get().mods || !Config::get().display.renderer.hq_assets {
        return;
    }

    unsafe {
        grim::open_bm_image.hook(bridge::open_bm_image as grim::OpenBmImage);
        grim::manage_resource.hook(bridge::manage_resource as grim::ManageResource);
        grim::copy_image.hook(bridge::copy_image as grim::CopyImage);
        grim::decompress_image.hook(bridge::decompress_image as grim::DecompressImage);

        grim::bind_image_surface.hook(bridge::bind_image_surface as grim::BindImageSurface);
        grim::surface_upload.hook(bridge::surface_upload as grim::SurfaceUpload);
        grim::setup_draw.hook(bridge::setup_draw as grim::SetupDraw);
        gl::delete_textures.hook(bridge::delete_textures as gl::DeleteTextures);

        if Config::get().display.renderer.video_cutouts {
            grim::init_gfx.hook(bridge::init_gfx as grim::InitGfx);
            grim::draw_indexed_primitives
                .hook(bridge::draw_indexed_primitives as grim::DrawIndexedPrimitives);
        }
    };
}

/// Change the original/remaster renderer toggle to a binary rather than smooth transition
pub fn quick_toggle() {
    if !Config::get().display.renderer.quick_toggle {
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
