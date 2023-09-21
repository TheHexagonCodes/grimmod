use windows::core::PCWSTR;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
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

    pub fn read<T: Sized>(&self, offset: usize) -> T {
        let address = self.base_address + offset;

        unsafe {
            let process_handle = GetCurrentProcess();
            let mut value: T = std::mem::zeroed();

            ReadProcessMemory(
                process_handle,
                address as *mut _,
                &mut value as *mut T as *mut _,
                std::mem::size_of::<T>(),
                None,
            );

            value
        }
    }

    pub fn write<T: Sized>(&self, offset: usize, value: T) {
        let address = self.base_address + offset;
        let size = std::mem::size_of::<T>();

        unsafe {
            let process_handle = GetCurrentProcess();
            let _result = WriteProcessMemory(
                process_handle,
                address as *mut _,
                &value as *const T as *const _,
                size,
                None,
            );
        }
    }
}
