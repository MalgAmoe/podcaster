// English translations for Solid app
export default {
  // Upload zone
  feedTheCow: "Feed the cow!",
  veryHungry: "She's VERY hungry for your audio",
  nomNomNom: "nom nom nom - WAV, MP3, FLAC",
  readyToMunch: "Ready to munch!",
  estimatedTime: "~{time}",
  finalizing: "Finalizing...",
  removeFile: "Remove file",
  uploadAudioFile: "Upload audio file. Click or drop a file here.",

  // Validation errors
  selectAudioFile: "Please select an audio file (WAV, MP3, FLAC, etc.)",
  fileTooLarge: "File too large. Maximum size is 500MB.",
  uploadFirst: "Please upload a file first",

  // Category toggle
  whatProcessing: "What are you processing?",
  voice: "Voice",
  mixedAudio: "Mixed Audio",

  // Mode selector
  processingMode: "Processing mode",
  natural: "Natural",
  studio: "Studio",
  aiClean: "AI Clean",
  takesLonger: "Takes longer to process",

  // Strength knob
  strength: "Strength",
  subtle: "Subtle",
  balanced: "Balanced",
  intense: "Intense",

  // Steps indicator
  feed: "Feed",
  munch: "Munch",
  enjoy: "Enjoy",

  // Main controls
  munchIt: "MUNCH IT!",

  // Job progress
  munchMunchMunch: "*munch munch munch*",
  munching: "Munching...",
  cancel: "Cancel",

  // Job complete
  mooo: "MOOO!",
  audioReady: "Your audio is ready!",
  original: "Original",
  processed: "Processed",
  download: "Download",
  downloadProcessedAudio: "Download processed audio",
  feedMeMore: "Feed me more!",
  uploadAnotherFile: "Upload another file",
  audioComparison: "Audio comparison",

  // Job failed
  cowChoked: "The cow choked!",
  feedHerAgain: "Feed her again",

  // Past munchings
  pastMunchings: "Past Munchings",
  pastDescription: "Your processed files from the last 7 days.",
  noMunchingsYet: "No munchings yet. Your processed files will appear here for 7 days.",
  today: "Today at {time}",
  yesterday: "Yesterday at {time}",
  daysAgo: "{count} days ago at {time}",

  // Connection status
  connectionRestored: "Connection restored",
  connectionLost: "Connection lost. Reconnecting...",
  unableToConnect: "Unable to connect. Please refresh the page.",

  // Billing
  notEnoughTime: "Not enough time available. Upgrade to Munch Plan or buy a Snack for more processing time.",
  needTime: " You need {needed} but only have {available} available.",
  upgrade: "See options",

  // Error messages
  unknownError: "Unknown error",
  formatNotSupported: "Audio format not supported. Try converting to WAV or MP3.",
  noAudioFound: "No audio found in file.",
  couldNotReadFile: "Could not read the audio file. It may be corrupted.",
  processingTooLong: "Processing took too long. Try a shorter file.",
  couldNotAccessFile: "Could not access the file. Please re-upload.",
  failedToCreateOutput: "Failed to create output file.",

  // Processing stages
  processing: "Processing...",
  stages: {
    waiting: "Waiting for available slot...",
    decoding: "Reading audio...",
    filters: "Cutting rumble & hiss...",
    input_gain: "Balancing levels...",
    analyzing_reverb: "Detecting room sound...",
    dereverb: "Removing room echo...",
    analyzing_noise: "Finding background noise...",
    denoise: "Cleaning up noise...",
    ai_denoise: "AI cleaning voice...",
    spectral_gate: "Gating quiet parts...",
    analyzing_peaks: "Finding harsh tones...",
    peak_attenuation: "Smoothing harsh tones...",
    expander: "Opening up dynamics...",
    compressor: "Leveling out...",
    analyzing_eq: "Checking the tone...",
    fixeq: "Fixing muddy spots...",
    deesser: "Taming the S's...",
    saturation: "Adding warmth...",
    buttercomp: "Gluing it together...",
    analyzing_enhance: "Optimizing presence...",
    enhanceeq: "Brightening up...",
    radio: "Broadcast polish...",
    tape: "Adding analog feel...",
    analyzing_levels: "Measuring loudness...",
    output: "Final limiting...",
    encoding: "Saving your file...",
    completed: "Done!"
  }
};
