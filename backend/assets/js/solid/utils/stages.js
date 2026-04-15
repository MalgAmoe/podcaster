const STAGE_LABELS = {
  "waiting": "Waiting for available slot...",
  "decoding": "Reading audio...",
  "filters": "Cutting rumble & hiss...",
  "input_gain": "Balancing levels...",
  "analyzing_reverb": "Detecting room sound...",
  "dereverb": "Removing room echo...",
  "analyzing_noise": "Finding background noise...",
  "denoise": "Cleaning up noise...",
  "ai_denoise": "AI cleaning voice...",
  "spectral_gate": "Gating quiet parts...",
  "peakcomp": "Leveling out...",
  "analyzing_eq": "Checking the tone...",
  "fixeq": "Fixing muddy spots...",
  "deesser": "Taming the S's...",
  "saturation": "Adding warmth...",
  "buttercomp": "Gluing it together...",
  "analyzing_enhance": "Optimizing presence...",
  "enhanceeq": "Brightening up...",
  "radio": "Broadcast polish...",
  "fetcomp": "Final compression...",
  "tape": "Adding analog feel...",
  "analyzing_levels": "Measuring loudness...",
  "output": "Final limiting...",
  "completed": "Done!"
};

export function getStageLabel(stage) {
  return STAGE_LABELS[stage] || "Processing...";
}
