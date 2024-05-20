use once_cell::sync::Lazy;
use retour::RawDetour;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem;
use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS,
};
use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
use windows::Win32::System::Threading::GetCurrentProcess;

use crate::debug;

pub static BASE_ADDRESS: Lazy<usize> = Lazy::new(|| base_address().unwrap_or(0));

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

pub unsafe fn as_ref<'a, T: Sized>(address: usize) -> Option<&'a T> {
    (address as *const T).as_ref()
}

pub unsafe fn read<T: Sized + Clone + Default>(address: usize) -> T {
    let value_ref = as_ref(address);
    value_ref.cloned().unwrap_or_default()
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

macro_rules! impl_direct_extern_fn_traits {
    ($conv:literal, $($T:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($T,)* R> FnOnce<($($T,)*)> for DirectFn<extern $conv fn($($T,)*) -> R> {
            type Output = R;

            extern "rust-call" fn call_once(self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern $conv fn($($T,)*) -> R = unsafe { mem::transmute(self.fn_addr()) };
                (f)($($T,)*)
            }
        }

        #[allow(non_snake_case)]
        impl<$($T, )* R> FnMut<($($T,)*)> for DirectFn<extern $conv fn($($T,)*) -> R> {
            extern "rust-call" fn call_mut(&mut self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern $conv fn($($T,)*) -> R = unsafe { mem::transmute(self.fn_addr()) };
                (f)($($T,)*)
            }
        }

        #[allow(non_snake_case)]
        impl<$($T, )* R> Fn<($($T,)*)> for DirectFn<extern $conv fn($($T,)*) -> R> {
            extern "rust-call" fn call(&self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern $conv fn($($T,)*) -> R = unsafe { mem::transmute(self.fn_addr()) };
                (f)($($T,)*)
            }
        }
    };
}

macro_rules! impl_direct_fn_traits {
    ($($T:ident),*) => {
        impl_direct_extern_fn_traits!("C", $($T),*);
        impl_direct_extern_fn_traits!("stdcall", $($T),*);
        impl_direct_extern_fn_traits!("fastcall", $($T),*);
    }
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
impl_direct_fn_traits!(A, B, C, D, E, F, G, H, I);
impl_direct_fn_traits!(A, B, C, D, E, F, G, H, I, J);

pub struct IndirectFn<F> {
    pub name: &'static str,
    pub relative_address: usize,
    pub hooked: Option<usize>,
    fn_type: PhantomData<F>,
}

impl<F> IndirectFn<F> {
    pub const fn new(name: &'static str, relative_address: usize) -> IndirectFn<F> {
        IndirectFn {
            name,
            relative_address,
            hooked: None,
            fn_type: PhantomData,
        }
    }

    pub fn address(&self) -> usize {
        relative_address(self.relative_address)
    }

    pub unsafe fn hook(&mut self, replacement: F) {
        let replacement_addr = *(&replacement as *const F as *const usize);
        self.hooked = Some(self.fn_addr());
        write(self.address(), replacement_addr);
    }

    pub unsafe fn unhook(&mut self) {
        if let Some(original_addr) = self.hooked {
            write(self.address(), original_addr);
            self.hooked = None;
        }
    }

    pub unsafe fn fn_addr(&self) -> usize {
        if let Some(original_addr) = self.hooked {
            original_addr
        } else {
            *(self.address() as *const usize)
        }
    }
}

macro_rules! impl_indirect_extern_fn_traits {
    ($conv:literal, $($T:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($T,)* R> FnOnce<($($T,)*)> for IndirectFn<extern $conv fn($($T,)*) -> R> {
            type Output = R;

            extern "rust-call" fn call_once(self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern $conv fn($($T,)*) -> R = unsafe { mem::transmute(self.fn_addr()) };
                (f)($($T,)*)
            }
        }

        #[allow(non_snake_case)]
        impl<$($T, )* R> FnMut<($($T,)*)> for IndirectFn<extern $conv fn($($T,)*) -> R> {
            extern "rust-call" fn call_mut(&mut self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern $conv fn($($T,)*) -> R = unsafe { mem::transmute(self.fn_addr()) };
                (f)($($T,)*)
            }
        }

        #[allow(non_snake_case)]
        impl<$($T, )* R> Fn<($($T,)*)> for IndirectFn<extern $conv fn($($T,)*) -> R> {
            extern "rust-call" fn call(&self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern $conv fn($($T,)*) -> R = unsafe { mem::transmute(self.fn_addr()) };
                (f)($($T,)*)
            }
        }
    };
}

macro_rules! impl_indirect_fn_traits {
    ($($T:ident),*) => {
        impl_indirect_extern_fn_traits!("C", $($T),*);
        impl_indirect_extern_fn_traits!("stdcall", $($T),*);
        impl_indirect_extern_fn_traits!("fastcall", $($T),*);
    }
}

impl_indirect_fn_traits!();
impl_indirect_fn_traits!(A);
impl_indirect_fn_traits!(A, B);
impl_indirect_fn_traits!(A, B, C);
impl_indirect_fn_traits!(A, B, C, D);
impl_indirect_fn_traits!(A, B, C, D, E);
impl_indirect_fn_traits!(A, B, C, D, E, F);
impl_indirect_fn_traits!(A, B, C, D, E, F, G);
impl_indirect_fn_traits!(A, B, C, D, E, F, G, H);
impl_indirect_fn_traits!(A, B, C, D, E, F, G, H, I);
impl_indirect_fn_traits!(A, B, C, D, E, F, G, H, I, J);

pub struct BoundFn<F> {
    pub name: &'static str,
    pub addr: Mutex<usize>,
    pub hook: Mutex<Option<RawDetour>>,
    fn_type: PhantomData<F>,
}

impl<F> BoundFn<F> {
    pub const fn new(name: &'static str) -> BoundFn<F> {
        BoundFn {
            name,
            addr: Mutex::new(0),
            hook: Mutex::new(None),
            fn_type: PhantomData,
        }
    }

    pub fn bind(&self, addr: usize) {
        *self.addr.lock().unwrap() = addr;
    }

    pub fn bind_from_imports(
        &self,
        name: &str,
        import_map: &HashMap<String, usize>,
    ) -> Result<(), String> {
        let import = import_map.get(name).ok_or_else(|| name.to_string())?;
        self.bind(*import);
        Ok(())
    }

    pub fn hook(&self, replacement: F) -> Result<(), HookError> {
        let addr = self.get_addr();
        if addr == 0 {
            return Err(HookError::Unbound(self.name.to_string()));
        }

        let mut hook_guard = self.hook.lock().unwrap();
        if hook_guard.is_some() {
            return Err(HookError::AlreadyHooked(self.name.to_string()));
        }

        let hook = unsafe {
            let replacement_addr = *(&replacement as *const F as *const usize);
            RawDetour::new(self.get_addr() as *const (), replacement_addr as *const ())
                .and_then(|hook| hook.enable().map(|_| hook))
                .map_err(|err| HookError::Retour(self.name.to_string(), err))?
        };

        *hook_guard = Some(hook);
        Ok(())
    }

    pub fn unhook(&self) -> Result<(), UnhookError> {
        if let Some(hook) = self.hook.lock().unwrap().take() {
            unsafe {
                hook.disable()
                    .map_err(|err| UnhookError::Retour(self.name.to_string(), err))
            }
        } else {
            Err(UnhookError::NotHooked(self.name.to_string()))
        }
    }

    pub fn get_addr(&self) -> usize {
        *self.addr.lock().unwrap()
    }

    pub fn original_fn_addr(&self) -> Option<usize> {
        if let Some(hook) = &self.hook.lock().unwrap().as_ref() {
            Some(hook.trampoline() as *const _ as usize)
        } else {
            let addr = self.get_addr();
            (addr != 0).then_some(addr)
        }
    }

    pub fn original_fn_addr_or_panic(&self) -> usize {
        if let Some(f) = self.original_fn_addr() {
            f
        } else {
            let error = format!("Tried to call unbound function '{}'", self.name);
            debug::error(&error);
            panic!("{}", &error)
        }
    }
}

pub enum HookError {
    Unbound(String),
    AlreadyHooked(String),
    Retour(String, retour::Error),
}

pub enum UnhookError {
    NotHooked(String),
    Retour(String, retour::Error),
}

macro_rules! impl_bound_extern_fn_traits {
    ($conv:literal, $($T:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($T,)* R> FnOnce<($($T,)*)> for BoundFn<extern $conv fn($($T,)*) -> R> {
            type Output = R;

            extern "rust-call" fn call_once(self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern $conv fn($($T,)*) -> R = unsafe { std::mem::transmute(self.original_fn_addr_or_panic()) };
                (f)($($T,)*)
            }
        }

        #[allow(non_snake_case)]
        impl<$($T, )* R> FnMut<($($T,)*)> for BoundFn<extern $conv fn($($T,)*) -> R> {
            extern "rust-call" fn call_mut(&mut self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern $conv fn($($T,)*) -> R = unsafe { std::mem::transmute(self.original_fn_addr_or_panic()) };
                (f)($($T,)*)
            }
        }

        #[allow(non_snake_case)]
        impl<$($T, )* R> Fn<($($T,)*)> for BoundFn<extern $conv fn($($T,)*) -> R> {
            extern "rust-call" fn call(&self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern $conv fn($($T,)*) -> R = unsafe { std::mem::transmute(self.original_fn_addr_or_panic()) };
                (f)($($T,)*)
            }
        }
    };
}

macro_rules! impl_bound_fn_traits {
    ($($T:ident),*) => {
        impl_bound_extern_fn_traits!("C", $($T),*);
        impl_bound_extern_fn_traits!("stdcall", $($T),*);
        impl_bound_extern_fn_traits!("fastcall", $($T),*);
    }
}

impl_bound_fn_traits!();
impl_bound_fn_traits!(A);
impl_bound_fn_traits!(A, B);
impl_bound_fn_traits!(A, B, C);
impl_bound_fn_traits!(A, B, C, D);
impl_bound_fn_traits!(A, B, C, D, E);
impl_bound_fn_traits!(A, B, C, D, E, F);
impl_bound_fn_traits!(A, B, C, D, E, F, G);
impl_bound_fn_traits!(A, B, C, D, E, F, G, H);
impl_bound_fn_traits!(A, B, C, D, E, F, G, H, I);
impl_bound_fn_traits!(A, B, C, D, E, F, G, H, I, J);

pub struct Value<T> {
    pub relative_address: usize,
    value_type: PhantomData<T>,
}

impl<T> Value<T> {
    pub const fn new(relative_address: usize) -> Value<T> {
        Value {
            relative_address,
            value_type: PhantomData,
        }
    }

    pub fn addr(&self) -> usize {
        relative_address(self.relative_address)
    }
}

impl<T> Value<T> {
    pub unsafe fn as_ref<'a>(&self) -> Option<&'a T> {
        as_ref::<T>(self.addr())
    }
}

impl<T: Clone> Value<T> {
    pub unsafe fn get(&self) -> T {
        self.as_ref().cloned().unwrap()
    }
}

impl<T> Value<*const T> {
    pub unsafe fn inner_addr(&self) -> usize {
        read::<usize>(self.addr())
    }

    pub unsafe fn inner_ref<'a>(&self) -> Option<&'a T> {
        as_ref::<T>(self.inner_addr())
    }
}
