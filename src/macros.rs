// === Windows macros ===
// Uses ident-based symbols, supports multiple calling conventions,
// byte-pattern scanning, IAT binding, and DLL proxy exports.

#[cfg(target_os = "windows")]
#[macro_export]
macro_rules! direct_fn_def {
    (
        $(#[pattern($pattern:literal, $offset:literal)])?
        $(#[symbol($symbol:ident, $dll:literal)])?
        extern $conv:literal fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
    ) => {
        paste::paste! {
            pub type [<$name:camel>] = extern $conv fn($($arg: $arg_ty),*) $(-> $ret)?;
            pub static $name: $crate::raw::memory::BoundFn<[<$name:camel>]> =
                $crate::direct_fn_def!(@new $name $($pattern $offset)? $($symbol $dll)?);
        }
    };

    (@new $name:ident $pattern:literal $offset:literal) => {
        $crate::raw::memory::BoundFn::direct(stringify!($name), Some(($pattern, $offset)))
    };

    (@new $name:ident $symbol:ident $dll:literal) => {
        $crate::raw::memory::BoundFn::direct(stringify!($symbol), None)
    };

    (@new $name:ident) => {
        $crate::raw::memory::BoundFn::direct(stringify!($name), None)
    };
}

#[cfg(target_os = "windows")]
#[macro_export]
macro_rules! direct_fns {
    (
        $(#![bind_with($binder_name:ident)])?
        $(
            $(#[pattern($pattern:literal, $offset:literal)])?
            $(#[symbol($symbol:ident, $dll:literal)])?
            extern $conv:literal fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        $($crate::direct_fn_def! {
            $(#[pattern($pattern, $offset)])?
            $(#[symbol($symbol, $dll)])?
            extern $conv fn $name($($arg : $arg_ty),*) $(-> $ret)?;
        })*

        $crate::direct_fns!(@binder $($binder_name)? {
            $($name $(pattern($pattern, $offset))? $(symbol($symbol, $dll))?),*
        });
    };

    (@binder $binder_name:ident { $($name:ident pattern($pattern:literal, $offset:literal)),* $(,)? }) => {
        pub fn $binder_name(code_area: usize, code_size: usize) -> Result<(), $crate::raw::memory::BindError> {
            $($name.find(code_area, code_size)?;)*
            Ok(())
        }
    };
    (@binder $binder_name:ident { $($name:ident symbol($symbol:ident, $dll:literal)),* $(,)? }) => {
        pub fn $binder_name() -> Result<(), $crate::raw::memory::BindError> {
            $($name.bind_virtual_import(stringify!($symbol), $dll)?;)*
            Ok(())
        }
    };
    (@binder $binder_name:ident { $($name:ident),* $(,)? }) => { compile_err };
    (@binder { $($name:ident),* }) => {};
}

#[cfg(target_os = "windows")]
#[macro_export]
macro_rules! indirect_fn_defs {
    (
        $(
            $(#[symbol($symbol_name:ident)])?
            extern $conv:literal fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        $(paste::paste! {
            pub type [<$name:camel>] = extern $conv fn($($arg: $arg_ty),*) $(-> $ret)?;

            pub static $name: $crate::raw::memory::BoundFn<[<$name:camel>]> =
                $crate::raw::memory::BoundFn::indirect(stringify!($name));
        })*
    }
}

#[cfg(target_os = "windows")]
#[macro_export]
macro_rules! indirect_fns {
    (
        #![bind_with($binder_name:ident)]
        $(
            $(#[symbol($symbol_name:ident)])?
            extern $conv:literal fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        $crate::indirect_fn_defs! { $(
            $(#[symbol($symbol_name)])?
            extern $conv fn $name($($arg : $arg_ty),*) $(-> $ret)?;
        )* }

        pub fn $binder_name() -> Result<(), $crate::raw::memory::BindError> {
            $($( $name.bind_symbol(stringify!($symbol_name))?; )?)*

            Ok(())
        }
    };

    ($(
        $(#[symbol($symbol_name:ident)])?
        extern $conv:literal fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
    )*) => {
        $crate::indirect_fn_defs! { $(
            $(#[symbol($symbol_name)])?
            extern $conv fn $name($($arg : $arg_ty),*) $(-> $ret)?;
        )* }
    }
}

#[cfg(target_os = "windows")]
#[macro_export]
macro_rules! proxy {
    (
        $(
            #[with($internal:path)]
            extern $conv:literal fn $name:ident($($arg_name:ident : $arg_ty:ty),*) $(-> $ret_ty:ty)?;
        )*
    ) => {
        $(
            #[no_mangle]
            pub unsafe extern "system" fn $name($($arg_name: $arg_ty),*) $(-> $ret_ty)? {
                $internal($($arg_name),*)
            }
        )*
    };
}

// === Linux macros ===
// Uses string-literal symbols, extern "C" only,
// symbol-name resolution via ELF symtab, GOT entry binding.

#[cfg(target_os = "linux")]
#[macro_export]
macro_rules! direct_fn_def {
    (
        $(#[symbol($symbol:literal)])?
        extern "C" fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
    ) => {
        paste::paste! {
            pub type [<$name:camel>] = extern "C" fn($($arg: $arg_ty),*) $(-> $ret)?;
            pub static $name: $crate::raw::memory::BoundFn<[<$name:camel>]> =
                $crate::raw::memory::BoundFn::direct(
                    stringify!($name),
                    $crate::direct_fn_def!(@symbol $($symbol)?),
                );
        }
    };

    (@symbol $symbol:literal) => { Some($symbol) };
    (@symbol) => { None };
}

#[cfg(target_os = "linux")]
#[macro_export]
macro_rules! direct_fns {
    // With bind_with
    (
        #![bind_with($binder_name:ident)]
        $(
            $(#[symbol($symbol:literal)])?
            extern "C" fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        $($crate::direct_fn_def! {
            $(#[symbol($symbol)])?
            extern "C" fn $name($($arg : $arg_ty),*) $(-> $ret)?;
        })*

        $crate::direct_fns!(@binder $binder_name { $($name $(symbol($symbol))?),* });
    };

    // Without bind_with
    (
        $(
            $(#[symbol($symbol:literal)])?
            extern "C" fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        $($crate::direct_fn_def! {
            $(#[symbol($symbol)])?
            extern "C" fn $name($($arg : $arg_ty),*) $(-> $ret)?;
        })*
    };

    (@binder $binder_name:ident { $($name:ident $(symbol($symbol:literal))?),* $(,)? }) => {
        pub fn $binder_name() -> Result<(), $crate::raw::memory::BindError> {
            $(
                $crate::direct_fns!(@bind_one $name $($symbol)?);
            )*
            Ok(())
        }
    };

    (@bind_one $name:ident $symbol:literal) => {
        $name.bind_symbol($symbol)?;
    };

    (@bind_one $name:ident) => {
        $name.bind_symbol(stringify!($name))?;
    };
}

#[cfg(target_os = "linux")]
#[macro_export]
macro_rules! indirect_fn_defs {
    (
        $(
            $(#[symbol($symbol_name:literal)])?
            extern "C" fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        $(paste::paste! {
            pub type [<$name:camel>] = extern "C" fn($($arg: $arg_ty),*) $(-> $ret)?;

            pub static $name: $crate::raw::memory::BoundFn<[<$name:camel>]> =
                $crate::raw::memory::BoundFn::indirect(stringify!($name));
        })*
    }
}

#[cfg(target_os = "linux")]
#[macro_export]
macro_rules! indirect_fns {
    (
        #![bind_with($binder_name:ident)]
        $(
            #[symbol($symbol_name:literal)]
            extern "C" fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        $crate::indirect_fn_defs! { $(
            #[symbol($symbol_name)]
            extern "C" fn $name($($arg : $arg_ty),*) $(-> $ret)?;
        )* }

        pub fn $binder_name() -> Result<(), $crate::raw::memory::BindError> {
            $($name.bind_got_entry($symbol_name)?;)*
            Ok(())
        }
    };

    // Without binder
    ($(
        $(#[symbol($symbol_name:literal)])?
        extern "C" fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
    )*) => {
        $crate::indirect_fn_defs! { $(
            $(#[symbol($symbol_name)])?
            extern "C" fn $name($($arg : $arg_ty),*) $(-> $ret)?;
        )* }
    }
}
