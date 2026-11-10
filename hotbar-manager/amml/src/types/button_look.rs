// The state a light glyph button is drawn in (common_buttons.light_glyph_button: default, hover, pressed)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonLook {
    Default,
    Hover,
    Pressed,
}
