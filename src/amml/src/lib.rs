//! @alaydriem/hotbar-manager as an AMML mod: draws sort, dump and drop buttons on chest and inventory screens, runs
//! them through the loader's container calls, and rotates the hotbar rows with one loader request.

#![deny(unsafe_code)]

#[forbid(unsafe_code)]
pub mod hotbar_manager_mod;
#[forbid(unsafe_code)]
pub mod services;
#[forbid(unsafe_code)]
pub mod traits;
#[forbid(unsafe_code)]
pub mod types;

pub use hotbar_manager_mod::HotbarManagerMod;

amml_api::amml_mod!(HotbarManagerMod);
