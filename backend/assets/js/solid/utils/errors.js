// Error message helpers from LiveView (process_live.ex)

export function getFriendlyJobError(error) {
  if (!error) return "Unknown error";

  if (error.includes("insufficient_minutes")) {
    return "Not enough minutes available. Upgrade to Pro for more processing time.";
  }
  if (error.includes("probe") || error.includes("Unsupported")) {
    return "Audio format not supported. Try converting to WAV or MP3.";
  }
  if (error.includes("No audio track")) {
    return "No audio found in file.";
  }
  if (error.includes("decode") || error.includes("Decoding")) {
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

  return error;
}

// Check if error is a billing-related error that should show upgrade CTA
export function isBillingError(error) {
  if (!error) return false;
  return error.includes("insufficient_minutes");
}

export function getUploadError(error) {
  switch (error) {
    case "too_large":
      return "File is too large (max 500MB)";
    case "not_accepted":
      return "File type not accepted";
    case "too_many_files":
      return "Too many files";
    default:
      return `Error: ${error}`;
  }
}
