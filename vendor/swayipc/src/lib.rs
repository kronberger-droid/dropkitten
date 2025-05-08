mod command;
mod event;
#[cfg(feature = "async")]
mod not_sync;
pub mod reply;
mod socket;
#[cfg(not(feature = "async"))]
mod sync;
mod utils;

pub use event::EventType;
pub use failure::{bail, ensure, Error, Fallible};

#[cfg(feature = "async")]
pub use async_std;
#[cfg(feature = "async")]
#[cfg(not(feature = "event_stream"))]
pub use not_sync::{Connection, EventIterator};
#[cfg(feature = "async")]
#[cfg(feature = "event_stream")]
pub use not_sync::{Connection, EventStream};
#[cfg(feature = "unstable")]
pub use swayipc_command_builder::{Command, Filter};
#[cfg(not(feature = "async"))]
pub use sync::{Connection, EventIterator};

const MAGIC: [u8; 6] = [105, 51, 45, 105, 112, 99];
