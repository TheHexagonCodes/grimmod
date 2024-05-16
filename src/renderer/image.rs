use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

use crate::config::Config;
use crate::renderer::graphics::{
    Image, ImageAddr, ImageContainer, ImageContainerAddr, SurfaceAddr, OVERLAYS,
};
use crate::renderer::{animation, video_cutouts};
use crate::{debug, file};

pub static BACKGROUND: Mutex<Option<Background>> = Mutex::new(None);
pub static BACKGROUND_WRITES: Lazy<Mutex<BackgroundWrites>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
pub static TARGET: Mutex<Option<Target>> = Mutex::new(None);

pub static HQ_IMAGES: Lazy<Mutex<Vec<HqImageContainer>>> = Lazy::new(|| Mutex::new(Vec::new()));

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

        if debug::verbose() {
            let addrs: Vec<_> = hq_images.iter().map(HqImage::format_addr).collect();
            debug::info(format!("Opened HQ {} ({})", name, addrs.join(", ")));
        }

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
            original_addr: image.addr,
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
                    name: format!("{} ({:02})", name, i + 1),
                    index: i,
                    width,
                    height,
                    scale: width / image.width as u32,
                    original_addr: image.addr,
                    data: dst,
                })
                .collect(),
        )
    }

    pub fn name(image_addr: ImageAddr) -> Option<String> {
        HqImage::map_loaded(image_addr, &mut HQ_IMAGES.lock().unwrap(), |hq_image| {
            Some(hq_image.name.clone())
        })
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
        video_cutouts::bind_for(&self.name);
        self.data.get_or_wait(|buffer, _| Background {
            name: self.name.clone(),
            width: self.width,
            height: self.height,
            scale: self.scale,
            original_addr: self.original_addr,
            buffer: buffer.to_vec(),
        })
    }

    fn format_addr(&self) -> String {
        format!("0x{:x}", self.original_addr.underlying())
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

    pub fn get_or_wait<F, R>(&mut self, mut f: F) -> Option<R>
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

pub type BackgroundWrites = HashMap<(u32, u32), (u32, u32)>;

pub struct Background {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    pub original_addr: ImageAddr,
    pub buffer: Vec<u8>,
}

impl Background {
    /// Write (or draw over) the HQ background
    pub fn write(image: Image, x: u32, y: u32) {
        if x == 0 && y == 0 && image.width == 640 && image.height == 480 {
            if debug::verbose() {
                let name = HqImage::name(image.addr).unwrap_or_default();
                debug::info(format!("Setting {} as background", name));
            }
            Background::set_from_image(image.addr, &mut HQ_IMAGES.lock().unwrap());
        } else {
            HqImage::with_loaded_or_else(
                image.addr,
                &mut HQ_IMAGES.lock().unwrap(),
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

    pub fn is_stencilled_video_scene() -> bool {
        if Config::get().renderer.video_cutouts
            && let Some(background) = BACKGROUND.lock().unwrap().as_ref()
        {
            video_cutouts::triangles_for(&background.name).is_some()
        } else {
            false
        }
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

pub fn get_target(surface_addr: SurfaceAddr) -> Option<Target> {
    if surface_addr.is_bitmap_underlays() {
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

pub fn with_target_hq_image<F: FnMut(TargetMut)>(mut f: F) {
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
