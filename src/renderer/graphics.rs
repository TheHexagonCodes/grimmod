use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;
use std::sync::Mutex;

use crate::config::Config;
use crate::debug;
use crate::perf;
use crate::raw::{gl, grim};
use crate::renderer::{image, video_cutouts};

pub static DECOMPRESSED: Mutex<Option<ImageAddr>> = Mutex::new(None);
pub static OVERLAYS: LazyLock<Mutex<HashMap<SurfaceAddr, ImageAddr>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
pub static SMUSH_SURFACE: Mutex<Option<SurfaceAddr>> = Mutex::new(None);

/// Atomic flags that control persistent sub-hook behavior (macOS only).
/// When a flag is set, the corresponding GL sub-hook applies custom logic;
/// when clear, it passes through to the original GL function. This lets us
/// install each GL hook once and never unhook it, avoiding repeated
/// mach_vm_protect kernel traps.
#[cfg(target_os = "macos")]
static HQ_UPLOAD_ACTIVE: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
static FORCE_LINEAR_ACTIVE: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
static STENCIL_DRAW_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Default, Hash, Eq, PartialEq)]
pub struct ImageContainerAddr(usize);

impl ImageContainerAddr {
    pub fn from_ptr(ptr: *const grim::ImageContainer) -> ImageContainerAddr {
        ImageContainerAddr(ptr as usize)
    }
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct ImageAddr(usize);

impl ImageAddr {
    pub fn from_ptr(ptr: *const grim::Image) -> ImageAddr {
        ImageAddr(ptr as usize)
    }

    pub fn underlying(&self) -> usize {
        self.0
    }

    pub fn image(&self) -> Option<Image> {
        Image::from_raw(self.0 as *const _)
    }

    /// Gets the original image address before decompression
    pub fn original(&self) -> ImageAddr {
        if self.is_decompression_buffer() {
            DECOMPRESSED
                .lock()
                .expect("DECOMPRESSED lock poisoned")
                .unwrap_or(*self)
        } else {
            *self
        }
    }

    pub fn is_decompression_buffer(&self) -> bool {
        self.0 == unsafe { grim::DECOMPRESSION_BUFFER.inner_addr() }
    }

    pub fn is_clean_buffer(&self) -> bool {
        self.0 == unsafe { grim::CLEAN_BUFFER.inner_addr() }
    }

    pub fn is_clean_z_buffer(&self) -> bool {
        self.0 == unsafe { grim::CLEAN_Z_BUFFER.inner_addr() }
    }

    /// Windows: BACK_BUFFER IS the image, so use .addr()
    /// Linux/macOS: BACK_BUFFER is a pointer to the image, so use .inner_addr()
    pub fn is_back_buffer(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            self.0 == unsafe { grim::BACK_BUFFER.addr() }
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.0 == unsafe { grim::BACK_BUFFER.inner_addr() }
        }
    }

    pub fn is_smush_buffer(&self) -> bool {
        self.0 == unsafe { grim::SMUSH_BUFFER.inner_addr() }
    }

    pub fn name(&self) -> String {
        if self.is_decompression_buffer() {
            format!("DECOMPRESSION_BUFFER aka {}", self.original().name())
        } else if self.is_clean_buffer() {
            "CLEAN_BUFFER".to_string()
        } else if self.is_clean_z_buffer() {
            "CLEAN_Z_BUFFER".to_string()
        } else if self.is_back_buffer() {
            "BACK_BUFFER".to_string()
        } else if self.is_smush_buffer() {
            "SMUSH_BUFFER".to_string()
        } else if let Some(name) = image::HqImage::name(*self) {
            name
        } else {
            format!("unknown/dynamic buffer (0x{:x})", self.0)
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct SurfaceAddr(usize);

impl SurfaceAddr {
    pub fn from_ptr(ptr: *const grim::Surface) -> SurfaceAddr {
        SurfaceAddr(ptr as usize)
    }

    /// Return the address for the background render pass's surface
    pub fn bitmap_underlays() -> Option<SurfaceAddr> {
        let t = perf::begin();
        let result = unsafe {
            let rp_ptr = grim::BITMAP_UNDERLAYS_RENDER_PASS.inner_addr();
            let render_pass = grim::BITMAP_UNDERLAYS_RENDER_PASS.inner_ref()?;
            let entities_data = render_pass.entities.data();
            let entity = entities_data.first()?;
            let surface_ptr = entity.surface;
            if debug::verbose() {
                debug::debug(format!(
                    "bitmap_underlays: rp_ptr=0x{:x}, entities.start=0x{:x}, entities.len={}, entity_ptr=0x{:x}, surface_ptr=0x{:x}",
                    rp_ptr,
                    render_pass.entities.start as usize,
                    entities_data.len(),
                    entity as *const _ as usize,
                    surface_ptr as usize,
                ));
            }
            Some(SurfaceAddr::from_ptr(surface_ptr))
        };
        perf::end(t, &perf::BITMAP_UNDERLAYS);
        result
    }

    pub fn is_bitmap_underlays(&self) -> bool {
        Some(self) == SurfaceAddr::bitmap_underlays().as_ref()
    }
}

pub struct ImageContainer<'a> {
    raw: &'a grim::ImageContainer,
    images: Vec<Image>,
    pub original_addr: ImageContainerAddr,
}

impl<'a> ImageContainer<'a> {
    pub fn from_raw(image_container: *const grim::ImageContainer) -> Option<ImageContainer<'a>> {
        let original_addr = ImageContainerAddr::from_ptr(image_container);
        unsafe { image_container.as_ref() }.map(|raw| {
            let image_ptrs =
                unsafe { std::slice::from_raw_parts(raw.images, raw.image_count as usize) };
            let images = image_ptrs
                .iter()
                .filter_map(|&image_ptr| Image::from_raw(image_ptr))
                .collect();
            ImageContainer {
                raw,
                images,
                original_addr,
            }
        })
    }

    pub fn name(&self) -> &str {
        let cstr = unsafe { CStr::from_ptr(self.raw.name.as_ptr()) };
        cstr.to_str().unwrap_or("")
    }

    pub fn images(&self) -> &[Image] {
        &self.images
    }

    pub fn image_addrs(&self) -> Vec<ImageAddr> {
        let image_addrs = unsafe {
            std::slice::from_raw_parts(
                self.raw.images as *const usize,
                self.raw.image_count as usize,
            )
        };
        image_addrs.iter().map(|&addr| ImageAddr(addr)).collect()
    }
}

pub struct Image {
    pub addr: ImageAddr,
    pub width: i32,
    pub height: i32,
}

impl Image {
    pub fn from_raw(image: *const grim::Image) -> Option<Image> {
        let addr = ImageAddr::from_ptr(image);
        unsafe { image.as_ref() }.map(|raw| {
            let (width, height) = (raw.attributes.width, raw.attributes.height);
            Image {
                addr,
                width,
                height,
            }
        })
    }
}

#[allow(dead_code)]
pub struct Draw {
    pub addr: usize,
    pub surface: SurfaceAddr,
    pub sampler: gl::Uint,
}

impl Draw {
    pub fn from_raw(raw: *mut grim::Draw) -> Option<Draw> {
        let addr = raw as usize;
        unsafe { raw.as_ref() }.map(|draw| Draw {
            addr,
            surface: SurfaceAddr::from_ptr(draw.surfaces[0]),
            sampler: draw.samplers[0] as c_uint,
        })
    }

    pub fn is_hq(&self) -> bool {
        let t = perf::begin();
        let result = (self.surface.is_bitmap_underlays()
            && image::BACKGROUND
                .lock()
                .expect("BACKGROUND lock poisoned")
                .is_some())
            || OVERLAYS
                .lock()
                .expect("OVERLAYS lock poisoned")
                .contains_key(&self.surface);
        perf::end(t, &perf::IS_HQ);
        result
    }

    pub fn is_smush(&self) -> bool {
        let t = perf::begin();
        let result =
            Some(self.surface) == *SMUSH_SURFACE.lock().expect("SMUSH_SURFACE lock poisoned");
        perf::end(t, &perf::IS_SMUSH);
        result
    }
}

/// Removes all HQ overlay pairs owned by dropped HQ container
pub fn unpair_overlay_surfaces(hq_image_container: &image::HqImageContainer) {
    let mut overlays = OVERLAYS.lock().expect("OVERLAYS lock poisoned");
    for hq_image in hq_image_container.images.iter() {
        overlays.retain(|_, image_addr| image_addr != &hq_image.original_addr);
    }
}

/// Install GL sub-hooks once and keep them permanent. The sub-hooks check
/// atomic flags to decide whether to apply custom behavior or pass through.
/// This avoids repeated hook/unhook cycles that each trigger a
/// mach_vm_protect kernel trap on macOS (4-6 traps per frame eliminated).
///
/// On Linux/Windows, we use the traditional per-frame hook/unhook pattern
/// instead, to isolate persistent hooks as a macOS-only optimization for now.
#[cfg(target_os = "macos")]
pub fn install_persistent_gl_hooks() {
    if let Err(e) = gl::tex_image_2d.hook(persistent_tex_image_2d as gl::TexImage2d) {
        debug::error(format!("Failed to persistently hook tex_image_2d: {}", e));
    }
    if let Err(e) = gl::pixel_storei.hook(persistent_pixel_storei as gl::PixelStorei) {
        debug::error(format!("Failed to persistently hook pixel_storei: {}", e));
    }
    if let Err(e) =
        gl::sampler_parameteri.hook(persistent_sampler_parameteri as gl::SamplerParameteri)
    {
        debug::error(format!(
            "Failed to persistently hook sampler_parameteri: {}",
            e
        ));
    }
    if let Err(e) = gl::draw_elements_base_vertex
        .hook(persistent_draw_elements_base_vertex as gl::DrawElementsBaseVertex)
    {
        debug::error(format!(
            "Failed to persistently hook draw_elements_base_vertex: {}",
            e
        ));
    }
    debug::info("Installed persistent GL sub-hooks (flag-gated, no per-frame kernel traps)");
}

/// On Linux/Windows, persistent GL hooks are not used. The per-frame
/// hook/unhook pattern is used instead (in surface_upload, render_scene, etc.).
#[cfg(not(target_os = "macos"))]
pub fn install_persistent_gl_hooks() {
    debug::info("Persistent GL sub-hooks disabled on this platform (using per-frame hook/unhook)");
}

/// Hooks BM image loading to load a modded HQ version of the image
pub extern "C" fn open_bm_image(
    filename: *const c_char,
    param_2: u32,
    param_3: u32,
) -> *mut grim::ImageContainer {
    let t = perf::begin();
    let image_container = grim::open_bm_image(filename, param_2, param_3);

    if let Some(image_container) = ImageContainer::from_raw(image_container) {
        let removed = image::HqImageContainer::load(&image_container);
        if let Some(hq_image_container) = removed {
            unpair_overlay_surfaces(&hq_image_container);
        }
    }

    perf::end(t, &perf::OPEN_BM_IMAGE);
    image_container
}

/// Hooks resource management to drop HQ images with original image
pub extern "C" fn manage_resource(resource: *mut grim::Resource) -> c_int {
    let t = perf::begin();
    let state = unsafe { (*resource).state };
    let image_container_addr = ImageContainerAddr(unsafe { (*resource).image_container as usize });

    if state == 2 {
        let removed = image::HqImageContainer::unload(image_container_addr);
        if let Some(hq_image_container) = removed {
            unpair_overlay_surfaces(&hq_image_container);
        }
    }

    let result = grim::manage_resource(resource);
    perf::end(t, &perf::MANAGE_RESOURCE);
    result
}

/// Hooks decompression to track an image through the system
pub extern "C" fn decompress_image(image: *const grim::Image) {
    let t = perf::begin();
    if debug::verbose() {
        debug::info(format!(
            "Decompressing {}",
            ImageAddr::from_ptr(image).name()
        ));
    }

    // store the address of the last image decompressed
    // it will shortly be copied to the clean buffer and rendered
    *DECOMPRESSED.lock().expect("DECOMPRESSED lock poisoned") = Some(ImageAddr::from_ptr(image));

    grim::decompress_image(image);
    perf::end(t, &perf::DECOMPRESS_IMAGE);
}

fn active_smush_frame_size() -> Option<(i32, i32)> {
    let frame = unsafe { grim::ACTIVE_SMUSH_FRAME.inner_ref() }?;
    Some((frame.attributes.width, frame.attributes.height))
}

/// Hooks image copying to detect when a background or overlay is being interacted with
#[cfg(not(target_os = "macos"))]
pub extern "C" fn copy_image(
    dst_image: *mut grim::Image,
    dst_surface: *mut grim::Surface,
    src_image: *mut grim::Image,
    src_surface: *mut grim::Surface,
    x: u32,
    y: u32,
    param_7: u32,
    param_8: u32,
) {
    let t = perf::begin();
    copy_image_inner(dst_image, dst_surface, src_image, src_surface, x, y);

    grim::copy_image(
        dst_image,
        dst_surface,
        src_image,
        src_surface,
        x,
        y,
        param_7,
        param_8,
    );
    perf::end(t, &perf::COPY_IMAGE);
}

/// Hooks image copying to detect when a background or overlay is being interacted with
/// macOS version: param_7 is LECRECT* (64-bit pointer), not u32
#[cfg(target_os = "macos")]
pub extern "C" fn copy_image(
    dst_image: *mut grim::Image,
    dst_surface: *mut grim::Surface,
    src_image: *mut grim::Image,
    src_surface: *mut grim::Surface,
    x: u32,
    y: u32,
    param_7: usize,
    param_8: u32,
) {
    let t = perf::begin();
    copy_image_inner(dst_image, dst_surface, src_image, src_surface, x, y);

    grim::copy_image(
        dst_image,
        dst_surface,
        src_image,
        src_surface,
        x,
        y,
        param_7,
        param_8,
    );
    perf::end(t, &perf::COPY_IMAGE);
}

/// Shared logic for copy_image hook (platform-independent)
fn copy_image_inner(
    dst_image: *mut grim::Image,
    _dst_surface: *mut grim::Surface,
    src_image: *mut grim::Image,
    _src_surface: *mut grim::Surface,
    x: u32,
    y: u32,
) {
    let src_image_addr = ImageAddr::from_ptr(src_image);
    let dst_image_addr = ImageAddr::from_ptr(dst_image);

    if debug::verbose() {
        debug::info(format!(
            "Copying {} to {}",
            src_image_addr.name(),
            dst_image_addr.name(),
        ));
    }

    let src_image_addr = ImageAddr::from_ptr(src_image).original();

    // an image being copied to the clean buffer first means it is a background (or draws
    // over the background) about to be rendered
    if dst_image_addr.is_clean_buffer() {
        if let Some(src_image) = src_image_addr.image() {
            image::Background::write(src_image, x, y);
        }
    }

    if dst_image_addr.is_back_buffer() {
        // remove any HQ background if a fullscreen video plays as it will cover it
        // these are not true cutscenes and don't change the scene
        // this is unnecessary for scenes with cutout backgrounds
        if src_image_addr.is_smush_buffer()
            && active_smush_frame_size() == Some((640, 480))
            && !image::Background::is_stencilled_video_scene()
        {
            *image::BACKGROUND.lock().expect("BACKGROUND lock poisoned") = None;
        }
    }
}

/// Hooks surface binding to associate surfaces with HQ overlays
pub extern "C" fn bind_image_surface(
    image: *mut grim::Image,
    param_2: u32,
    param_3: u32,
    param_4: u32,
) -> *mut grim::Surface {
    let t = perf::begin();
    let image_addr = ImageAddr::from_ptr(image).original();
    let is_hq = image::HqImage::is_loaded(image_addr);
    let surface = grim::bind_image_surface(image, param_2, param_3, param_4);
    let surface_addr = SurfaceAddr::from_ptr(surface);

    if image_addr.is_smush_buffer() {
        *SMUSH_SURFACE.lock().expect("SMUSH_SURFACE lock poisoned") = Some(surface_addr);
    }

    if is_hq {
        if debug::verbose() {
            debug::info(format!(
                "Binding {} to surface 0x{:x}",
                image_addr.name(),
                surface_addr.0
            ));
        }
        OVERLAYS
            .lock()
            .expect("OVERLAYS lock poisoned")
            .insert(surface_addr, image_addr);
    } else {
        OVERLAYS
            .lock()
            .expect("OVERLAYS lock poisoned")
            .remove(&surface_addr);
    }

    perf::end(t, &perf::BIND_IMAGE_SURFACE);
    surface
}

/// Hooks texture deletion to clear out any bound HQ overlays
///
/// Windows: stdcall calling convention (GL functions are stdcall on Windows)
/// Linux/macOS: C calling convention
///
/// The offset from `textures` pointer back to the Surface struct differs by
/// pointer width: 0x20 on 32-bit (Windows/Linux), 0x28 on 64-bit (macOS).
#[cfg(target_os = "windows")]
pub extern "stdcall" fn delete_textures(n: gl::Sizei, textures: *const gl::Uint) {
    let t = perf::begin();
    let surface_addr = SurfaceAddr(textures as usize - 0x20);
    OVERLAYS
        .lock()
        .expect("OVERLAYS lock poisoned")
        .remove(&surface_addr);

    gl::delete_textures(n, textures);
    perf::end(t, &perf::DELETE_TEXTURES);
}

#[cfg(target_os = "linux")]
pub extern "C" fn delete_textures(n: gl::Sizei, textures: *const gl::Uint) {
    let t = perf::begin();
    let surface_addr = SurfaceAddr(textures as usize - 0x20);
    OVERLAYS
        .lock()
        .expect("OVERLAYS lock poisoned")
        .remove(&surface_addr);

    gl::delete_textures(n, textures);
    perf::end(t, &perf::DELETE_TEXTURES);
}

#[cfg(target_os = "macos")]
pub extern "C" fn delete_textures(n: gl::Sizei, textures: *const gl::Uint) {
    let t = perf::begin();
    let surface_addr = SurfaceAddr(textures as usize - 0x28);
    OVERLAYS
        .lock()
        .expect("OVERLAYS lock poisoned")
        .remove(&surface_addr);

    gl::delete_textures(n, textures);
    perf::end(t, &perf::DELETE_TEXTURES);
}

/// Hooks draw preparation to set state for HQ images
pub extern "C" fn setup_draw(draw: *mut grim::Draw, index_buffer: *const c_void) {
    let t = perf::begin();

    let hq_draw = Draw::from_raw(draw).filter(|draw| draw.is_hq());

    // for hq images, use a custom shader that keeps the full resolution
    if hq_draw.is_some() {
        unsafe {
            grim::set_draw_shader(
                draw,
                grim::TEXTURED_QUAD_SHADER.inner_addr() as *mut grim::Shader,
            )
        };
    }

    grim::setup_draw(draw, index_buffer);

    if let Some(hq_draw) = hq_draw {
        // for hq images, use a linear texture filter for better quality
        gl::sampler_parameteri(
            hq_draw.sampler,
            gl::TEXTURE_MIN_FILTER,
            gl::LINEAR as gl::Int,
        );
        gl::sampler_parameteri(
            hq_draw.sampler,
            gl::TEXTURE_MAG_FILTER,
            gl::LINEAR as gl::Int,
        );

        if !hq_draw.surface.is_bitmap_underlays() {
            gl::blend_func_separate(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA, 1, 0);
        }
    }

    perf::end(t, &perf::SETUP_DRAW);
}

/// Hooks the main draw call batch to detect a video that needs cutouts
///
/// On 64-bit macOS, param_2 (rsi) and param_3 (rdx) must be pointer-sized because
/// param_3 is actually a zgIndexBuffer* that gets passed through to DrawSetup.
/// On 32-bit Linux/Windows, u32 == pointer size so this doesn't matter.
#[cfg(not(target_os = "macos"))]
pub extern "C" fn draw_indexed_primitives(
    draw: *mut grim::Draw,
    param_2: u32,
    param_3: u32,
    param_4: u32,
    param_5: u32,
) {
    let t = perf::begin();

    if image::Background::is_stencilled_video_scene()
        && Draw::from_raw(draw).is_some_and(|draw| draw.is_smush())
    {
        // Per-frame hook: install stencil draw hook before the call
        if let Err(e) = gl::draw_elements_base_vertex
            .hook(draw_elements_base_vertex as gl::DrawElementsBaseVertex)
        {
            debug::error(format!("Failed to hook draw_elements_base_vertex: {}", e));
        }
    }

    grim::draw_indexed_primitives(draw, param_2, param_3, param_4, param_5);
    perf::end(t, &perf::DRAW_INDEXED_PRIMITIVES);
}

#[cfg(target_os = "macos")]
pub extern "C" fn draw_indexed_primitives(
    draw: *mut grim::Draw,
    param_2: usize,
    param_3: usize,
    param_4: u32,
    param_5: u32,
) {
    let t = perf::begin();

    if image::Background::is_stencilled_video_scene()
        && Draw::from_raw(draw).is_some_and(|draw| draw.is_smush())
    {
        STENCIL_DRAW_ACTIVE.store(true, Ordering::Release);
    }

    grim::draw_indexed_primitives(draw, param_2, param_3, param_4, param_5);
    perf::end(t, &perf::DRAW_INDEXED_PRIMITIVES);
}

/// Hooks the opengl draw call for videos to perform a stencil test for cutouts.
///
/// macOS: persistent sub-hook, uses STENCIL_DRAW_ACTIVE flag.
/// Linux/Windows: per-frame hook that unhooks itself after use.
#[cfg(target_os = "windows")]
extern "stdcall" fn persistent_draw_elements_base_vertex(
    mode: gl::Enum,
    count: gl::Sizei,
    typ: gl::Enum,
    indicies: *mut c_void,
    basevertex: gl::Int,
) {
    if STENCIL_DRAW_ACTIVE.swap(false, Ordering::Acquire) {
        video_cutouts::with_stencil(|| {
            gl::draw_elements_base_vertex(mode, count, typ, indicies, basevertex);
        });
    } else {
        gl::draw_elements_base_vertex(mode, count, typ, indicies, basevertex);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn persistent_draw_elements_base_vertex(
    mode: gl::Enum,
    count: gl::Sizei,
    typ: gl::Enum,
    indicies: *mut c_void,
    basevertex: gl::Int,
) {
    if STENCIL_DRAW_ACTIVE.swap(false, Ordering::Acquire) {
        video_cutouts::with_stencil(|| {
            gl::draw_elements_base_vertex(mode, count, typ, indicies, basevertex);
        });
    } else {
        gl::draw_elements_base_vertex(mode, count, typ, indicies, basevertex);
    }
}

/// Per-frame hook for draw_elements_base_vertex on Linux — unhooks itself after use.
#[cfg(target_os = "linux")]
extern "C" fn draw_elements_base_vertex(
    mode: gl::Enum,
    count: gl::Sizei,
    typ: gl::Enum,
    indicies: *mut c_void,
    basevertex: gl::Int,
) {
    video_cutouts::with_stencil(|| {
        gl::draw_elements_base_vertex(mode, count, typ, indicies, basevertex);
    });

    if let Err(e) = gl::draw_elements_base_vertex.unhook() {
        debug::error(format!("Failed to unhook draw_elements_base_vertex: {}", e));
    }
}

/// Hooks texture uploads swap out regular assets for their HQ versions.
///
/// On macOS, relies on persistent GL sub-hooks gated by atomic flags.
/// On Linux/Windows, uses per-frame hook/unhook of glTexImage2D and glPixelStorei.
pub extern "C" fn surface_upload(surface: *mut grim::Surface, image_data: *mut c_void) {
    let t = perf::begin();
    let surface_addr = SurfaceAddr::from_ptr(surface);
    let target = image::get_target(surface_addr);

    if target.is_none() {
        unsafe {
            // call with null to reset the buffer size as it might have been changed by a hq image
            if !image_data.is_null() && (*surface).format < 0x10 {
                grim::surface_upload(surface, std::ptr::null_mut());
            }
            grim::surface_upload(surface, image_data);
        }
        perf::end(t, &perf::SURFACE_UPLOAD);
        return;
    }

    if image_data.is_null() {
        perf::end(t, &perf::SURFACE_UPLOAD);
        return;
    }

    *image::TARGET.lock().expect("TARGET lock poisoned") = target;

    // macOS: persistent hooks are already installed, just set the flag
    #[cfg(target_os = "macos")]
    {
        HQ_UPLOAD_ACTIVE.store(true, Ordering::Release);
        grim::surface_upload(surface, std::ptr::null_mut());
        HQ_UPLOAD_ACTIVE.store(false, Ordering::Release);
    }

    // Linux/Windows: per-frame hook/unhook
    #[cfg(not(target_os = "macos"))]
    {
        if let Err(e) = gl::tex_image_2d.hook(hq_tex_image_2d as gl::TexImage2d) {
            debug::error(format!("Failed to hook tex_image_2d: {}", e));
        }
        if let Err(e) = gl::pixel_storei.hook(hq_pixel_storei as gl::PixelStorei) {
            debug::error(format!("Failed to hook pixel_storei: {}", e));
        }

        grim::surface_upload(surface, std::ptr::null_mut());

        if let Err(e) = gl::pixel_storei.unhook() {
            debug::error(format!("Failed to unhook pixel_storei: {}", e));
        }
        if let Err(e) = gl::tex_image_2d.unhook() {
            debug::error(format!("Failed to unhook tex_image_2d: {}", e));
        }
    }

    *image::TARGET.lock().expect("TARGET lock poisoned") = None;
    perf::end(t, &perf::SURFACE_UPLOAD);
}

/// Persistent sub-hook for glTexImage2D — replaces texture data with HQ
/// version when HQ_UPLOAD_ACTIVE flag is set, otherwise passes through.
#[cfg(target_os = "windows")]
extern "stdcall" fn persistent_tex_image_2d(
    target: gl::Enum,
    level: gl::Int,
    internalformat: gl::Int,
    width: gl::Sizei,
    height: gl::Sizei,
    border: gl::Int,
    format: gl::Enum,
    typ: gl::Enum,
    data: *const c_void,
) {
    if !HQ_UPLOAD_ACTIVE.load(Ordering::Acquire) {
        gl::tex_image_2d(
            target,
            level,
            internalformat,
            width,
            height,
            border,
            format,
            typ,
            data,
        );
        return;
    }
    fn tex_image_2d(width: u32, height: u32, ptr: *const u8) {
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
    let handled = image::with_target_hq_image(|target_ref| match target_ref {
        image::TargetMut::Background(background) => {
            tex_image_2d(
                background.width,
                background.height,
                background.buffer.as_ptr(),
            );
            true
        }
        image::TargetMut::Image(hq_image) => {
            let width = hq_image.width;
            let height = hq_image.height;
            hq_image
                .data
                .get_or_wait(|buffer, _| tex_image_2d(width, height, buffer.as_ptr()))
                .is_some()
        }
    });
    if !handled {
        gl::tex_image_2d(
            target,
            level,
            internalformat,
            width,
            height,
            border,
            format,
            typ,
            data,
        );
    }
}

#[cfg(target_os = "macos")]
extern "C" fn persistent_tex_image_2d(
    target: gl::Enum,
    level: gl::Int,
    internalformat: gl::Int,
    width: gl::Sizei,
    height: gl::Sizei,
    border: gl::Int,
    format: gl::Enum,
    typ: gl::Enum,
    data: *const c_void,
) {
    if !HQ_UPLOAD_ACTIVE.load(Ordering::Acquire) {
        gl::tex_image_2d(
            target,
            level,
            internalformat,
            width,
            height,
            border,
            format,
            typ,
            data,
        );
        return;
    }
    fn tex_image_2d(width: u32, height: u32, ptr: *const u8) {
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
        );
    }
    let handled = image::with_target_hq_image(|target_ref| match target_ref {
        image::TargetMut::Background(background) => {
            tex_image_2d(
                background.width,
                background.height,
                background.buffer.as_ptr(),
            );
            true
        }
        image::TargetMut::Image(hq_image) => {
            let width = hq_image.width;
            let height = hq_image.height;
            hq_image
                .data
                .get_or_wait(|buffer, _| tex_image_2d(width, height, buffer.as_ptr()))
                .is_some()
        }
    });
    if !handled {
        gl::tex_image_2d(
            target,
            level,
            internalformat,
            width,
            height,
            border,
            format,
            typ,
            data,
        );
    }
}

/// Persistent sub-hook for glPixelStorei — adjusts row length for HQ
/// image width when HQ_UPLOAD_ACTIVE flag is set, otherwise passes through.
#[cfg(target_os = "windows")]
extern "stdcall" fn persistent_pixel_storei(pname: gl::Enum, param: gl::Int) {
    if !HQ_UPLOAD_ACTIVE.load(Ordering::Acquire) {
        gl::pixel_storei(pname, param);
        return;
    }
    let handled = image::with_target_hq_image(|target_ref| {
        if pname == gl::UNPACK_ROW_LENGTH {
            let width = match target_ref {
                image::TargetMut::Background(background) => background.width,
                image::TargetMut::Image(hq_image) => hq_image.width,
            };
            gl::pixel_storei(gl::UNPACK_ROW_LENGTH, width as gl::Int);
        } else {
            gl::pixel_storei(pname, param);
        }
        true
    });
    if !handled {
        gl::pixel_storei(pname, param);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn persistent_pixel_storei(pname: gl::Enum, param: gl::Int) {
    if !HQ_UPLOAD_ACTIVE.load(Ordering::Acquire) {
        gl::pixel_storei(pname, param);
        return;
    }
    let handled = image::with_target_hq_image(|target_ref| {
        if pname == gl::UNPACK_ROW_LENGTH {
            let width = match target_ref {
                image::TargetMut::Background(background) => background.width,
                image::TargetMut::Image(hq_image) => hq_image.width,
            };
            gl::pixel_storei(gl::UNPACK_ROW_LENGTH, width as gl::Int);
        } else {
            gl::pixel_storei(pname, param);
        }
        true
    });
    if !handled {
        gl::pixel_storei(pname, param);
    }
}

/// Per-frame sub-hook for glTexImage2D on Linux — replaces texture data with
/// HQ version. Only installed during surface_upload, so no flag check needed.
#[cfg(target_os = "linux")]
extern "C" fn hq_tex_image_2d(
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
        );
    }
    image::with_target_hq_image(|target_ref| match target_ref {
        image::TargetMut::Background(background) => {
            tex_image_2d(
                background.width,
                background.height,
                background.buffer.as_ptr(),
            );
            true
        }
        image::TargetMut::Image(hq_image) => {
            let width = hq_image.width;
            let height = hq_image.height;
            hq_image
                .data
                .get_or_wait(|buffer, _| tex_image_2d(width, height, buffer.as_ptr()))
                .is_some()
        }
    });
}

/// Per-frame sub-hook for glPixelStorei on Linux — adjusts row length for HQ
/// image width. Only installed during surface_upload, so no flag check needed.
#[cfg(target_os = "linux")]
extern "C" fn hq_pixel_storei(pname: gl::Enum, param: gl::Int) {
    image::with_target_hq_image(|target_ref| {
        if pname == gl::UNPACK_ROW_LENGTH {
            let width = match target_ref {
                image::TargetMut::Background(background) => background.width,
                image::TargetMut::Image(hq_image) => hq_image.width,
            };
            gl::pixel_storei(gl::UNPACK_ROW_LENGTH, width as gl::Int);
        } else {
            gl::pixel_storei(pname, param);
        }
        true
    });
}

/// Per-frame sub-hook for glSamplerParameteri on Linux — forces LINEAR
/// filtering. Only installed during render_scene, so no flag check needed.
#[cfg(target_os = "linux")]
extern "C" fn forced_linear_sampler_parameteri(target: gl::Enum, pname: gl::Enum, param: gl::Int) {
    if pname == gl::TEXTURE_MIN_FILTER || pname == gl::TEXTURE_MAG_FILTER {
        gl::sampler_parameteri(target, pname, gl::LINEAR as gl::Int);
    } else {
        gl::sampler_parameteri(target, pname, param);
    }
}

/// Wraps final scene draw to make the renderer toggle instant and
/// to make the image smooth on lower res displays.
///
/// On macOS, relies on persistent sampler_parameteri hook gated by FORCE_LINEAR_ACTIVE.
/// On Linux/Windows, uses per-frame hook/unhook of sampler_parameteri.
pub extern "C" fn render_scene(
    draw: *const grim::Draw,
    surface: *const grim::Surface,
    transition: f32,
) {
    let t = perf::begin();

    let value = if transition == 1.0 && Config::get().renderer.quick_toggle {
        1.0
    } else {
        unsafe { grim::RENDERING_MODE.get() }
    };

    // macOS: persistent hook is already installed, just set the flag
    #[cfg(target_os = "macos")]
    {
        FORCE_LINEAR_ACTIVE.store(true, Ordering::Release);
        grim::render_scene(draw, surface, value);
        FORCE_LINEAR_ACTIVE.store(false, Ordering::Release);
    }

    // Linux/Windows: per-frame hook/unhook
    #[cfg(not(target_os = "macos"))]
    {
        if let Err(e) =
            gl::sampler_parameteri.hook(forced_linear_sampler_parameteri as gl::SamplerParameteri)
        {
            debug::error(format!("Failed to hook sampler_parameteri: {}", e));
        }
        grim::render_scene(draw, surface, value);
        if let Err(e) = gl::sampler_parameteri.unhook() {
            debug::error(format!("Failed to unhook sampler_parameteri: {}", e));
        }
    }

    if let Some(start) = t {
        let elapsed = start.elapsed().as_nanos() as u64;
        perf::RENDER_SCENE.record(elapsed);
        perf::frame_tick(elapsed);
    }
}

/// Persistent sub-hook for glSamplerParameteri — forces LINEAR filtering
/// only when FORCE_LINEAR_ACTIVE flag is set, otherwise passes through.
#[cfg(target_os = "windows")]
extern "stdcall" fn persistent_sampler_parameteri(
    target: gl::Enum,
    pname: gl::Enum,
    param: gl::Int,
) {
    if !FORCE_LINEAR_ACTIVE.load(Ordering::Acquire) {
        gl::sampler_parameteri(target, pname, param);
        return;
    }
    if pname == gl::TEXTURE_MIN_FILTER || pname == gl::TEXTURE_MAG_FILTER {
        gl::sampler_parameteri(target, pname, gl::LINEAR as gl::Int);
    } else {
        gl::sampler_parameteri(target, pname, param);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn persistent_sampler_parameteri(target: gl::Enum, pname: gl::Enum, param: gl::Int) {
    if !FORCE_LINEAR_ACTIVE.load(Ordering::Acquire) {
        gl::sampler_parameteri(target, pname, param);
        return;
    }
    if pname == gl::TEXTURE_MIN_FILTER || pname == gl::TEXTURE_MAG_FILTER {
        gl::sampler_parameteri(target, pname, gl::LINEAR as gl::Int);
    } else {
        gl::sampler_parameteri(target, pname, param);
    }
}

/// Hooks compressed texture uploads — passes through to the ARB version.
#[cfg(target_os = "windows")]
pub extern "stdcall" fn compressed_tex_image2d(
    target: gl::Enum,
    level: gl::Int,
    internalformat: gl::Enum,
    width: gl::Sizei,
    height: gl::Sizei,
    border: gl::Int,
    image_size: gl::Sizei,
    data: *const c_void,
) {
    let t = perf::begin();
    gl::compressed_tex_image2d(
        target,
        level,
        internalformat,
        width,
        height,
        border,
        image_size,
        data,
    );
    perf::end(t, &perf::COMPRESSED_TEX_IMAGE);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub extern "C" fn compressed_tex_image2d(
    target: gl::Enum,
    level: gl::Int,
    internalformat: gl::Enum,
    width: gl::Sizei,
    height: gl::Sizei,
    border: gl::Int,
    image_size: gl::Sizei,
    data: *const c_void,
) {
    let t = perf::begin();
    gl::compressed_tex_image2d(
        target,
        level,
        internalformat,
        width,
        height,
        border,
        image_size,
        data,
    );
    perf::end(t, &perf::COMPRESSED_TEX_IMAGE);
}
