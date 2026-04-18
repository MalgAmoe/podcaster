export function getErrorMessage(error) {
  if (!error) return "Unknown error";

  if (error.includes("insufficient_seconds")) {
    return "Not enough time available. Upgrade to Munch Plan or buy a Snack for more processing time.";
  }
  if (error.includes("too_long")) {
    return "Preview clips must stay within the 30-second limit.";
  }
  if (error.includes("preview_busy")) {
    return "Preview service is busy right now. Try again in a moment.";
  }
  if (error.includes("invalid_input")) {
    return "Could not read that preview clip. Try a different WAV or MP3 file.";
  }
  if (error.includes("probe") || error.includes("Unsupported")) {
    return "Audio format not supported. Try converting to WAV or MP3.";
  }
  if (error.includes("No audio track")) {
    return "No audio found in file.";
  }
  if (error.includes("decode") || error.includes("Decoding") || error.includes("malformed") || error.includes("corrupt")) {
    return "Could not read the audio file. It may be corrupted.";
  }
  if (error.includes("timeout") || error.includes("timed out")) {
    return "Processing took too long. Try a shorter file.";
  }
  if (error.includes("S3") || error.includes("download")) {
    return "Could not access the file. Please re-upload.";
  }
  if (error.includes("encode") || error.includes("MP3")) {
    return "Failed to create output file.";
  }

  return "Unknown error";
}
