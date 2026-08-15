#![cfg_attr(not(test), no_std)]

//! Compatibility facade for the board application.
//!
//! The implementation lives in `mejiro-core`; keeping this re-export lets
//! existing callers continue to use `rmk_mejiro::mejiro` during the
//! incremental extraction.

pub use mejiro_core::mejiro;
