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
///
/// The enum is `#[non_exhaustive]`: the API may add values. `from_value` maps an integer
/// received from the API back to a variant.
macro_rules! int_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $value:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$vmeta])* $variant = $value ),+
        }

        #[allow(deprecated)]
        impl $name {
            /// The integer value the API uses for this variant.
            pub fn value(self) -> u8 {
                self as u8
            }

            /// The variant for an integer value received from the API, or `None` for a value
            /// this version of the SDK does not know.
            pub fn from_value(value: i64) -> Option<Self> {
                match value {
                    $( $value => Some(Self::$variant), )+
                    _ => None,
                }
            }
        }

        #[allow(deprecated)]
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

/// Declares a fieldless enum that serializes as a string, as the API expects.
/// The enum is `#[non_exhaustive]`: the API may add values.
macro_rules! str_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $value:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$vmeta])* $variant ),+
        }

        impl $name {
            /// The string the API uses for this variant.
            pub fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $value, )+
                }
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}
