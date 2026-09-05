// Each phase of the build adds a sibling module here, e.g.:
//   pub mod pty;      // integrated terminal sessions
//   pub mod lsp;      // language server process management + JSON-RPC bridge
//   pub mod plugins;  // native extension host
//
// Keeping one file per concern (rather than one giant lib.rs) is what makes
// it manageable to register dozens of commands later without the file
// becoming unreadable.

pub mod app;
pub mod fs;
