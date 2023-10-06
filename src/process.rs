use windows::core::PCWSTR;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS,
};
use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
use windows::Win32::System::Threading::GetCurrentProcess;

pub struct Process {
    pub base_address: usize,
}

impl Process {
    pub fn new(base_address: usize) -> Process {
        Process { base_address }
    }

    pub fn init() -> Option<Process> {
        Process::get_base_address().map(Process::new)
    }

    pub fn get_base_address() -> Option<usize> {
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

    pub unsafe fn read<T: Sized + Copy>(&self, address: usize) -> T {
        let fixed = self.base_address + address;
        *(fixed as *const T)
    }

    pub unsafe fn write<T: Sized>(&self, address: usize, value: T) {
        let fixed = self.base_address + address;
        let mut existing_flags: PAGE_PROTECTION_FLAGS = std::mem::zeroed();

        VirtualProtect(
            address as *mut _,
            std::mem::size_of::<T>(),
            PAGE_EXECUTE_READWRITE,
            &mut existing_flags,
        );

        *(fixed as *mut T) = value;

        VirtualProtect(
            address as *mut _,
            std::mem::size_of::<T>(),
            existing_flags,
            &mut existing_flags,
        );
    }
}
