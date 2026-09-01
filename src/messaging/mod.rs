#[allow(unused_imports)]
pub use interface::{MessagingError, MessagingEvent};
pub use implementation::MessagingService;

mod implementation;
mod interface;
mod service;
