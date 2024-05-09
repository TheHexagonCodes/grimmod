use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::{c_uint, CString};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

use crate::animation;
use crate::bridge::{Image, ImageAddr, ImageContainer, ImageContainerAddr, SurfaceAddr, OVERLAYS};
use crate::debug;
use crate::file;
use crate::gl;
use crate::grim;

pub static BACKGROUND: Mutex<Option<Background>> = Mutex::new(None);
pub static BACKGROUND_WRITES: Lazy<Mutex<HashMap<(u32, u32), (u32, u32)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
pub static TARGET: Mutex<Option<Target>> = Mutex::new(None);

pub static HQ_IMAGES: Lazy<Mutex<Vec<HqImageContainer>>> = Lazy::new(|| Mutex::new(Vec::new()));
pub static BACKGROUND_SHADER: Lazy<usize> = Lazy::new(|| compile_background_shader() as usize);

#[derive(Debug)]
pub enum Target {
    Background,
    Image(ImageAddr),
}

pub enum TargetMut<'a> {
    Background(&'a mut Background),
    Image(&'a mut HqImage),
}

pub struct HqImageContainer {
    pub name: String,
    pub original_addr: ImageContainerAddr,
    pub images: Vec<HqImage>,
}

pub struct HqImage {
    pub name: String,
    pub index: usize,
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    pub original_addr: ImageAddr,
    pub data: HqImageAsyncData,
}

impl HqImageContainer {
    pub fn open(image_container: &ImageContainer) -> Option<HqImageContainer> {
        let filename = image_container.name().to_lowercase();
        if !filename.ends_with(".bm") {
            return None;
        }
        let name = Path::new(&filename).file_stem()?.to_str()?;
        let images = image_container.images();
        let hq_images = HqImage::open_image(name, &images)
            .or_else(|| HqImage::open_animation(name, &images))?;

        Some(HqImageContainer {
            name: name.to_string(),
            original_addr: image_container.original_addr,
            images: hq_images,
        })
    }

    pub fn load(image_container: &ImageContainer) -> Option<HqImageContainer> {
        let hq_image_container = HqImageContainer::open(image_container)?;
        let mut hq_image_containers = HQ_IMAGES.lock().unwrap();
        let image_addrs = image_container.image_addrs();
        let existing_index = hq_image_containers.iter().position(|hq_image_container| {
            hq_image_container
                .images
                .iter()
                .any(|image| image_addrs.contains(&image.original_addr))
        });
        let removed = existing_index.map(|index| hq_image_containers.remove(index));
        hq_image_containers.push(hq_image_container);

        removed
    }

    pub fn unload(image_container_addr: ImageContainerAddr) -> Option<HqImageContainer> {
        let mut hq_image_containers = HQ_IMAGES.lock().unwrap();
        let index = hq_image_containers.iter().position(|hq_image_container| {
            hq_image_container.original_addr == image_container_addr
        });
        index.map(|index| hq_image_containers.remove(index))
    }
}

impl HqImage {
    fn open_image(name: &str, images: &[&Image]) -> Option<Vec<HqImage>> {
        let path = file::find_modded(&format!("{}.png", name))?;
        if images.len() != 1 {
            debug::error(format!(
                "tried to open {} as image, should be animation",
                name
            ));
            return None;
        }
        let image = images.first()?;
        let (width, height) = image::image_dimensions(&path).ok()?;
        let data = HqImageAsyncData::new();

        let mut data_clone = data.clone();
        thread::spawn(move || {
            if let Ok(png) = image::open(path) {
                let has_alpha = png.color().has_alpha();
                let buffer = png.to_rgba8().into_vec();
                data_clone.loaded(buffer, has_alpha);
            } else {
                data_clone.failed();
            }
        });

        Some(vec![HqImage {
            name: name.to_string(),
            index: 0,
            width,
            height,
            scale: width / image.width as u32,
            original_addr: image.original_addr,
            data,
        }])
    }

    fn open_animation(name: &str, images: &[&Image]) -> Option<Vec<HqImage>> {
        let path = file::find_modded(&format!("{}.mkv", name))?;
        let datas: Vec<_> = (0..images.len()).map(|_| HqImageAsyncData::new()).collect();
        let (width, height) = animation::open(path, datas.clone())?;

        Some(
            datas
                .into_iter()
                .zip(images)
                .enumerate()
                .map(|(i, (dst, image))| HqImage {
                    name: format!("{} ({:02})", name, i),
                    index: i,
                    width,
                    height,
                    scale: width / image.width as u32,
                    original_addr: image.original_addr,
                    data: dst,
                })
                .collect(),
        )
    }

    pub fn is_loaded(addr: ImageAddr) -> bool {
        let mut hq_images = HQ_IMAGES.lock().unwrap();
        hq_images.iter_mut().any(|hq_image_container| {
            hq_image_container
                .images
                .iter_mut()
                .any(|hq_image| hq_image.original_addr == addr)
        })
    }

    fn map_loaded<F, R>(
        original_addr: ImageAddr,
        hq_images: &mut MutexGuard<Vec<HqImageContainer>>,
        f: F,
    ) -> Option<R>
    where
        F: FnMut(&mut HqImage) -> Option<R>,
    {
        hq_images
            .iter_mut()
            .find_map(|hq_image_container| {
                hq_image_container
                    .images
                    .iter_mut()
                    .find(|hq_image| hq_image.original_addr == original_addr)
            })
            .and_then(f)
    }

    fn with_loaded_or_else<SF, NF, R>(
        original_addr: ImageAddr,
        hq_images: &mut MutexGuard<Vec<HqImageContainer>>,
        mut some: SF,
        mut none: NF,
    ) -> R
    where
        SF: FnMut(&mut HqImage) -> R,
        NF: FnMut(&mut MutexGuard<Vec<HqImageContainer>>) -> R,
    {
        let hq_image = hq_images.iter_mut().find_map(|hq_image_container| {
            hq_image_container
                .images
                .iter_mut()
                .find(|hq_image| hq_image.original_addr == original_addr)
        });
        match hq_image {
            Some(hq_image) => some(hq_image),
            None => none(hq_images),
        }
    }

    fn to_background_mut(&mut self) -> Option<Background> {
        let width = self.width;
        let height = self.height;
        let scale = self.scale;
        let original_addr = self.original_addr;
        self.data.get_or_wait(|buffer, _| Background {
            width,
            height,
            scale,
            original_addr,
            buffer: buffer.to_vec(),
        })
    }
}

pub enum HqImageState {
    Loading,
    Failed,
    Loaded { buffer: Vec<u8>, has_alpha: bool },
}

#[derive(Clone)]
pub struct HqImageAsyncData {
    pub raw: Arc<(Mutex<HqImageState>, Condvar)>,
}

impl HqImageAsyncData {
    pub fn new() -> HqImageAsyncData {
        HqImageAsyncData {
            raw: Arc::new((Mutex::new(HqImageState::Loading), Condvar::new())),
        }
    }

    pub fn loaded(&mut self, buffer: Vec<u8>, has_alpha: bool) {
        *self.raw.0.lock().unwrap() = HqImageState::Loaded { buffer, has_alpha };
        self.raw.1.notify_all();
    }

    pub fn failed(&mut self) {
        *self.raw.0.lock().unwrap() = HqImageState::Failed;
        self.raw.1.notify_all();
    }

    fn get_or_wait<F, R>(&mut self, mut f: F) -> Option<R>
    where
        F: FnMut(&[u8], bool) -> R,
    {
        let mut state = self.raw.0.lock().unwrap();
        while matches!(*state, HqImageState::Loading) {
            state = self.raw.1.wait(state).unwrap();
        }

        match &*state {
            HqImageState::Loaded { buffer, has_alpha } => Some(f(buffer, *has_alpha)),
            _ => None,
        }
    }
}

pub struct Background {
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    pub original_addr: ImageAddr,
    pub buffer: Vec<u8>,
}

impl Background {
    /// Write (or draw over) the HQ background
    pub fn write(image_addr: ImageAddr, x: u32, y: u32) {
        let mut hq_images = HQ_IMAGES.lock().unwrap();
        if x == 0 && y == 0 {
            Background::set_from_image(image_addr, &mut hq_images);
        } else {
            HqImage::with_loaded_or_else(
                image_addr,
                &mut hq_images,
                |overlay| Background::animate(x, y, overlay),
                |hq_images| {
                    Background::restore(x, y, hq_images);
                },
            );
        }
    }

    fn animate(x: u32, y: u32, overlay: &mut HqImage) {
        let mut background = BACKGROUND.lock().unwrap();
        if let Some(background) = background.as_mut() {
            // before writing the first frame of an animation, save a snapshot of the bg
            // this will be restored as the background once the animation ends
            if overlay.index == 0 {
                background.save(x, y, overlay.width, overlay.height);
            }
            background.overlay(x, y, overlay);
        }
    }

    fn set_from_image(image_addr: ImageAddr, hq_images: &mut MutexGuard<Vec<HqImageContainer>>) {
        *BACKGROUND.lock().unwrap() =
            HqImage::map_loaded(image_addr, hq_images, HqImage::to_background_mut);
        *BACKGROUND_WRITES.lock().unwrap() = HashMap::new();
    }

    fn restore(x: u32, y: u32, hq_images: &mut MutexGuard<Vec<HqImageContainer>>) -> Option<()> {
        let mut background_guard = BACKGROUND.lock().unwrap();
        let background = background_guard.as_mut()?;
        let (width, height) = BACKGROUND_WRITES.lock().unwrap().remove(&(x, y))?;

        HqImage::map_loaded(background.original_addr, hq_images, |hq_background| {
            background.copy_from(hq_background, x, y, width, height);
            Some(())
        })
    }

    fn blend_pixels(background: (u8, u8, u8, u8), foreground: (u8, u8, u8, u8)) -> (u8, u8, u8) {
        let (br, bg, bb, _) = background;
        let (fr, fg, fb, fa) = foreground;

        let r = (fr as u16 * fa as u16 + br as u16 * (255 - fa) as u16) / 255;
        let g = (fg as u16 * fa as u16 + bg as u16 * (255 - fa) as u16) / 255;
        let b = (fb as u16 * fa as u16 + bb as u16 * (255 - fa) as u16) / 255;

        (r as u8, g as u8, b as u8)
    }

    fn save(&self, x: u32, y: u32, width: u32, height: u32) {
        BACKGROUND_WRITES
            .lock()
            .unwrap()
            .insert((x, y), (width, height));
    }

    fn copy_from(&mut self, image: &mut HqImage, x: u32, y: u32, width: u32, height: u32) {
        let x = (x * self.scale) as usize;
        let y = (y * self.scale) as usize;
        let full_width = self.width as usize;
        image.data.get_or_wait(|image_buffer, _| {
            let bytes_per_pixel = 4;
            let bytes_width = width as usize * bytes_per_pixel;
            let full_bytes_width = full_width * bytes_per_pixel;
            for i in 0..height as usize {
                let start = ((y + i) * full_bytes_width) + (x * bytes_per_pixel);
                let src_slice = &image_buffer[start..start + bytes_width];
                let dst_slice = &mut self.buffer[start..start + bytes_width];
                dst_slice.copy_from_slice(src_slice);
            }
        });
    }

    fn overlay(&mut self, x: u32, y: u32, overlay: &mut HqImage) {
        if self.scale != overlay.scale {
            debug::error(format!("{} has wrong scale for background", overlay.name));
            return;
        }

        let overlay_width = overlay.width;
        let overlay_height = overlay.height;
        let overlay_scale = overlay.scale;

        overlay
            .data
            .get_or_wait(|overlay_buffer, overlay_has_alpha| {
                let bytes_per_pixel = 4;
                let x = (x * overlay_scale) as usize;
                let y = (y * overlay_scale) as usize;
                let width = overlay_width as usize;
                let bytes_width = width * bytes_per_pixel;
                if !overlay_has_alpha {
                    for i in 0..overlay_height as usize {
                        let src_start = i * bytes_width;
                        let src_slice = &overlay_buffer[src_start..src_start + bytes_width];
                        let dst_start = (self.width as usize * (y + i) + x) * bytes_per_pixel;
                        let dst_slice = &mut self.buffer[dst_start..dst_start + bytes_width];
                        dst_slice.copy_from_slice(src_slice);
                    }
                } else {
                    for i in 0..overlay_height as usize {
                        for j in 0..width {
                            let dst_start =
                                (self.width as usize * (y + i) + (x + j)) * bytes_per_pixel;
                            let dst_pixel = (
                                self.buffer[dst_start],
                                self.buffer[dst_start + 1],
                                self.buffer[dst_start + 2],
                                self.buffer[dst_start + 3],
                            );
                            let src_start = (width * i + j) * bytes_per_pixel;
                            let src_pixel = (
                                overlay_buffer[src_start],
                                overlay_buffer[src_start + 1],
                                overlay_buffer[src_start + 2],
                                overlay_buffer[src_start + 3],
                            );
                            let (r, g, b) = Background::blend_pixels(dst_pixel, src_pixel);
                            self.buffer[dst_start] = r;
                            self.buffer[dst_start + 1] = g;
                            self.buffer[dst_start + 2] = b;
                        }
                    }
                }
            });
    }
}

/// Return the address for the background render pass's surface
fn bitmap_underlays_surface() -> Option<SurfaceAddr> {
    unsafe {
        let render_pass = grim::BITMAP_UNDERLAYS_RENDER_PASS.inner_ref();
        let render_pass_data =
            render_pass.and_then(|render_pass| render_pass.entities.data().first());
        let surface = render_pass_data.map(|render_pass_data| render_pass_data.surface);
        surface.map(SurfaceAddr::from_ptr)
    }
}

fn virtual_depth_surface() -> Option<SurfaceAddr> {
    unsafe {
        let render_pass = grim::VIRTUAL_DEPTH_RENDER_PASS.inner_ref();
        let render_pass_data =
            render_pass.and_then(|render_pass| render_pass.entities.data().first());
        let surface = render_pass_data.map(|render_pass_data| render_pass_data.surface);
        surface.map(SurfaceAddr::from_ptr)
    }
}

/// Prepare a surface (aka texture) for uploading to the GPU or upload it now
///
/// This is an overload for a native function that will be hooked
pub extern "C" fn surface_upload(surface: *mut grim::Surface, image_data: *mut c_void) {
    let surface_addr = SurfaceAddr::from_ptr(surface);
    // overriding textures does not work and leads to bugs in in the following situations:
    // 1. The modern deferred renderer is off
    // 2. The game is paused
    // 3. It's the virtual depth's pass (hq surface can apparently get reused)
    // so exit early in those cases
    let deferred_renderer_active = unsafe { grim::DEFERRED_RENDERER_ACTIVE.get().as_bool() };
    let is_paused = unsafe { grim::is_paused().as_bool() };
    let is_virtual_depth_surface = virtual_depth_surface() == Some(surface_addr);
    let target = if deferred_renderer_active && !is_paused && !is_virtual_depth_surface {
        get_target(surface_addr)
    } else {
        None
    };

    if target.is_none() {
        return unsafe {
            // call with null to reset the buffer size as it might have been changed by a hq image
            if !image_data.is_null() {
                grim::surface_upload(surface, std::ptr::null_mut());
            }
            grim::surface_upload(surface, image_data);
        };
    }

    if image_data.is_null() {
        return;
    }

    *TARGET.lock().unwrap() = target;
    unsafe {
        gl::tex_image_2d.hook(hq_tex_image_2d as gl::TexImage2d);
        gl::pixel_storei.hook(hq_pixel_storei as gl::PixelStorei);
        grim::surface_upload(surface, std::ptr::null_mut());
        gl::pixel_storei.unhook();
        gl::tex_image_2d.unhook();
    }
    *TARGET.lock().unwrap() = None;
}

fn get_target(surface_addr: SurfaceAddr) -> Option<Target> {
    if bitmap_underlays_surface() == Some(surface_addr) {
        BACKGROUND
            .lock()
            .unwrap()
            .is_some()
            .then_some(Target::Background)
    } else {
        OVERLAYS
            .lock()
            .unwrap()
            .get(&surface_addr)
            .cloned()
            .map(Target::Image)
    }
}

fn with_target_hq_image<F: FnMut(TargetMut)>(mut f: F) {
    let mut background = BACKGROUND.lock().unwrap();
    let mut hq_images = HQ_IMAGES.lock().unwrap();
    match TARGET.lock().unwrap().as_mut() {
        Some(Target::Background) if let Some(background) = background.as_mut() => {
            f(TargetMut::Background(background))
        }
        Some(Target::Image(image_addr)) => HqImage::with_loaded_or_else(
            *image_addr,
            &mut hq_images,
            |hq_image| f(TargetMut::Image(hq_image)),
            |_| {},
        ),
        _ => {}
    }
}

extern "stdcall" fn hq_tex_image_2d(
    _target: gl::Enum,
    _level: gl::Int,
    _internalformat: gl::Int,
    _width: gl::Sizei,
    _height: gl::Sizei,
    _border: gl::Int,
    _format: gl::Enum,
    _typ: gl::Enum,
    _data: *const c_void,
) {
    fn tex_image_2d(width: u32, height: u32, ptr: *const u8) {
        unsafe {
            gl::tex_image_2d(
                gl::TEXTURE_2D,
                0,
                gl::RGBA8 as gl::Int,
                width as gl::Int,
                height as gl::Int,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                ptr as *const _,
            )
        }
    }
    with_target_hq_image(|target_ref| match target_ref {
        TargetMut::Background(background) => tex_image_2d(
            background.width,
            background.height,
            background.buffer.as_ptr(),
        ),
        TargetMut::Image(hq_image) => {
            let width = hq_image.width;
            let height = hq_image.height;
            hq_image
                .data
                .get_or_wait(|buffer, _| tex_image_2d(width, height, buffer.as_ptr()));
        }
    })
}

extern "stdcall" fn hq_pixel_storei(pname: gl::Enum, param: gl::Int) {
    with_target_hq_image(|target_ref| unsafe {
        if pname == gl::UNPACK_ROW_LENGTH {
            let width = match target_ref {
                TargetMut::Background(background) => background.width,
                TargetMut::Image(hq_image) => hq_image.width,
            };
            gl::pixel_storei(gl::UNPACK_ROW_LENGTH, width as gl::Int);
        } else {
            gl::pixel_storei(pname, param);
        }
    })
}

fn is_drawing_hq(draw: *const grim::Draw) -> bool {
    let Some(surface) = drawing_surface(draw) else {
        return false;
    };

    (Some(surface) == bitmap_underlays_surface() && BACKGROUND.lock().unwrap().is_some())
        || OVERLAYS.lock().unwrap().contains_key(&surface)
}

fn drawing_surface(draw: *const grim::Draw) -> Option<SurfaceAddr> {
    unsafe { draw.as_ref() }.map(|draw| SurfaceAddr::from_ptr(draw.surfaces[0]))
}

fn drawing_sampler(draw: *const grim::Draw) -> Option<gl::Uint> {
    unsafe { draw.as_ref() }.map(|draw| draw.samplers[0] as c_uint)
}

/// Sets the OpenGL state for the next draw call
///
/// This is an overload for a native function that will be hooked
pub extern "C" fn setup_draw(draw: *mut grim::Draw, index_buffer: *const c_void) {
    let hq = is_drawing_hq(draw);

    // for hq images, use a custom shader that keeps the full resolution
    if hq && let Some(draw) = unsafe { draw.as_mut() } {
        draw.shader = *BACKGROUND_SHADER as *const grim::Shader;
    }

    unsafe {
        grim::setup_draw(draw, index_buffer);
    }

    if hq && let (Some(surface), Some(sampler)) = (drawing_surface(draw), drawing_sampler(draw)) {
        unsafe {
            // for hq images, use a linear texture filter for better quality
            gl::sampler_parameteri(sampler, gl::TEXTURE_MIN_FILTER, gl::LINEAR as gl::Int);
            gl::sampler_parameteri(sampler, gl::TEXTURE_MAG_FILTER, gl::LINEAR as gl::Int);

            if Some(surface) != bitmap_underlays_surface() {
                gl::blend_func_separate(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA, 1, 0);
            }
        }
    }
}

pub fn compile_background_shader() -> *const grim::Shader {
    let name = CString::new("grimmod_background").unwrap();
    unsafe { grim::compile_shader(name.as_ptr()) }
}

pub static BACKGROUND_V_SHADER: &str = r#"
    #version 330
    layout(std140) uniform VSConstants {
         vec4    MultiplyColor;     vec4    TexelSizeDesaturateGamma;     vec4    ColorKey;
    } constantsV;
    in vec3 vs_Position;
    in vec2 vs_TexCoord;
    out vec2 ps_TexCoordPx;
    void main ()
    {
      gl_Position = vec4(vs_Position, 1.0);
      ps_TexCoordPx = vs_TexCoord;
    }
"#;

pub static BACKGROUND_P_SHADER: &str = r#"
    #version 330
    layout(std140) uniform PSConstants {
         vec4    MultiplyColor;     vec4    TexelSizeDesaturateGamma;     vec4    ColorKey;
    } constantsP;
    uniform sampler2D ps_Texture0;
    in vec2 ps_TexCoordPx;
    out vec4 ps_Result;
    void main ()
    {
      ps_Result = texture(ps_Texture0, ps_TexCoordPx);
    }
"#;
