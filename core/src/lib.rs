//! Framework-agnostic engine for managing a Switch over MTP, Tinfoil USB, LAN
//! install and FTP, plus the sys-con virtual pad.
//!
//! Everything here works without a GUI so the same code backs the Tauri app
//! and the CLI.

pub mod aria2;
pub mod config;
pub mod debrid;
pub mod device;
pub mod doctor;
pub mod error;
pub mod fm;
pub mod ftp;
pub mod homebrew;
pub mod install;
pub mod library;
pub mod mcp;
pub mod mtp;
pub mod netinstall;
pub mod nsp;
pub mod sources;
pub mod store;
pub mod syscon;
pub mod tinfoil;
pub mod transfer;

pub use config::{Config, Host, InstallTarget};
pub use error::{Error, Result};
pub use library::Entry;
pub use transfer::{Job, JobManager, JobState};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Product name used for data directories and window titles.
pub const PRODUCT: &str = "GODsend Switch";
