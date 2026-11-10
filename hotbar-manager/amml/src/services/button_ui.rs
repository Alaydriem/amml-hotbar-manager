use std::ffi::c_void;

use amml_game::RawObject;
use hotbar_manager_core::structs::{ButtonAction, ButtonSpec, PanelRect};

use crate::services::{ButtonPainter, ClickRouter, PanelLocator, ScreenDetector};
use crate::types::{ButtonCalls, ClickRoute, InputMode, ScreenCalls};

// The mod buttons on the open classic screen: the `ScreenView::render` detour draws them after the game, and the
// `ScreenView::handleButtonEvent` detour asks which pointer events they take.
//
// Each container screen is also rendered by a second view at a taller size (docs/findings/spike-align/report.md
// section 6). So the drawn layout is kept per view, each view's clicks are tested against its own layout, and only
// a button event from another view that drew buttons (or the held view showing no buttons) clears the held flags: a
// render of the second view never does, and neither does an event of a view that drew none
pub struct ButtonUi {
    bound: Option<BoundButtons>,
    router: ClickRouter,
    views: Vec<ViewLayout>,
}

// the game side of the buttons, present only when every entry resolved and both hooks went in
pub struct BoundButtons {
    calls: ButtonCalls,
    detector: ScreenDetector,
    locator: PanelLocator,
    painter: ButtonPainter,
}

// the buttons one view drew on its last frame
#[derive(Debug, Clone, PartialEq)]
pub struct ViewLayout {
    pub view: usize,
    pub panel: PanelRect,
    pub specs: Vec<ButtonSpec>,
}

impl BoundButtons {
    pub fn new(screens: ScreenCalls, calls: ButtonCalls, textures_dir: Option<String>) -> Self {
        Self {
            calls,
            detector: ScreenDetector::new(screens),
            locator: PanelLocator::new(screens, calls),
            painter: ButtonPainter::new(calls, textures_dir),
        }
    }
}

impl Default for ButtonUi {
    fn default() -> Self {
        Self::new()
    }
}

impl ButtonUi {
    // the views kept: the real view and the second render, with room for a screen that opens over another
    pub const MAX_VIEWS: usize = 4;

    pub fn new() -> Self {
        Self { bound: None, router: ClickRouter::new(), views: Vec::with_capacity(Self::MAX_VIEWS) }
    }

    // enable binds, disable unbinds (None); either way no old layout or held flag is kept
    pub fn bind(&mut self, bound: Option<BoundButtons>) {
        self.bound = bound;
        self.views.clear();
        self.router.clear();
    }

    pub fn bound(&self) -> bool {
        self.bound.is_some()
    }

    pub fn layout(&self, view: usize) -> Option<&ViewLayout> {
        self.views.iter().find(|layout| layout.view == view)
    }

    // the view drew `layout`; it becomes the newest kept view
    pub fn remember(&mut self, layout: ViewLayout) {
        self.views.retain(|kept| kept.view != layout.view);
        self.views.insert(0, layout);
        self.views.truncate(Self::MAX_VIEWS);
    }

    // the view drew no buttons this frame
    pub fn forget(&mut self, view: usize) {
        self.views.retain(|kept| kept.view != view);
        self.router.forget_view(view);
    }

    // one pointer event of `view`: the route the router gives against the view's last drawn layout. A view with no
    // layout takes only the release of a press it swallowed. The game hands each pointer event to every view of the
    // screen stack (on 1.26.63.01 two views that draw nothing get every event after the screen's view), so an event
    // of a view that drew no buttons and holds no press goes to the game and leaves the held flags alone
    pub fn route(
        &mut self,
        view: usize,
        pointer: usize,
        pressed: bool,
        pointer_units: (f32, f32),
        mode: Option<InputMode>,
    ) -> ClickRoute {
        let layout = self.layout(view);
        if layout.is_none() && !self.router.holds(view) {
            return ClickRoute::Pass;
        }
        let over = layout.and_then(|layout| ClickRouter::hit(&layout.specs, layout.panel, pointer_units));
        self.router.route(view, pointer, pressed, over, mode)
    }

    /// the body of the `ScreenView::render` detour, after the original: draws the buttons of the view's screen and
    /// keeps the drawn layout for the view's clicks. `client` is the ClientInstance of the last tick
    ///
    /// # Safety
    /// `view` and `ctx` must be the live arguments of the current `ScreenView::render` call, and `client` null or
    /// the live ClientInstance, on the game thread
    #[allow(unsafe_code)]
    pub unsafe fn on_render(&mut self, view: usize, ctx: usize, client: usize) {
        if view == 0 {
            return;
        }
        // SAFETY: forwarded to the caller
        let layout = unsafe { self.draw(view as *mut c_void, ctx as *mut c_void, client as *mut c_void) };
        match layout {
            Some(layout) if !layout.specs.is_empty() => self.remember(layout),
            _ => self.forget(view),
        }
    }

    #[allow(unsafe_code)]
    unsafe fn draw(&mut self, view: *mut c_void, ctx: *mut c_void, client: *mut c_void) -> Option<ViewLayout> {
        let bound = self.bound.as_mut()?;
        // SAFETY: forwarded to the caller
        let (kind, profile) = unsafe { bound.detector.detect(view, client) };
        let specs = ClickRouter::buttons(kind, profile);
        if specs.is_empty() {
            return None;
        }
        // SAFETY: forwarded to the caller
        let panel = unsafe { bound.locator.panel(view, kind) }?;
        // SAFETY: forwarded to the caller
        let scale = unsafe { bound.painter.gui_scale(ctx) }?;
        // a frame the game left images queued in: no buttons drawn, and the view keeps its last layout, so a held
        // press is not cleared
        // SAFETY: forwarded to the caller
        if unsafe { bound.painter.queue_busy(ctx) } {
            return self.views.iter().find(|layout| layout.view == view as usize).cloned();
        }
        // SAFETY: a live view holds its pointer (vec2, GUI units) at the entry's offset
        let pointer = unsafe { RawObject::read::<[f32; 2]>(view, bound.calls.pointer_location) }?;
        let hovered = ClickRouter::hit(&specs, panel, (pointer[0], pointer[1]));
        let pressed = self.router.pressed(view as usize, hovered);
        // SAFETY: forwarded to the caller
        let drawn = unsafe { bound.painter.paint(ctx, kind, panel, scale, hovered, pressed) };
        Some(ViewLayout { view: view as usize, panel, specs: drawn })
    }

    /// the body of the `ScreenView::handleButtonEvent` detour: the route of one button event. Ids that are not
    /// pointer ids go to the game
    ///
    /// # Safety
    /// `view` must be the live `ScreenView` of the current call, on the game thread
    #[allow(unsafe_code)]
    pub unsafe fn on_button_event(&mut self, view: usize, id: u32, pressed: bool) -> ClickRoute {
        let Some(bound) = self.bound.as_ref() else { return ClickRoute::Pass };
        let calls = bound.calls;
        let pointer = view as *const c_void;
        if pointer.is_null() {
            return ClickRoute::Pass;
        }
        // SAFETY: a live view holds its pointer button ids (`std::array<uint, 2>`) at the entry's offset
        let Some(ids) = (unsafe { RawObject::read::<[u32; ClickRouter::POINTERS]>(pointer, calls.pointer_button_ids) })
        else {
            return ClickRoute::Pass;
        };
        let Some(index) = ids.iter().position(|known| *known == id) else { return ClickRoute::Pass };
        // SAFETY: a live view holds its pointer (vec2, GUI units) at the entry's offset
        let Some(location) = (unsafe { RawObject::read::<[f32; 2]>(pointer, calls.pointer_location) }) else {
            return ClickRoute::Pass;
        };
        let mode = calls.input_mode.and_then(|offset| {
            // SAFETY: a live view holds its InputMode (a 4-byte enum) at the entry's offset
            unsafe { RawObject::read::<u32>(pointer, offset) }.map(InputMode::from_value)
        });
        self.route(view, index, pressed, (location[0], location[1]), mode)
    }

    // the action of a route, for the runtime to run once the lock is free
    pub fn fired(route: ClickRoute) -> Option<ButtonAction> {
        match route {
            ClickRoute::Fire(action) => Some(action),
            ClickRoute::Pass | ClickRoute::Swallow => None,
        }
    }
}
