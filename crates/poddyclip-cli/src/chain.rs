//! Chain preset loading and parsing
//!
//! Chain presets are TOML files that combine all processor settings into
//! named configurations.

#![allow(dead_code)]

use serde::Deserialize;
use std::path::PathBuf;

/// Chain preset configuration
#[derive(Debug, Deserialize)]
pub struct ChainPreset {
    /// Display name
    pub name: String,

    /// Optional description
    #[serde(default)]
    pub description: Option<String>,

    /// Denoiser preset (1-5, always enabled)
    #[serde(default = "default_preset")]
    pub denoiser: u8,

    /// Expander setting (1-5 or false to disable)
    #[serde(default)]
    pub expander: ProcessorSetting,

    /// Compressor setting (type + preset, or false to disable)
    #[serde(default)]
    pub compressor: CompressorSetting,

    /// FixEQ enabled
    #[serde(default = "default_true")]
    pub fixeq: bool,

    /// DeEsser enabled
    #[serde(default = "default_true")]
    pub deesser: bool,

    /// Saturation/Channel9 setting (1-5 or false)
    #[serde(default)]
    pub saturation: ProcessorSetting,

    /// ButterComp setting (1-5 or false)
    #[serde(default)]
    pub buttercomp: ProcessorSetting,

    /// EnhanceEQ setting (1-5 or false)
    #[serde(default)]
    pub enhanceeq: ProcessorSetting,

    /// TapeGlue setting (1-5 or false)
    #[serde(default)]
    pub tape: ProcessorSetting,

    /// Output setting: LUFS target (-14, -16, -18, -24) + limiter, or false to disable
    #[serde(default)]
    pub output: OutputSetting,
}

/// Processor setting: can be a preset number (1-5), false to disable, or omitted for default
#[derive(Debug, Clone)]
pub enum ProcessorSetting {
    /// Use default preset (3)
    Default,
    /// Disabled
    Disabled,
    /// Specific preset (1-5)
    Preset(u8),
}

impl Default for ProcessorSetting {
    fn default() -> Self {
        ProcessorSetting::Default
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
                    // true means use default
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
                    Err(de::Error::custom(format!(
                        "preset must be 1-5, got {}",
                        v
                    )))
                }
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v >= 1 && v <= 5 {
                    Ok(ProcessorSetting::Preset(v as u8))
                } else {
                    Err(de::Error::custom(format!(
                        "preset must be 1-5, got {}",
                        v
                    )))
                }
            }
        }

        deserializer.deserialize_any(ProcessorSettingVisitor)
    }
}

impl ProcessorSetting {
    /// Get the preset value, or None if disabled
    pub fn preset(&self) -> Option<u8> {
        match self {
            ProcessorSetting::Default => Some(3),
            ProcessorSetting::Disabled => None,
            ProcessorSetting::Preset(p) => Some(*p),
        }
    }

    /// Check if the processor is enabled
    pub fn is_enabled(&self) -> bool {
        !matches!(self, ProcessorSetting::Disabled)
    }
}

/// Output setting: LUFS target + limiter paired together
#[derive(Debug, Clone)]
pub enum OutputSetting {
    /// Use default (-16 LUFS + limiter)
    Default,
    /// Disabled (no LUFS normalization, no limiter)
    Disabled,
    /// Specific LUFS target (-14, -16, -18, -24) + limiter
    LufsTarget(i8),
}

impl Default for OutputSetting {
    fn default() -> Self {
        OutputSetting::Default
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
    /// Get the LUFS target, or None if disabled
    pub fn lufs_target(&self) -> Option<f32> {
        match self {
            OutputSetting::Default => Some(-16.0),
            OutputSetting::Disabled => None,
            OutputSetting::LufsTarget(t) => Some(*t as f32),
        }
    }

    /// Check if output processing is enabled
    pub fn is_enabled(&self) -> bool {
        !matches!(self, OutputSetting::Disabled)
    }
}

/// Compressor setting: type (peak or fet) and preset
#[derive(Debug, Clone)]
pub struct CompressorSetting {
    /// Compressor type: "peak" or "fet"
    pub comp_type: CompressorType,
    /// Preset level (1-5)
    pub preset: u8,
    /// Whether compressor is enabled
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompressorType {
    Peak,
    Fet,
}

impl Default for CompressorSetting {
    fn default() -> Self {
        CompressorSetting {
            comp_type: CompressorType::Peak,
            preset: 3,
            enabled: true,
        }
    }
}

impl<'de> Deserialize<'de> for CompressorSetting {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};

        struct CompressorSettingVisitor;

        impl<'de> Visitor<'de> for CompressorSettingVisitor {
            type Value = CompressorSetting;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a compressor setting table { type = \"peak\"/\"fet\", preset = 1-5 } or false")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v {
                    Ok(CompressorSetting::default())
                } else {
                    Ok(CompressorSetting {
                        comp_type: CompressorType::Peak,
                        preset: 3,
                        enabled: false,
                    })
                }
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut comp_type = CompressorType::Peak;
                let mut preset = 3u8;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "type" => {
                            let type_str: String = map.next_value()?;
                            comp_type = match type_str.as_str() {
                                "peak" => CompressorType::Peak,
                                "fet" => CompressorType::Fet,
                                _ => {
                                    return Err(de::Error::custom(format!(
                                        "compressor type must be 'peak' or 'fet', got '{}'",
                                        type_str
                                    )))
                                }
                            };
                        }
                        "preset" => {
                            preset = map.next_value()?;
                            if preset < 1 || preset > 5 {
                                return Err(de::Error::custom(format!(
                                    "preset must be 1-5, got {}",
                                    preset
                                )));
                            }
                        }
                        _ => {
                            let _: toml::Value = map.next_value()?;
                        }
                    }
                }

                Ok(CompressorSetting {
                    comp_type,
                    preset,
                    enabled: true,
                })
            }
        }

        deserializer.deserialize_any(CompressorSettingVisitor)
    }
}

fn default_preset() -> u8 {
    3
}

fn default_true() -> bool {
    true
}

/// Load a chain preset by name
pub fn load_chain(name: &str) -> Result<ChainPreset, String> {
    let chain_dir = get_chains_dir();
    let path = chain_dir.join(format!("{}.toml", name));

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

/// List all available chain presets
pub fn list_chains() -> Vec<(String, String)> {
    let chain_dir = get_chains_dir();
    let mut chains = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&chain_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "toml").unwrap_or(false) {
                if let Some(name) = path.file_stem() {
                    let name_str = name.to_string_lossy().to_string();
                    if let Ok(chain) = load_chain(&name_str) {
                        chains.push((name_str, chain.description.unwrap_or_default()));
                    }
                }
            }
        }
    }

    chains.sort_by(|a, b| a.0.cmp(&b.0));
    chains
}

/// Get the chains directory path
fn get_chains_dir() -> PathBuf {
    // Look for chains/ next to executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let chains = dir.join("chains");
            if chains.exists() {
                return chains;
            }
        }
    }

    // Fallback to current directory
    PathBuf::from("chains")
}
