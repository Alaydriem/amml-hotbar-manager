// the descriptor the manager registers the mod from; it names the claim, the three keybinds and the settings schema
pub struct HotbarDescriptor;

impl HotbarDescriptor {
    pub const TEXT: &'static str = include_str!("../../descriptor.hcl");
}
