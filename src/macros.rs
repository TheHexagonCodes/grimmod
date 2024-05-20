#[macro_export]
macro_rules! bound_fns {
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
                bound_fns!(@call_method $(#[pattern($pattern, $offset)])? $name);
        })*
    };

    (@call_method #[pattern($pattern:literal, $offset:literal)] $name:ident) => {
        $crate::raw::memory::BoundFn::new(stringify!($name), Some(($pattern, $offset)))
    };

    (@call_method $name:ident) => {
        $crate::raw::memory::BoundFn::new(stringify!($name), None)
    };
}
