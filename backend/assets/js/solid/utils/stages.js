// Stage name mappings from LiveView (process_live.ex)
export const STAGE_NAMES = {
  null: "Reading audio...",
  undefined: "Reading audio...",
  "decoding": "Reading audio...",
  "filters": "Cutting rumble & hiss...",
  "input_gain": "Balancing levels...",
  "analyzing_reverb": "Detecting room sound...",
  "dereverb": "Removing room echo...",
  "analyzing_noise": "Finding background noise...",
  "denoise": "Cleaning up noise...",
  "spectral_gate": "Gating quiet parts...",
  "analyzing_peaks": "Finding harsh tones...",
  "peak_attenuation": "Smoothing harsh tones...",
  "expander": "Opening up dynamics...",
  "compressor": "Leveling out...",
  "analyzing_eq": "Checking the tone...",
  "fixeq": "Fixing muddy spots...",
  "deesser": "Taming the S's...",
  "saturation": "Adding warmth...",
  "buttercomp": "Gluing it together...",
  "analyzing_enhance": "Optimizing presence...",
  "enhanceeq": "Brightening up...",
  "radio": "Broadcast polish...",
  "tape": "Adding analog feel...",
  "analyzing_levels": "Measuring loudness...",
  "output": "Final limiting...",
  "encoding": "Saving your file...",
  "completed": "Done!"
};

export function getFriendlyStage(stage) {
  return STAGE_NAMES[stage] || stage || "Processing...";
}
