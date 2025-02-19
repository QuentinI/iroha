//! Data primitives used inside Iroha, but not related directly to the
//! blockchain-specific data model.
//!
//! If you need a thin wrapper around a third-party library, so that
//! it can be used in `IntoSchema`, as well as [`parity_scale_codec`]'s
//! `Encode` and `Decode` trait implementations, you should add the
//! wrapper as a submodule to this crate, rather than into
//! `iroha_data_model` directly.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod addr;
#[cfg(not(feature = "ffi_import"))]
pub mod conststr;
pub mod fixed;
pub mod must_use;
pub mod riffle_iter;
pub mod small;

use fixed::prelude::*;

/// Trait for values that can be converted into `f64` and observed by prometheus.
pub trait IntoMetric {
    /// Convert `value` into `f64`.
    fn into_metric(self) -> f64;
}

// TODO: this should be a blanket impl.
impl IntoMetric for u128 {
    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn into_metric(self) -> f64 {
        self as f64
    }
}

impl IntoMetric for u32 {
    #[inline]
    fn into_metric(self) -> f64 {
        self.into()
    }
}

impl IntoMetric for Fixed {
    #[inline]
    fn into_metric(self) -> f64 {
        self.into()
    }
}

/// Trait for checked operations on iroha's primitive types.
pub trait CheckedOp {
    /// Checked addition. Computes `self + rhs`, returning None if overflow occurred.
    fn checked_add(self, rhs: Self) -> Option<Self>
    where
        Self: Sized;

    /// Checked subtraction. Computes `self - rhs`, returning None if overflow occurred.
    fn checked_sub(self, rhs: Self) -> Option<Self>
    where
        Self: Sized;
}

impl CheckedOp for u32 {
    #[inline]
    fn checked_add(self, rhs: Self) -> Option<Self> {
        self.checked_add(rhs)
    }

    #[inline]
    fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.checked_sub(rhs)
    }
}

impl CheckedOp for u128 {
    #[inline]
    fn checked_add(self, rhs: Self) -> Option<Self> {
        self.checked_add(rhs)
    }

    #[inline]
    fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.checked_sub(rhs)
    }
}

impl CheckedOp for Fixed {
    #[inline]
    fn checked_add(self, rhs: Self) -> Option<Self> {
        self.checked_add(rhs).ok()
    }

    #[inline]
    fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.checked_sub(rhs).ok()
    }
}

mod ffi {
    //! Definitions and implementations of FFI related functionalities

    macro_rules! ffi_item {
        ($it: item $($attr: meta)?) => {
            #[cfg(all(not(feature = "ffi_export"), not(feature = "ffi_import")))]
            $it

            #[cfg(all(feature = "ffi_export", not(feature = "ffi_import")))]
            #[derive(iroha_ffi::FfiType)]
            #[iroha_ffi::ffi_export]
            $(#[$attr])?
            $it

            #[cfg(feature = "ffi_import")]
            iroha_ffi::ffi! {
                #[iroha_ffi::ffi_import]
                $(#[$attr])?
                $it
            }
        };
    }

    pub(crate) use ffi_item;
}
