use amml_sdk::SdkError;

use crate::traits::GameHost;

// The resolved game code the hidden inventory open needs besides the packet path and the player calls: the player's
// runtime id, which the Interact packet names as its target. The entry name is stratum's
#[derive(Clone, Copy, Debug)]
pub struct RotationCalls {
    // `ActorRuntimeID Actor::getRuntimeID() const`, the body
    pub get_runtime_id: usize,
}

impl RotationCalls {
    pub const GET_RUNTIME_ID: &'static str = "Actor.getRuntimeID";

    // the calls of the running build; an error names the entry that is missing
    pub fn resolve<H: GameHost>(host: &H) -> Result<Self, SdkError> {
        Ok(Self { get_runtime_id: host.resolve(Self::GET_RUNTIME_ID)? as usize })
    }
}
