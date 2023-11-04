use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS,
};
use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
use windows::Win32::System::Threading::GetCurrentProcess;

static BASE_ADDRESS: Mutex<usize> = Mutex::new(0);

/// A memory address, either absolute or relative to the base address
pub enum Address {
    Absolute(usize),
    Relative(usize),
}

impl Address {
    pub fn absolute(self) -> usize {
        match self {
            Address::Absolute(address) => address,
            Address::Relative(address) => base_address() + address,
        }
    }
}

pub fn init() -> bool {
    match calculate_base_address() {
        Some(base_address) => {
            *BASE_ADDRESS.lock().unwrap() = base_address;
            true
        }
        None => false,
    }
}

pub fn calculate_base_address() -> Option<usize> {
    unsafe {
        let process_handle = GetCurrentProcess();
        let hmodule = GetModuleHandleW(PCWSTR::null()).ok()?;
        let mut module_info = std::mem::zeroed::<MODULEINFO>();
        let result = GetModuleInformation(
            process_handle,
            hmodule,
            &mut module_info as *mut MODULEINFO,
            std::mem::size_of::<MODULEINFO>() as u32,
        );
        (result == BOOL(1)).then_some(module_info.lpBaseOfDll as usize)
    }
}

pub fn base_address() -> usize {
    *BASE_ADDRESS.lock().unwrap()
}

pub unsafe fn read<T: Sized + Copy>(address: Address) -> T {
    *(address.absolute() as *const T)
}

pub unsafe fn with_mut_ref<T, F: FnOnce(&mut T)>(address: Address, block: F) {
    let absolute = address.absolute();
    let mut existing_flags: PAGE_PROTECTION_FLAGS = std::mem::zeroed();

    VirtualProtect(
        absolute as *mut _,
        std::mem::size_of::<T>(),
        PAGE_EXECUTE_READWRITE,
        &mut existing_flags,
    );

    let value = &mut *(absolute as *mut T);
    block(value);

    VirtualProtect(
        absolute as *mut _,
        std::mem::size_of::<T>(),
        existing_flags,
        &mut existing_flags,
    );
}

pub unsafe fn write<T: Sized>(address: Address, value: T) {
    with_mut_ref(address, |reference| {
        *reference = value;
    });
}
