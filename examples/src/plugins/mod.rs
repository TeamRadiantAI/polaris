//! Plugins for the example agent.
//!
//! - [`TerminalIOPlugin`] — Terminal I/O provider for CLI interaction
//! - [`FileToolsPlugin`] — Sandboxed file operation tools
//! - [`SessionPlugin`] — Session persistence via JSON file storage

mod file_tools;
mod session;
mod terminal_io;

pub use file_tools::{FileToolsConfig, FileToolsPlugin};
pub use session::SessionPlugin;
pub use terminal_io::TerminalIOPlugin;
