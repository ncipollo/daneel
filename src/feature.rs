//! Domain layer. All agent logic lives here; the cli layer calls through to it.

pub mod chat;
pub mod config;
pub mod events;
pub mod greeting;
pub mod interaction;
pub mod oneshot;
pub mod provider;
pub mod providers;
#[cfg(test)]
pub mod script_provider;
pub mod session;
pub mod strategy;
pub mod tactic;
pub mod task;
pub mod tools;
