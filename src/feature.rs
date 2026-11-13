//! Domain layer. All agent logic lives here; the cli layer calls through to it.

pub mod chat;
pub mod config;
pub mod events;
pub mod greeting;
pub mod oneshot;
pub mod provider;
pub mod providers;
pub mod session;
pub mod task;
pub mod tools;
