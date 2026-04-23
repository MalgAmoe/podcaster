const STAGE_LABELS = {
  "waiting": "Waiting for available slot...",
  "queued": "Waiting for available slot...",
  "decoding": "Reading audio...",
  "processing": "Reading audio...",
  "filters": "Cutting rumble & hiss...",
  "input_gain": "Balancing levels...",
  "denoise": "Cleaning up noise...",
  "peakcomp": "Leveling out...",
  "analyzing_eq": "Checking the tone...",
  "fixeq": "Fixing muddy spots...",
  "deesser": "Taming the S's...",
  "analyzing_enhance": "Optimizing presence...",
  "enhanceeq": "Brightening up...",
  "fetcomp": "Final compression...",
  "analyzing_levels": "Measuring loudness...",
  "output": "Final limiting...",
  "completed": "Done!"
};

export function getStageLabel(stage) {
  return STAGE_LABELS[stage] || "Processing...";
}
