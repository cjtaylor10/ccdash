//! JSON-RPC 2.0 server over Unix socket.

pub mod codec;
pub mod dispatch;
pub mod handlers;

mod server;
pub use server::serve;
