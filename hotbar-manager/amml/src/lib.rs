//! @alaydriem/hotbar-manager as an AMML mod: hooks the client update, registers with the mod manager and, in later
//! tasks, rotates the hotbar rows and sorts containers.

#![deny(unsafe_code)]

pub mod detours;
pub mod errors;
pub mod game;
pub mod hotbar_manager_mod;
pub mod services;
pub mod traits;
pub mod types;

pub use hotbar_manager_mod::HotbarManagerMod;

amml_sdk::amml_mod!(HotbarManagerMod);
