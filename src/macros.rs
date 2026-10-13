//! Internal macros for the request types.

/// Generates a chainable setter for each optional field of a request type.
/// Each setter takes the value (not the `Option`) and stores `Some(value)`.
macro_rules! setters {
    ($ty:ident { $($field:ident : $fty:ty),+ $(,)? }) => {
        impl $ty {
            $(
                #[doc = concat!("Sets the `", stringify!($field), "` field.")]
                #[must_use]
                pub fn $field(mut self, value: impl Into<$fty>) -> Self {
                    self.$field = Some(value.into());
                    self
                }
            )+
        }
    };
}

/// Declares a fieldless enum that serializes as its integer value, as the API expects.
macro_rules! int_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $value:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant = $value ),+
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                serializer.serialize_u8(*self as u8)
            }
        }
    };
}
