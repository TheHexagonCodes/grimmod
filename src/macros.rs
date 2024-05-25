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
