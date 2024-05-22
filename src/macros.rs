#[macro_export]
macro_rules! direct_fns {
    (
        $(
            $(#[pattern($pattern:literal, $offset:literal)])?
            extern $conv:literal fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        $(paste::paste! {
            pub type [<$name:camel>] =
                extern $conv fn($($arg: $arg_ty),*) $(-> $ret)?;

            pub static $name: $crate::raw::memory::BoundFn<[<$name:camel>]> =
                direct_fns!(@call_method $(#[pattern($pattern, $offset)])? $name);
        })*
    };

    (@call_method #[pattern($pattern:literal, $offset:literal)] $name:ident) => {
        $crate::raw::memory::BoundFn::direct(stringify!($name), Some(($pattern, $offset)))
    };

    (@call_method $name:ident) => {
        $crate::raw::memory::BoundFn::direct(stringify!($name), None)
    };
}

#[macro_export]
macro_rules! indirect_fns {
    (
        $(
            extern $conv:literal fn $name:ident($($arg:ident : $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        $(paste::paste! {
            pub type [<$name:camel>] =
                extern $conv fn($($arg: $arg_ty),*) $(-> $ret)?;

            pub static $name: $crate::raw::memory::BoundFn<[<$name:camel>]> =
                $crate::raw::memory::BoundFn::indirect(stringify!($name));
        })*
    };
}
