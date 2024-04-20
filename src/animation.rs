use std::fs::File;
use std::path::Path;
use std::slice;
use webm_iterable::{
    matroska_spec::{Block, MatroskaSpec},
    WebmIterator,
};

pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub buffer: Vec<u8>,
}

pub fn open<P: AsRef<Path>>(path: P) -> Option<Vec<Frame>> {
    let mut src = File::open(path).ok()?;
    let mut frames = Vec::new();

    unsafe {
        let mut codec = std::mem::zeroed();
        let config = vpx_sys::vpx_codec_dec_cfg {
            threads: 4,
            w: 0,
            h: 0,
        };
        let init_result = vpx_sys::vpx_codec_dec_init_ver(
            &mut codec,
            vpx_sys::vpx_codec_vp9_dx(),
            &config,
            0,
            vpx_sys::VPX_DECODER_ABI_VERSION as i32,
        );

        if init_result != vpx_sys::VPX_CODEC_OK {
            return None;
        }

        let tag_iterator = WebmIterator::new(&mut src, &[]);

        let mut vpx_iter = std::ptr::null();
        for tag in tag_iterator {
            let Ok(MatroskaSpec::SimpleBlock(ref data)) = tag else {
                continue;
            };
            let block: Block = data.try_into().unwrap();
            let decode_result = vpx_sys::vpx_codec_decode(
                &mut codec,
                block.raw_frame_data().as_ptr(),
                block.raw_frame_data().len() as u32,
                std::ptr::null_mut(),
                0,
            );

            if decode_result != vpx_sys::VPX_CODEC_OK {
                return None;
            }

            while let Some(image) = vpx_sys::vpx_codec_get_frame(&mut codec, &mut vpx_iter).as_ref()
            {
                frames.push(Frame {
                    width: image.d_w,
                    height: image.d_h,
                    has_alpha: true,
                    buffer: vpx_to_rgb(image),
                });
            }
        }
    }

    Some(frames)
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
