use lazy_static::lazy_static;
use retour::RawDetour;
use std::marker::PhantomData;
use std::mem;
use windows::core::PCWSTR;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS,
};
use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
use windows::Win32::System::Threading::GetCurrentProcess;

use crate::debug;

lazy_static! {
    pub static ref BASE_ADDRESS: usize = base_address().unwrap_or(0);
}

pub fn base_address() -> Option<usize> {
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

pub fn relative_address(address: usize) -> usize {
    *BASE_ADDRESS + address
}

pub unsafe fn read<T: Sized + Copy>(address: usize) -> T {
    *(address as *const T)
}

pub unsafe fn with_mut_ref<T, F: FnOnce(&mut T)>(address: usize, block: F) {
    let mut existing_flags: PAGE_PROTECTION_FLAGS = std::mem::zeroed();

    VirtualProtect(
        address as *mut _,
        std::mem::size_of::<T>(),
        PAGE_EXECUTE_READWRITE,
        &mut existing_flags,
    );

    let value = &mut *(address as *mut T);
    block(value);

    VirtualProtect(
        address as *mut _,
        std::mem::size_of::<T>(),
        existing_flags,
        &mut existing_flags,
    );
}

pub unsafe fn write<T: Sized>(address: usize, value: T) {
    with_mut_ref(address, |reference| {
        *reference = value;
    });
}

pub struct DirectFn<F> {
    pub name: &'static str,
    pub relative_address: usize,
    pub enabled: bool,
    pub hook: Option<RawDetour>,
    fn_type: PhantomData<F>,
}

impl<F> DirectFn<F> {
    pub const fn new(name: &'static str, relative_address: usize) -> DirectFn<F> {
        DirectFn {
            name,
            relative_address,
            enabled: false,
            hook: None,
            fn_type: PhantomData,
        }
    }

    pub unsafe fn prepare_hook(&mut self, replacement: F) {
        let replacement_addr = *(&replacement as *const F as *const usize);
        let hook = RawDetour::new(
            relative_address(self.relative_address) as *const (),
            replacement_addr as *const (),
        );
        match hook {
            Ok(hook) => {
                self.hook = Some(hook);
            }
            Err(error) => {
                debug::error(format!("Could not hook function {}: {}", self.name, error));
            }
        }
    }

    pub unsafe fn enable_hook(&mut self) {
        if let Some(hook) = &self.hook {
            if let Err(error) = hook.enable() {
                debug::error(format!(
                    "Could not enable hook for function {}: {}",
                    self.name, error
                ));
            } else {
                self.enabled = true;
            }
        }
    }

    pub unsafe fn hook(&mut self, replacement: F) {
        self.prepare_hook(replacement);
        self.enable_hook();
    }

    pub unsafe fn fn_addr(&self) -> usize {
        if self.enabled
            && let Some(hook) = &self.hook
        {
            hook.trampoline() as *const _ as usize
        } else {
            relative_address(self.relative_address)
        }
    }
}

macro_rules! impl_direct_fn_traits {
    ($($T:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($T,)* R> FnOnce<($($T,)*)> for DirectFn<extern "C" fn($($T,)*) -> R> {
            type Output = R;

            extern "rust-call" fn call_once(self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern "C" fn($($T,)*) -> R = unsafe { mem::transmute(self.fn_addr()) };
                (f)($($T,)*)
            }
        }

        #[allow(non_snake_case)]
        impl<$($T, )* R> FnMut<($($T,)*)> for DirectFn<extern "C" fn($($T,)*) -> R> {
            extern "rust-call" fn call_mut(&mut self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern "C" fn($($T,)*) -> R = unsafe { mem::transmute(self.fn_addr()) };
                (f)($($T,)*)
            }
        }

        #[allow(non_snake_case)]
        impl<$($T, )* R> Fn<($($T,)*)> for DirectFn<extern "C" fn($($T,)*) -> R> {
            extern "rust-call" fn call(&self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern "C" fn($($T,)*) -> R = unsafe { mem::transmute(self.fn_addr()) };
                (f)($($T,)*)
            }
        }
    };
}

impl_direct_fn_traits!();
impl_direct_fn_traits!(A);
impl_direct_fn_traits!(A, B);
impl_direct_fn_traits!(A, B, C);
impl_direct_fn_traits!(A, B, C, D);
impl_direct_fn_traits!(A, B, C, D, E);
impl_direct_fn_traits!(A, B, C, D, E, F);
impl_direct_fn_traits!(A, B, C, D, E, F, G);
impl_direct_fn_traits!(A, B, C, D, E, F, G, H);
