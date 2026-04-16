//! Saturation effects

pub mod channel9;
pub mod presets;
pub mod tape;

pub use channel9::Channel9;
pub use presets::{
    get_saturation_preset, get_saturation_preset_name, SaturationPreset, SATURATION_PRESETS,
    SATURATION_PRESET_NAMES,
};
pub use tape::TapeGlue;
