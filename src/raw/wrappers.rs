use std::ffi::CString;
use windows::core::PCSTR;
use windows::Win32::Foundation::{HMODULE, MAX_PATH};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows::Win32::System::SystemInformation::GetSystemDirectoryA;

use super::memory::BoundFn;

pub struct Dll {
    raw: HMODULE,
}

impl Dll {
    pub fn get_proc_addr(&self, name: &str) -> Option<usize> {
        let proc_cname = CString::new(name).ok()?;
        let proc =
            unsafe { GetProcAddress(self.raw, PCSTR::from_raw(proc_cname.as_ptr() as *const u8)) };
        proc.map(|f| f as usize)
    }

    pub fn bind<F>(&self, unbound_fn: &BoundFn<F>, proc_name: &str) -> Result<(), String> {
        let proc_addr = self
            .get_proc_addr(proc_name)
            .ok_or_else(|| proc_name.to_string())?;
        unbound_fn.bind(proc_addr);
        Ok(())
    }
}

pub enum DllError {
    DllNotOpened(String),
    ProcNotFound(String, String),
}

impl std::fmt::Display for DllError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DllError::DllNotOpened(dll) => write!(f, "Could not open '{}'", dll),
            DllError::ProcNotFound(dll, proc) => {
                write!(f, "Could not find '{}' function in '{}'", proc, dll)
            }
        }
    }
}

/// Load and use a system DLL with the intent of providing its functions for the
/// entire lifetime of the application.
///
/// Since the underlying DLL handle is never exposed and can't be freed,
/// the function pointers can be considered as always valid.
pub fn with_system_dll<F>(name: &str, f: F) -> Result<(), DllError>
where
    F: Fn(Dll) -> Result<(), String>,
{
    let err = || DllError::DllNotOpened(name.to_string());
    if !name.chars().all(|c| c.is_alphanumeric() || c == '.') {
        return Err(err());
    }
    let dll_path = CString::new(system_dll_path(name)).map_err(|_| err())?;
    // SAFETY: The path is known to be a well-formed ascii cstring
    let raw_dll = unsafe { LoadLibraryA(PCSTR::from_raw(dll_path.as_ptr() as *const u8)).ok() };
    let dll = Dll {
        raw: raw_dll.ok_or_else(err)?,
    };
    f(dll).map_err(|err| DllError::ProcNotFound(name.to_string(), err))
}

/// Find the system path for a DLL, e.g. C:\Windows\SysWOW64\opengl32.dll
pub fn system_dll_path(name: &str) -> String {
    let mut buffer = vec![0u8; MAX_PATH as usize];
    // SAFETY: With buffer defined and set to MAX_PATH, this can't reasonably fail
    let length = unsafe { GetSystemDirectoryA(Some(&mut buffer)) };
    let mut path = String::from_utf8_lossy(&buffer[..length as usize]).to_string();
    path.push('\\');
    path.push_str(name);
    path
}
