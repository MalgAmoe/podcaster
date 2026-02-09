// Stage name mappings - now using translation keys
// The actual strings come from locales/en.js and locales/es.js

const STAGE_KEYS = {
  null: "stages.waiting",
  undefined: "stages.waiting",
  "waiting": "stages.waiting",
  "decoding": "stages.decoding",
  "filters": "stages.filters",
  "input_gain": "stages.input_gain",
  "analyzing_reverb": "stages.analyzing_reverb",
  "dereverb": "stages.dereverb",
  "analyzing_noise": "stages.analyzing_noise",
  "denoise": "stages.denoise",
  "ai_denoise": "stages.ai_denoise",
  "spectral_gate": "stages.spectral_gate",
  "analyzing_peaks": "stages.analyzing_peaks",
  "peak_attenuation": "stages.peak_attenuation",
  "expander": "stages.expander",
  "peakcomp": "stages.peakcomp",
  "analyzing_eq": "stages.analyzing_eq",
  "fixeq": "stages.fixeq",
  "deesser": "stages.deesser",
  "saturation": "stages.saturation",
  "buttercomp": "stages.buttercomp",
  "analyzing_enhance": "stages.analyzing_enhance",
  "enhanceeq": "stages.enhanceeq",
  "radio": "stages.radio",
  "fetcomp": "stages.fetcomp",
  "tape": "stages.tape",
  "analyzing_levels": "stages.analyzing_levels",
  "output": "stages.output",
  "encoding": "stages.encoding",
  "completed": "stages.completed"
};

export function getStageKey(stage) {
  return STAGE_KEYS[stage] || "processing";
}

// For backwards compatibility - returns translation key
export function getFriendlyStage(stage) {
  return STAGE_KEYS[stage] || stage || "processing";
}
