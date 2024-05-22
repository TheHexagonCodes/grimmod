#![allow(non_upper_case_globals)]

use std::ffi::{c_uint, c_void};

use crate::{
    direct_fns,
    raw::wrappers::{with_system_dll, DllError},
};

direct_fns! {
    extern "stdcall" fn error_string(error_code: c_uint) -> *const u8;
    extern "stdcall" fn tess_begin_contour(tess: *mut c_void);
    extern "stdcall" fn tess_end_contour(tess: *mut c_void);
    extern "stdcall" fn tess_begin_polygon(tess: *mut c_void, data: *mut c_void);
    extern "stdcall" fn tess_end_polygon(tess: *mut c_void);
    extern "stdcall" fn new_tess() -> *mut c_void;
    extern "stdcall" fn delete_tess(tess: *mut c_void);
    extern "stdcall" fn tess_property(tess: *mut c_void, which: c_uint, data: f64);
    extern "stdcall" fn tess_normal(tess: *mut c_void, x: f64, y: f64, z: f64);
    extern "stdcall" fn tess_callback(tess: *mut c_void, which: c_uint, cb: *mut c_void);
    extern "stdcall" fn tess_vertex(tess: *mut c_void, location: *mut f64, data: *mut c_void);
}

pub fn bind_fns() -> Result<(), DllError> {
    with_system_dll("glu32.dll", |dll| {
        dll.bind(&error_string, "gluErrorString")?;
        dll.bind(&tess_begin_contour, "gluTessBeginContour")?;
        dll.bind(&tess_end_contour, "gluTessEndContour")?;
        dll.bind(&tess_begin_polygon, "gluTessBeginPolygon")?;
        dll.bind(&tess_end_polygon, "gluTessEndPolygon")?;
        dll.bind(&new_tess, "gluNewTess")?;
        dll.bind(&delete_tess, "gluDeleteTess")?;
        dll.bind(&tess_property, "gluTessProperty")?;
        dll.bind(&tess_normal, "gluTessNormal")?;
        dll.bind(&tess_callback, "gluTessCallback")?;
        dll.bind(&tess_vertex, "gluTessVertex")?;

        Ok(())
    })
}
