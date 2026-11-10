use amml_sdk::SdkError;

use crate::traits::GameHost;

// The resolved offsets and slots the screen detector reads: the root control's name, and the UI profile through the
// client's options. All entries have recipes (docs/findings/spike-panel/report.md section 1)
#[derive(Clone, Copy, Debug)]
pub struct ScreenCalls {
    // offset of `ScreenView::mVisualTree`
    pub visual_tree: u64,
    // offset of `VisualTree::mRootControl`
    pub root_control: u64,
    // offset of `UIControl::mName`
    pub control_name: u64,
    // vtable slot of `IOptionRegistry& IClientInstance::getOptions()`
    pub get_options_slot: u64,
    // vtable slot of `UIProfile IOptionsReader::getUIProfile() const`
    pub get_ui_profile_slot: u64,
}

impl ScreenCalls {
    pub const VISUAL_TREE: &'static str = "ScreenView.mVisualTree";
    pub const ROOT_CONTROL: &'static str = "VisualTree.mRootControl";
    pub const CONTROL_NAME: &'static str = "UIControl.mName";
    pub const GET_OPTIONS: &'static str = "ClientInstance.getOptions";
    pub const GET_UI_PROFILE: &'static str = "IOptionRegistry.getUIProfile";
    pub const ENTRIES: [&'static str; 5] =
        [Self::VISUAL_TREE, Self::ROOT_CONTROL, Self::CONTROL_NAME, Self::GET_OPTIONS, Self::GET_UI_PROFILE];

    // the calls of the running build; an error names the first entry that is missing
    pub fn resolve<H: GameHost>(host: &H) -> Result<Self, SdkError> {
        Ok(Self {
            visual_tree: host.resolve(Self::VISUAL_TREE)?,
            root_control: host.resolve(Self::ROOT_CONTROL)?,
            control_name: host.resolve(Self::CONTROL_NAME)?,
            get_options_slot: host.resolve(Self::GET_OPTIONS)?,
            get_ui_profile_slot: host.resolve(Self::GET_UI_PROFILE)?,
        })
    }
}
