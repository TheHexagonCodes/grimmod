#[macro_export]
macro_rules! fns {
    (
        $(
            #[address($addr:expr)]
            extern $conv:literal fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        $(paste::paste! {
            pub type [<$name:camel>] =
                extern $conv fn($($arg: $arg_ty),*) $(-> $ret)?;

            pub static mut $name: $crate::process::DirectFn<[<$name:camel>]> =
                $crate::process::DirectFn::new(stringify!($name), $addr);
        })*
    };
}

#[macro_export]
macro_rules! fn_refs {
    (
        $(
            #[address($addr:expr)]
            extern $conv:literal fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        $(paste::paste! {
            pub type [<$name:camel>] =
                extern $conv fn($($arg: $arg_ty),*) $(-> $ret)?;

            pub static mut $name: $crate::process::IndirectFn<[<$name:camel>]> =
                $crate::process::IndirectFn::new(stringify!($name), $addr);
        })*
    };
}
