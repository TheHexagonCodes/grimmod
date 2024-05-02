use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::slice;
use std::thread;
use webm_iterable::{
    matroska_spec::{Block, Master, MatroskaSpec},
    WebmIterator,
};

use crate::debug;
use crate::image::HqImageAsyncData;

struct Decoder {
    codec: vpx_sys::vpx_codec_ctx_t,
    vpx_iter: vpx_sys::vpx_codec_iter_t,
    mode: DecoderMode,
}

impl Decoder {
    pub fn new(mode: DecoderMode) -> Option<Decoder> {
        let mut codec = unsafe { std::mem::zeroed() };
        let vpx_iter = null();
        let config = vpx_sys::vpx_codec_dec_cfg {
            threads: 4,
            w: 0,
            h: 0,
        };
        let init_result = unsafe {
            vpx_sys::vpx_codec_dec_init_ver(
                &mut codec,
                vpx_sys::vpx_codec_vp9_dx(),
                &config,
                0,
                vpx_sys::VPX_DECODER_ABI_VERSION as i32,
            )
        };
        if init_result != vpx_sys::VPX_CODEC_OK {
            return None;
        }
        Some(Decoder {
            codec,
            vpx_iter,
            mode,
        })
    }

    pub fn decode<F>(&mut self, data: &Vec<u8>, f: F) -> Option<Vec<u8>>
    where
        F: Fn(&vpx_sys::vpx_image_t) -> Vec<u8>,
    {
        let (data, data_size) = if let DecoderMode::Color = self.mode {
            let block: Block = data.try_into().ok()?;
            let data = block.raw_frame_data();
            (data.as_ptr(), data.len())
        } else {
            (data.as_slice().as_ptr(), data.len())
        };
        let decode_result = unsafe {
            vpx_sys::vpx_codec_decode(&mut self.codec, data, data_size as u32, null_mut(), 0)
        };

        if decode_result != vpx_sys::VPX_CODEC_OK {
            return None;
        }

        let image =
            unsafe { vpx_sys::vpx_codec_get_frame(&mut self.codec, &mut self.vpx_iter).as_ref()? };
        Some(f(image))
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            vpx_sys::vpx_codec_destroy(&mut self.codec);
        }
    }
}

pub enum DecoderMode {
    Color,
    Alpha,
}

pub fn open<P: AsRef<Path>>(path: P, datas: Vec<HqImageAsyncData>) -> Option<(u32, u32)> {
    let src = File::open(&path).ok()?;
    let mut reader = BufReader::new(src);
    let (mut width, mut height) = (None, None);

    for tag in WebmIterator::new(&mut reader, &[]) {
        match tag {
            Ok(MatroskaSpec::PixelWidth(pixel_width)) => {
                width = Some(pixel_width as u32);
                if height.is_some() {
                    break;
                }
            }
            Ok(MatroskaSpec::PixelHeight(pixel_height)) => {
                height = Some(pixel_height as u32);
                if width.is_some() {
                    break;
                }
            }
            _ => {}
        }
    }

    let path = path.as_ref().to_owned();
    thread::spawn(move || {
        let mut datas = datas.into_iter();
        let result = decode(&path, &mut datas);
        if result.is_none() {
            debug::error("error while decoding animation frames");
            datas.for_each(|mut data| data.failed());
        }
    });

    width.zip(height)
}

fn decode(path: &Path, datas: &mut impl Iterator<Item = HqImageAsyncData>) -> Option<()> {
    let mut src = File::open(path).ok()?;
    let mut color_decoder = Decoder::new(DecoderMode::Color)?;
    let mut alpha_decoder = Decoder::new(DecoderMode::Alpha)?;

    let mut block_id = 0;
    let mut has_alpha = false;
    let mut buffer = Vec::new();
    let mut alpha = Vec::new();

    for tag in WebmIterator::new(&mut src, &[]) {
        match tag {
            Ok(MatroskaSpec::AlphaMode(mode)) => {
                has_alpha = mode == 1;
            }
            Ok(MatroskaSpec::SimpleBlock(data)) => {
                let decoded = color_decoder.decode(&data, vpx_to_rgb)?;
                datas.next()?.loaded(decoded, has_alpha);
            }
            Ok(MatroskaSpec::Block(data)) => {
                buffer = color_decoder.decode(&data, vpx_to_rgb)?;
            }
            Ok(MatroskaSpec::BlockAddID(block_add_id)) => {
                block_id = block_add_id;
            }
            Ok(MatroskaSpec::BlockAdditional(data)) if block_id == 1 => {
                alpha = alpha_decoder.decode(&data, vpx_to_alpha)?;
            }
            Ok(MatroskaSpec::BlockGroup(Master::End)) => {
                block_id = 0;
                let mut data = datas.next()?;
                merge_alpha(&mut buffer, &mut alpha);
                data.loaded(buffer, has_alpha);
                buffer = Vec::new();
            }
            _ => {}
        }
    }

    Some(())
}

fn merge_alpha(rgba: &mut [u8], alpha: &mut [u8]) {
    if rgba.len() == alpha.len() * 4 {
        for i in 0..alpha.len() {
            rgba[i * 4 + 3] = alpha[i];
        }
    }
}

fn yuv_to_rgb(y: u8, u: u8, v: u8) -> (u8, u8, u8) {
    let shifted_y = 298 * (y as i32 - 16);
    let r: i32 = (shifted_y + 409 * (v as i32 - 128) + 128) >> 8;
    let g: i32 = (shifted_y - 100 * (u as i32 - 128) - 208 * (v as i32 - 128) + 128) >> 8;
    let b: i32 = (shifted_y + 516 * (u as i32 - 128) + 128) >> 8;

    let r = r.clamp(0, 255) as u8;
    let g = g.clamp(0, 255) as u8;
    let b = b.clamp(0, 255) as u8;

    (r, g, b)
}

fn vpx_to_rgb(vpx_img: &vpx_sys::vpx_image_t) -> Vec<u8> {
    let width = vpx_img.d_w as usize;
    let height = vpx_img.d_h as usize;

    let y_stride = vpx_img.stride[0] as usize;
    let uv_stride = vpx_img.stride[1] as usize;

    let y_plane = unsafe { slice::from_raw_parts(vpx_img.planes[0], height * y_stride) };
    let u_plane = unsafe { slice::from_raw_parts(vpx_img.planes[1], (height / 2) * uv_stride) };
    let v_plane = unsafe { slice::from_raw_parts(vpx_img.planes[2], (height / 2) * uv_stride) };

    let mut rgb_image = Vec::with_capacity(width * height * 4);

    for y in 0..height {
        for x in 0..width {
            let y_index = y * y_stride + x;
            let uv_index = (y / 2) * uv_stride + (x / 2);

            let y_pixel = y_plane[y_index];
            let u_pixel = u_plane[uv_index];
            let v_pixel = v_plane[uv_index];

            let (r, g, b) = yuv_to_rgb(y_pixel, u_pixel, v_pixel);
            rgb_image.extend_from_slice(&[r, g, b, 255]);
        }
    }

    rgb_image
}

fn vpx_to_alpha(vpx_img: &vpx_sys::vpx_image_t) -> Vec<u8> {
    let width = vpx_img.d_w as usize;
    let height = vpx_img.d_h as usize;
    let y_stride = vpx_img.stride[0] as usize;
    let y_plane = unsafe { slice::from_raw_parts(vpx_img.planes[0], height * y_stride) };
    let mut alpha = Vec::with_capacity(width * height);

    for y in 0..height {
        for x in 0..width {
            let y_index = y * y_stride + x;
            let y_pixel = y_plane[y_index];
            alpha.push(y_pixel);
        }
    }

    alpha
}
