use paste::paste;
use std::ffi::{c_uint, c_void, CString};
use std::mem::transmute;
use windows::core::PCSTR;
use windows::Win32::Foundation::{HMODULE, MAX_PATH};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows::Win32::System::SystemInformation::GetSystemDirectoryA;

macro_rules! proxy {
    (fn $name:ident($($arg_name:ident : $arg_ty:ty),*)) => {
        proxy!(fn $name($($arg_name: $arg_ty),*) -> ());
    };
    (fn $name:ident($($arg_name:ident : $arg_ty:ty),*) -> $ret_ty:ty) => {
        paste! {
            type [<$name:camel>] = unsafe extern "system" fn($($arg_name: $arg_ty),*) -> $ret_ty;

            static mut [<$name:snake:upper>]: Option<[<$name:camel>]> = None;

            #[no_mangle]
            pub unsafe extern "system" fn $name($($arg_name: $arg_ty),*) -> $ret_ty {
                let original_fn = paste! { [<$name:snake:upper>] }.unwrap();
                let out = original_fn($($arg_name),*);
                out
            }
        }
    };
}

proxy!(fn gluErrorString(error_code: c_uint) -> *const u8);
proxy!(fn gluTessBeginContour(tess: *mut c_void));
proxy!(fn gluTessEndContour(tess: *mut c_void));
proxy!(fn gluTessBeginPolygon(tess: *mut c_void, data: *mut c_void));
proxy!(fn gluTessEndPolygon(tess: *mut c_void));
proxy!(fn gluNewTess() -> *mut c_void);
proxy!(fn gluDeleteTess(tess: *mut c_void));
proxy!(fn gluTessProperty(tess: *mut c_void, which: c_uint, data: f64));
proxy!(fn gluTessNormal(tess: *mut c_void, x: f64, y: f64, z: f64));
proxy!(fn gluTessCallback(tess: *mut c_void, which: c_uint, cb: *mut c_void));
proxy!(fn gluTessVertex(tess: *mut c_void, location: *mut f64, data: *mut c_void));

pub unsafe fn attach() -> Option<()> {
    let glu32 = load_library(&system_glu32_path())?;

    GLU_ERROR_STRING = transmute(get_proc(glu32, "gluErrorString"));
    GLU_TESS_BEGIN_CONTOUR = transmute(get_proc(glu32, "gluTessBeginContour"));
    GLU_TESS_END_CONTOUR = transmute(get_proc(glu32, "gluTessEndContour"));
    GLU_TESS_BEGIN_POLYGON = transmute(get_proc(glu32, "gluTessBeginPolygon"));
    GLU_TESS_END_POLYGON = transmute(get_proc(glu32, "gluTessEndPolygon"));
    GLU_NEW_TESS = transmute(get_proc(glu32, "gluNewTess"));
    GLU_DELETE_TESS = transmute(get_proc(glu32, "gluDeleteTess"));
    GLU_TESS_PROPERTY = transmute(get_proc(glu32, "gluTessProperty"));
    GLU_TESS_NORMAL = transmute(get_proc(glu32, "gluTessNormal"));
    GLU_TESS_CALLBACK = transmute(get_proc(glu32, "gluTessCallback"));
    GLU_TESS_VERTEX = transmute(get_proc(glu32, "gluTessVertex"));

    Some(())
}

pub fn load_library(dll_path: &str) -> Option<HMODULE> {
    let dll_path = CString::new(dll_path).ok()?;
    unsafe { LoadLibraryA(PCSTR::from_raw(dll_path.as_ptr() as *const u8)).ok() }
}

pub fn system_glu32_path() -> String {
    let mut buffer = vec![0u8; MAX_PATH as usize];
    let length = unsafe { GetSystemDirectoryA(Some(&mut buffer)) };
    let mut path = String::from_utf8_lossy(&buffer[..length as usize]).to_string();
    path.push_str("\\glu32.dll");
    path
}

pub fn get_proc(dll: HMODULE, proc: &str) -> Option<unsafe extern "system" fn() -> isize> {
    let proc = CString::new(proc).ok()?;
    unsafe { GetProcAddress(dll, PCSTR::from_raw(proc.as_ptr() as *const u8)) }
}
