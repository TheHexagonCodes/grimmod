use lightningscanner::Scanner;
use once_cell::sync::Lazy;
use retour::RawDetour;
use std::marker::PhantomData;
use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS,
};
use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
use windows::Win32::System::Threading::GetCurrentProcess;

use crate::{debug, raw::process};

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

enum FnHook {
    Direct(Mutex<Option<RawDetour>>),
    Indirect(Mutex<Option<usize>>),
}

impl FnHook {
    fn is_hooked(&self) -> bool {
        match self {
            FnHook::Direct(mutex) => mutex.lock().unwrap().is_some(),
            FnHook::Indirect(mutex) => mutex.lock().unwrap().is_some(),
        }
    }
}

pub struct BoundFn<F> {
    pub name: &'static str,
    pub addr: Mutex<usize>,
    hook: FnHook,
    pub pattern: Option<(&'static str, usize)>,
    fn_type: PhantomData<F>,
}

impl<F> BoundFn<F> {
    pub const fn direct(name: &'static str, pattern: Option<(&'static str, usize)>) -> BoundFn<F> {
        BoundFn {
            name,
            addr: Mutex::new(0),
            hook: FnHook::Direct(Mutex::new(None)),
            pattern,
            fn_type: PhantomData,
        }
    }

    pub const fn indirect(name: &'static str) -> BoundFn<F> {
        BoundFn {
            name,
            addr: Mutex::new(0),
            hook: FnHook::Indirect(Mutex::new(None)),
            pattern: None,
            fn_type: PhantomData,
        }
    }

    pub fn get_addr(&self) -> usize {
        *self.addr.lock().unwrap()
    }

    pub fn bind(&self, addr: usize) -> Result<(), BindError> {
        let mut addr_guard = self.addr.lock().unwrap();
        if *addr_guard == 0 {
            *addr_guard = addr;
            Ok(())
        } else {
            Err(BindError::AlreadyBound(self.name.to_string()))
        }
    }

    pub fn bind_symbol(&self, name: &str) -> Result<(), BindError> {
        let addr = process::get_symbol_addr(name).ok_or_else(|| self.not_found())?;
        self.bind(addr)
    }

    pub fn bind_virtual_import(&self, proc_name: &str, module_name: &str) -> Result<(), BindError> {
        let addr = process::get_import_virtual_addr(proc_name, module_name)
            .ok_or_else(|| self.not_found())?;
        self.bind(addr)
    }

    pub fn find(&self, code_addr: usize, code_size: usize) -> Result<(), BindError> {
        let Some((pattern, offset)) = self.pattern else {
            return Err(self.not_found());
        };

        let scanner = Scanner::new(pattern);
        let result = unsafe { scanner.find(None, code_addr as _, code_size) };
        if result.is_valid() {
            if debug::verbose() {
                debug::info(format!(
                    "Found address for {}: 0x{:x}",
                    self.name,
                    result.get_addr() as usize - offset
                ));
            }
            self.bind(result.get_addr() as usize - offset)
        } else {
            Err(self.not_found())
        }
    }

    pub fn hook(&self, replacement: F) -> Result<(), HookError> {
        let addr = self.get_addr();
        if addr == 0 {
            return Err(HookError::Unbound(self.name.to_string()));
        }

        if self.hook.is_hooked() {
            return Err(self.already_hooked());
        }

        let replacement_addr = unsafe { *(&replacement as *const F as *const usize) };

        match &self.hook {
            FnHook::Direct(mutex) => {
                let raw_detour = unsafe {
                    RawDetour::new(addr as *const (), replacement_addr as *const ())
                        .and_then(|hook| hook.enable().map(|_| hook))
                        .map_err(|err| HookError::Retour(self.name.to_string(), err))?
                };
                *mutex.lock().unwrap() = Some(raw_detour);
            }
            FnHook::Indirect(mutex) => {
                let underlying_addr = unsafe { *(addr as *const usize) };
                unsafe { write(addr, replacement_addr) };
                *mutex.lock().unwrap() = Some(underlying_addr);
            }
        }

        Ok(())
    }

    pub fn unhook(&self) -> Result<(), UnhookError> {
        match &self.hook {
            FnHook::Direct(mutex) => {
                let raw_detour = mutex
                    .lock()
                    .unwrap()
                    .take()
                    .ok_or_else(|| self.not_hooked())?;
                unsafe {
                    raw_detour
                        .disable()
                        .map_err(|err| UnhookError::Retour(self.name.to_string(), err))
                }
            }
            FnHook::Indirect(mutex) => {
                let underlying_addr = mutex
                    .lock()
                    .unwrap()
                    .take()
                    .ok_or_else(|| self.not_hooked())?;
                unsafe { write(self.get_addr(), underlying_addr) };
                Ok(())
            }
        }
    }

    pub fn original_fn_addr(&self) -> Option<usize> {
        let addr = self.get_addr();
        let unhooked = || (addr != 0).then_some(addr);
        match &self.hook {
            FnHook::Direct(mutex) => mutex
                .lock()
                .unwrap()
                .as_ref()
                .map(|raw_detour| raw_detour.trampoline() as *const _ as usize)
                .or_else(unhooked),
            FnHook::Indirect(mutex) => (*mutex.lock().unwrap())
                .or_else(|| unhooked().map(|addr| unsafe { *(addr as *const usize) })),
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

    pub fn not_found(&self) -> BindError {
        BindError::NotFound(self.name.to_string())
    }

    pub fn already_hooked(&self) -> HookError {
        HookError::AlreadyHooked(self.name.to_string())
    }

    pub fn not_hooked(&self) -> UnhookError {
        UnhookError::NotHooked(self.name.to_string())
    }
}

pub enum BindError {
    AlreadyBound(String),
    NotFound(String),
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BindError::AlreadyBound(func) => {
                write!(f, "Tried to find '{}' but it has already been found", func)
            }
            BindError::NotFound(func) => {
                write!(f, "Could not find '{}'", func)
            }
        }
    }
}

pub enum HookError {
    Unbound(String),
    AlreadyHooked(String),
    Retour(String, retour::Error),
}

impl std::fmt::Display for HookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookError::Unbound(func) => write!(f, "Tried to hook '{}' before it was found", func),
            HookError::AlreadyHooked(func) => {
                write!(f, "Tried to hook '{}' while already hooked", func)
            }
            HookError::Retour(func, retour_err) => {
                write!(
                    f,
                    "Low-level error while hooking '{}': {:?}",
                    func, retour_err
                )
            }
        }
    }
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

pub struct Value<T, F: 'static> {
    name: &'static str,
    relative_to: &'static BoundFn<F>,
    offset: usize,
    addr: Mutex<Option<usize>>,
    value_type: PhantomData<T>,
}

impl<T, F> Value<T, F> {
    pub const fn new(
        name: &'static str,
        relative_to: &'static BoundFn<F>,
        offset: usize,
    ) -> Value<T, F> {
        Value {
            name,
            relative_to,
            offset,
            addr: Mutex::new(None),
            value_type: PhantomData,
        }
    }

    pub fn addr(&self) -> usize {
        let mut addr_guard = self.addr.lock().unwrap();
        match *addr_guard {
            Some(addr) => addr,
            None => {
                let ref_addr = self.relative_to.get_addr() + self.offset;
                let addr = unsafe { std::ptr::read(ref_addr as *const usize) };
                if debug::verbose() {
                    debug::info(format!(
                        "Found address for static {}: 0x{:x}",
                        self.name, addr
                    ));
                }
                *addr_guard = Some(addr);
                addr
            }
        }
    }
}

impl<T, F> Value<T, F> {
    pub unsafe fn as_ref<'a>(&self) -> Option<&'a T> {
        as_ref::<T>(self.addr())
    }
}

impl<T: Clone, F> Value<T, F> {
    pub unsafe fn get(&self) -> T {
        self.as_ref().cloned().unwrap()
    }
}

impl<T, F> Value<*const T, F> {
    pub unsafe fn inner_addr(&self) -> usize {
        read::<usize>(self.addr())
    }

    pub unsafe fn inner_ref<'a>(&self) -> Option<&'a T> {
        as_ref::<T>(self.inner_addr())
    }
}
