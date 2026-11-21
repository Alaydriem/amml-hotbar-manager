use amml_api::ScreenKind as ApiKind;
use hotbar_manager_core::structs::ScreenKind;

// the screen kinds of the loader as the core crate names them
pub struct KindMap;

impl KindMap {
    // a kind the core crate does not know is `Other`
    pub fn kind(kind: ApiKind) -> ScreenKind {
        match kind {
            ApiKind::SmallChest => ScreenKind::SmallChest,
            ApiKind::LargeChest => ScreenKind::LargeChest,
            ApiKind::Barrel => ScreenKind::Barrel,
            ApiKind::ShulkerBox => ScreenKind::ShulkerBox,
            ApiKind::EnderChest => ScreenKind::EnderChest,
            ApiKind::Hopper => ScreenKind::Hopper,
            ApiKind::Dispenser => ScreenKind::Dispenser,
            ApiKind::Dropper => ScreenKind::Dropper,
            ApiKind::PlayerInventory => ScreenKind::PlayerInventory,
            _ => ScreenKind::Other,
        }
    }
}
