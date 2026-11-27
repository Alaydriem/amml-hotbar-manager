use amml_api::{InputMode, Logger, Point, ScreenView, TextureReload, Textures, UiDraw, UiProfile};
use hotbar_manager_core::structs::{ButtonAction, ButtonSpec, PanelRect, ScreenKind};

use crate::services::{ButtonPainter, ClickRouter, KindMap};
use crate::types::ClickRoute;

// The mod buttons on the open classic screen: the `ScreenRender` listener draws them after the game, and the
// `UiPointer` listener asks which pointer events they take.
//
// Each container screen is also rendered by a second view at a taller size (docs/findings/spike-align/report.md
// section 6). So the drawn layout is kept per view, each view's clicks are tested against its own layout, and only
// a button event from another view that drew buttons (or the held view showing no buttons) clears the held flags: a
// render of the second view never does, and neither does an event of a view that drew none
pub struct ButtonUi {
    router: ClickRouter,
    painter: ButtonPainter,
    log: Logger,
    views: Vec<ViewLayout>,
}

// the buttons one view drew on its last frame
#[derive(Debug, Clone, PartialEq)]
pub struct ViewLayout {
    pub view: u64,
    pub kind: ScreenKind,
    pub panel: PanelRect,
    pub specs: Vec<ButtonSpec>,
}

impl ButtonUi {
    // the views kept: the real view and the second render, with room for a screen that opens over another
    pub const MAX_VIEWS: usize = 4;

    pub fn new(textures: Textures, log: Logger) -> Self {
        Self {
            router: ClickRouter::new(),
            painter: ButtonPainter::new(textures, log),
            log,
            views: Vec::with_capacity(Self::MAX_VIEWS),
        }
    }

    // disable: no old layout or held flag is kept
    pub fn clear(&mut self) {
        self.views.clear();
        self.router.clear();
    }

    // the textures were reloaded: the painter reads its set again. Gives the number of textures it held
    pub fn textures_reloaded(&mut self, reason: TextureReload) -> usize {
        self.painter.textures_reloaded(reason)
    }

    pub fn layout(&self, view: u64) -> Option<&ViewLayout> {
        self.views.iter().find(|layout| layout.view == view)
    }

    // the view drew `layout`; it becomes the newest kept view
    pub fn remember(&mut self, layout: ViewLayout) {
        self.views.retain(|kept| kept.view != layout.view);
        self.views.insert(0, layout);
        self.views.truncate(Self::MAX_VIEWS);
    }

    // the view drew no buttons this frame
    pub fn forget(&mut self, view: u64) {
        self.views.retain(|kept| kept.view != view);
        self.router.forget_view(view);
    }

    // one pointer event of `view`: the route the router gives against the view's last drawn layout. A view with no
    // layout takes only the release of a press it swallowed. The game hands each pointer event to every view of the
    // screen stack (on 1.26.63.01 two views that draw nothing get every event after the screen's view), so an event
    // of a view that drew no buttons and holds no press goes to the game and leaves the held flags alone
    pub fn route(
        &mut self,
        view: u64,
        pointer: usize,
        pressed: bool,
        pointer_units: (f32, f32),
        mode: InputMode,
    ) -> ClickRoute {
        let layout = self.layout(view);
        if layout.is_none() && !self.router.holds(view) {
            return ClickRoute::Pass;
        }
        let over = layout.and_then(|layout| ClickRouter::hit(&layout.specs, layout.panel, pointer_units));
        self.router.route(view, pointer, pressed, over, mode)
    }

    // the body of the `ScreenRender` listener: draws the buttons of the view's screen and keeps the drawn layout for the
    // view's clicks
    pub fn on_render(&mut self, view_id: u64, view: &ScreenView<'_>, draw: &UiDraw<'_>) {
        match self.draw(view_id, view, draw) {
            Some(layout) if !layout.specs.is_empty() => self.remember(layout),
            _ => self.forget(view_id),
        }
    }

    fn draw(&mut self, view_id: u64, view: &ScreenView<'_>, draw: &UiDraw<'_>) -> Option<ViewLayout> {
        let kind = KindMap::kind(view.kind());
        if kind == ScreenKind::Other {
            return None;
        }
        let profile = view.ui_profile().unwrap_or(UiProfile::Classic);
        let specs = ClickRouter::buttons(kind, profile);
        if specs.is_empty() {
            return None;
        }
        let rect = view.content_rect()?;
        let panel = PanelRect { x: rect.x, y: rect.y, w: rect.width, h: rect.height };
        let scale = draw.gui_scale()?;
        if !(scale.is_finite() && scale > 0.0) {
            return None;
        }
        // a frame the game left images queued in: no buttons drawn, and the view keeps its last layout, so a held
        // press is not cleared
        if !draw.can_draw() {
            return self.layout(view_id).cloned();
        }
        let pointer: Point = view.pointer()?;
        let hovered = ClickRouter::hit(&specs, panel, (pointer.x, pointer.y));
        let pressed = self.router.pressed(view_id, hovered);
        let drawn = self.painter.paint(draw, kind, panel, hovered, pressed);
        let layout = ViewLayout { view: view_id, kind, panel, specs: drawn };
        self.note_layout(&layout, view.size());
        Some(layout)
    }

    // one log line the first time a view draws buttons, and one per button when its place changes, with the centre of
    // the button as a fraction of the view (the in-game script clicks there)
    fn note_layout(&self, layout: &ViewLayout, size: Option<amml_api::Size>) {
        let kept = self.layout(layout.view);
        if kept.is_some_and(|kept| kept.panel == layout.panel && kept.kind == layout.kind) || layout.specs.is_empty() {
            return;
        }
        self.log.info(
            "buttons drawn",
            &[
                ("kind", &format!("{:?}", layout.kind)),
                ("profile", "Classic"),
                ("buttons", &layout.specs.len().to_string()),
            ],
        );
        let Some(size) = size.filter(|size| size.width > 0 && size.height > 0) else { return };
        for spec in &layout.specs {
            let rect = layout.panel.resolve(spec);
            let rx = (rect.x + rect.w / 2.0) / size.width as f32;
            let ry = (rect.y + rect.h / 2.0) / size.height as f32;
            self.log.info(
                "button placed",
                &[
                    ("kind", &format!("{:?}", layout.kind)),
                    ("action", &format!("{:?}", spec.action)),
                    ("rx", &format!("{rx:.4}")),
                    ("ry", &format!("{ry:.4}")),
                ],
            );
        }
    }

    // the action of a route, for the runtime to run once the lock is free
    pub fn fired(route: ClickRoute) -> Option<ButtonAction> {
        match route {
            ClickRoute::Fire(action) => Some(action),
            ClickRoute::Pass | ClickRoute::Swallow => None,
        }
    }
}
