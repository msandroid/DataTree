#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![allow(clippy::mod_module_files)]
#![allow(clippy::unseparated_literal_suffix)]
#![allow(clippy::missing_inline_in_public_items)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::blanket_clippy_restriction_lints)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cargo_common_metadata)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::unused_self)]
#![allow(clippy::unused_async_trait_impl)]
#![allow(clippy::implicit_hasher)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::doc_markdown)]

pub mod cli;
pub mod delete;
pub mod error;
pub mod install;
pub mod mcp;
pub mod model;
pub mod query;
pub mod session;
pub mod volumes;

pub use error::ApiError;
pub use session::Session;
