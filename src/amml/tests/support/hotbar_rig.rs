use alaydriem_hotbar_manager::HotbarManagerMod;
use amm_client::test_support::FakeTable;
use amm_core::structs::event::{KeybindPhase, ModEvent};
use amml_api::amml_abi::{
    ClientTick, InventoryRequestDone, ModsLoaded, ProviderGone, ProviderReady, ScreenChanged, ScreenRender,
    TexturesReloaded, UiPointer,
};
use amml_api::testing::{FakeClient, FakeGame, FakeItem, FakeScreen, FakeSlots};
use amml_api::{Area, InputMode, Point, Rect, Screen, ScreenKind, Size, UiProfile};

// the keybind actions of the descriptor
pub const ROTATE_UP: u16 = 0;
pub const ROTATE_DOWN: u16 = 1;
pub const SORT: u16 = 2;

// the hotbar-manager mod enabled on a fake game that serves every event and area of its `USES`, with the mod manager
// provided and the client on the HUD
pub struct HotbarRig {
    pub game: FakeGame,
}

impl HotbarRig {
    pub fn new() -> Self {
        let game = Self::offered();
        FakeTable::provide(&game.ctx()).unwrap();
        game.load::<HotbarManagerMod>().unwrap();
        game.enable::<HotbarManagerMod>().unwrap();
        let rig = Self { game };
        rig.show(Screen::Hud);
        rig
    }

    // a fake game that offers everything the mod uses
    pub fn offered() -> FakeGame {
        let game = FakeTable::game();
        game.offer::<ClientTick>();
        game.offer::<ProviderReady>();
        game.offer::<ProviderGone>();
        game.offer::<ModsLoaded>();
        game.offer::<ScreenRender>();
        game.offer::<UiPointer>();
        game.offer::<ScreenChanged>();
        game.offer::<InventoryRequestDone>();
        game.offer::<TexturesReloaded>();
        for area in [
            Area::CLIENT_READ,
            Area::UI_SCREEN.at(2),
            Area::UI_DRAW,
            Area::UI_TEXTURES,
            Area::INVENTORY_READ.at(3),
            Area::INVENTORY_REQUEST,
            Area::CLIENT_TOP_SCREEN,
        ] {
            game.offer_area(area);
        }
        game
    }

    // the client has a local player and `screen` on top, and a tick reads it
    pub fn show(&self, screen: Screen) {
        self.game.set_client(FakeClient { local_player: true, top_screen: screen, ..FakeClient::default() });
        self.tick();
    }

    pub fn tick(&self) {
        self.game.raise_with_client(&mut ClientTick { header: FakeGame::header::<ClientTick>() });
    }

    // a keybind of the manager, pressed, and a tick that reads it
    pub fn press(&self, action: u16) {
        FakeTable::push_event(ModEvent::Keybind { action, phase: KeybindPhase::Pressed });
        self.tick();
    }

    pub fn render(&self, view: u64, screen: u64, fake: &FakeScreen) {
        self.game.raise_screen_render(amml_api::ScreenId(screen), amml_api::ViewId(view), fake);
    }

    // the stack game raises `ScreenChanged` from `old` to `new`
    pub fn changed(&self, old: u64, new: u64, screen: Screen) {
        self.game.raise_screen_changed(amml_api::ScreenId(old), amml_api::ScreenId(new), screen);
    }

    // the ready flag of the button textures of generation `generation`: the ids the painter got, in load order
    pub fn textures_ready(&self, ids: std::ops::RangeInclusive<u64>, generation: u32) {
        for id in ids {
            self.game.textures().set_ready(amml_api::TextureId { id, generation }, true);
        }
    }

    // true when the log holds `line` (level, message and fields as the fake writes them)
    pub fn logged(&self, line: &str) -> bool {
        self.game.logs().iter().any(|logged| logged == line)
    }

    // the number of log lines that start with `prefix`
    pub fn count_logged(&self, prefix: &str) -> usize {
        self.game.logs().iter().filter(|logged| logged.starts_with(prefix)).count()
    }
}

// a stack of `count` of item `name` with the identity `identity`, the way the loader gives it
pub fn item(name: &'static str, count: u8, identity: u64) -> Option<FakeItem> {
    Some(FakeItem {
        type_name: name,
        count,
        aux: Some(0),
        max_count: Some(64),
        identity: Some(identity),
        ..FakeItem::EMPTY
    })
}

// a classic screen of `kind` with its panel at (200, 100), 176 by 166 GUI units
pub fn screen(kind: ScreenKind, container: usize) -> FakeScreen {
    FakeScreen {
        kind,
        size: Some(Size { width: 640, height: 360 }),
        content_rect: Some(Rect { x: 200.0, y: 100.0, width: 176.0, height: 166.0 }),
        pointer: Some(Point { x: 0.0, y: 0.0 }),
        input_mode: Some(InputMode::Mouse),
        ui_profile: Some(UiProfile::Classic),
        gui_scale: Some(2.0),
        can_draw: true,
        slots: FakeSlots { container: vec![None; container], hotbar: vec![None; 9], inventory: vec![None; 27] },
        cursor: None,
    }
}

impl Default for HotbarRig {
    fn default() -> Self {
        Self::new()
    }
}
