use crate::{proxy, raw::glu32};
use std::ffi::{c_uint, c_void};

proxy! {
    #[with(glu32::error_string)]
    extern "stdcall" fn gluErrorString(error_code: c_uint) -> *const u8;

    #[with(glu32::tess_begin_contour)]
    extern "stdcall" fn gluTessBeginContour(tess: *mut c_void);

    #[with(glu32::tess_end_contour)]
    extern "stdcall" fn gluTessEndContour(tess: *mut c_void);

    #[with(glu32::tess_begin_polygon)]
    extern "stdcall" fn gluTessBeginPolygon(tess: *mut c_void, data: *mut c_void);

    #[with(glu32::tess_end_polygon)]
    extern "stdcall" fn gluTessEndPolygon(tess: *mut c_void);

    #[with(glu32::new_tess)]
    extern "stdcall" fn gluNewTess() -> *const c_void;

    #[with(glu32::delete_tess)]
    extern "stdcall" fn gluDeleteTess(tess: *mut c_void);

    #[with(glu32::tess_property)]
    extern "stdcall" fn gluTessProperty(tess: *mut c_void, which: c_uint, data: f64);

    #[with(glu32::tess_normal)]
    extern "stdcall" fn gluTessNormal(tess: *mut c_void, x: f64, y: f64, z: f64);

    #[with(glu32::tess_callback)]
    extern "stdcall" fn gluTessCallback(tess: *mut c_void, which: c_uint, cb: *mut c_void);

    #[with(glu32::tess_vertex)]
    extern "stdcall" fn gluTessVertex(tess: *mut c_void, location: *mut f64, data: *mut c_void);
}
