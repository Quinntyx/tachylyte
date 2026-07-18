//! Native light GPUI settings surfaces for account, synchronization, and publishing.
//!
//! The surfaces are deliberately transport-free: they render service models and
//! record typed intents for the host to execute. They never receive credentials.

pub mod account;
pub mod model;
pub mod publish;
pub mod sync;

pub use account::AccountSurface;
pub use model::{
    AccountIntent, AccountModel, AccountStatus, ActivityEntry, Device, PublishIntent, PublishModel,
    SyncIntent, SyncModel, SyncState,
};
pub use publish::PublishSurface;
pub use sync::SyncSurface;
