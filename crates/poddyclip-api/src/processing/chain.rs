//! Chain preset loading for the API

use serde::Deserialize;
use std::path::Path;

/// Chain preset configuration (matches CLI format)
#[derive(Debug, Deserialize, Clone)]
pub struct ChainPreset {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_preset")]
    pub denoiser: u8,
    #[serde(default)]
    pub dereverb: u8,
    #[serde(default)]
    pub spectral_gate: u8,
    #[serde(default)]
    pub depeak: bool,
    #[serde(default)]
    pub expander: ProcessorSetting,
    #[serde(default)]
    pub peakcomp: ProcessorSetting,
    #[serde(default)]
    pub fetcomp: ProcessorSetting,
    #[serde(default)]
    pub fixeq: ProcessorSetting,
    #[serde(default)]
    pub deesser: bool,
    #[serde(default)]
    pub saturation: ProcessorSetting,
    #[serde(default)]
    pub buttercomp: ProcessorSetting,
    #[serde(default)]
    pub enhanceeq: ProcessorSetting,
    #[serde(default)]
    pub tape: ProcessorSetting,
    #[serde(default)]
    pub output: OutputSetting,
    #[serde(default)]
    pub radio: bool,
    #[serde(default = "default_radio_amount")]
    pub radio_amount: f32,
}

/// Processor setting: preset number (1-5), disabled, or default
#[derive(Debug, Clone)]
pub enum ProcessorSetting {
    Default,
    Disabled,
    Preset(u8),
}

impl Default for ProcessorSetting {
    fn default() -> Self {
        ProcessorSetting::Disabled
    }
}

impl<'de> Deserialize<'de> for ProcessorSetting {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct ProcessorSettingVisitor;

        impl<'de> Visitor<'de> for ProcessorSettingVisitor {
            type Value = ProcessorSetting;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a preset number (1-5) or false")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v {
                    Ok(ProcessorSetting::Default)
                } else {
                    Ok(ProcessorSetting::Disabled)
                }
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v >= 1 && v <= 5 {
                    Ok(ProcessorSetting::Preset(v as u8))
                } else {
                    Err(de::Error::custom(format!("preset must be 1-5, got {}", v)))
                }
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v >= 1 && v <= 5 {
                    Ok(ProcessorSetting::Preset(v as u8))
                } else {
                    Err(de::Error::custom(format!("preset must be 1-5, got {}", v)))
                }
            }
        }

        deserializer.deserialize_any(ProcessorSettingVisitor)
    }
}

impl ProcessorSetting {
    pub fn preset(&self) -> Option<u8> {
        match self {
            ProcessorSetting::Default => Some(3),
            ProcessorSetting::Disabled => None,
            ProcessorSetting::Preset(p) => Some(*p),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self, ProcessorSetting::Disabled)
    }
}

/// Output setting: LUFS target + limiter
#[derive(Debug, Clone)]
pub enum OutputSetting {
    Default,
    Disabled,
    LufsTarget(i8),
}

impl Default for OutputSetting {
    fn default() -> Self {
        OutputSetting::Disabled
    }
}

impl<'de> Deserialize<'de> for OutputSetting {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct OutputSettingVisitor;

        impl<'de> Visitor<'de> for OutputSettingVisitor {
            type Value = OutputSetting;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a LUFS target (-14, -16, -18, -24) or false")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v {
                    Ok(OutputSetting::Default)
                } else {
                    Ok(OutputSetting::Disabled)
                }
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match v {
                    -14 | -16 | -18 | -24 => Ok(OutputSetting::LufsTarget(v as i8)),
                    _ => Err(de::Error::custom(format!(
                        "LUFS target must be -14, -16, -18, or -24, got {}",
                        v
                    ))),
                }
            }
        }

        deserializer.deserialize_any(OutputSettingVisitor)
    }
}

impl OutputSetting {
    pub fn lufs_target(&self) -> Option<f32> {
        match self {
            OutputSetting::Default => Some(-16.0),
            OutputSetting::Disabled => None,
            OutputSetting::LufsTarget(t) => Some(*t as f32),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self, OutputSetting::Disabled)
    }
}


fn default_preset() -> u8 {
    3
}

fn default_radio_amount() -> f32 {
    1.0
}

/// Validate chain name to prevent path traversal attacks.
/// Only allows alphanumeric characters, dashes, and underscores.
fn validate_chain_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Chain name cannot be empty".to_string());
    }
    if name.len() > 64 {
        return Err("Chain name too long".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "Invalid chain name '{}': only alphanumeric, dash, and underscore allowed",
            name
        ));
    }
    Ok(())
}

/// Load a chain preset by name from the given directory
pub fn load_chain(name: &str, chains_dir: &Path) -> Result<ChainPreset, String> {
    // Validate chain name to prevent path traversal
    validate_chain_name(name)?;

    let path = chains_dir.join(format!("{}.toml", name));

    if !path.exists() {
        return Err(format!(
            "Chain '{}' not found at {}",
            name,
            path.display()
        ));
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    toml::from_str(&content).map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
}

/// List all available chain presets from the given directory
pub fn list_chains(chains_dir: &Path) -> Vec<(String, String)> {
    let mut chains = Vec::new();

    if let Ok(entries) = std::fs::read_dir(chains_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "toml").unwrap_or(false) {
                if let Some(name) = path.file_stem() {
                    let name_str = name.to_string_lossy().to_string();
                    if let Ok(chain) = load_chain(&name_str, chains_dir) {
                        chains.push((name_str, chain.description.unwrap_or_default()));
                    }
                }
            }
        }
    }

    chains.sort_by(|a, b| a.0.cmp(&b.0));
    chains
}
