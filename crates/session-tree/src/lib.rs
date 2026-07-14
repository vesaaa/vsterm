//! Session tree data model and YAML persistence.

mod config;
mod error;
mod store;
mod tree;

pub use config::{AuthConfig, AuthType, BackendKind, SessionConfig};
pub use error::SessionTreeError;
pub use store::{AppPaths, SessionStore};
pub use tree::{SessionTree, TreeNode};
