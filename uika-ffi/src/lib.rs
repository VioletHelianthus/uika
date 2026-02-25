// uika-ffi: #[repr(C)] types, handle types, API table definition.
// Zero external dependencies. This crate defines the complete Rust ↔ C++ contract.

pub mod handles;
pub mod error;
pub mod api_table;
pub mod callbacks;
pub mod reify_types;
pub mod contract_tests;

pub use handles::*;
pub use error::*;
pub use api_table::*;
pub use callbacks::*;
pub use reify_types::*;
pub use uika_ue_flags::*;
