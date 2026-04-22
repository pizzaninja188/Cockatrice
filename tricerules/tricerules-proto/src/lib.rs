#![allow(clippy::derive_partial_eq_without_eq)]
#![allow(clippy::large_enum_variant)]

/// Prost emits flat items for `package ruled.v1`; nest them to match the proto package path.
pub mod ruled {
    pub mod v1 {
        include!(concat!(env!("OUT_DIR"), "/ruled.v1.rs"));
    }
}

pub use ruled::v1::*;
