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

            pub static mut $name: $crate::raw::memory::DirectFn<[<$name:camel>]> =
                $crate::raw::memory::DirectFn::new(stringify!($name), $addr);
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

            pub static mut $name: $crate::raw::memory::IndirectFn<[<$name:camel>]> =
                $crate::raw::memory::IndirectFn::new(stringify!($name), $addr);
        })*
    };
}

#[macro_export]
macro_rules! bound_fns {
    (
        $(
            extern $conv:literal fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        $(paste::paste! {
            pub type [<$name:camel>] =
                extern $conv fn($($arg: $arg_ty),*) $(-> $ret)?;

            pub static $name: $crate::raw::memory::BoundFn<[<$name:camel>]> =
                $crate::raw::memory::BoundFn::new(stringify!($name));
        })*
    };
}
