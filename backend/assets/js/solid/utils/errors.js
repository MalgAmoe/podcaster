// Error message helpers - returns translation keys
// The actual strings come from locales/en.js and locales/es.js

export function getErrorKey(error) {
  if (!error) return "unknownError";

  if (error.includes("insufficient_minutes")) {
    return "notEnoughMinutes";
  }
  if (error.includes("probe") || error.includes("Unsupported")) {
    return "formatNotSupported";
  }
  if (error.includes("No audio track")) {
    return "noAudioFound";
  }
  if (error.includes("decode") || error.includes("Decoding")) {
    return "couldNotReadFile";
  }
  if (error.includes("timeout") || error.includes("timed out")) {
    return "processingTooLong";
  }
  if (error.includes("S3") || error.includes("download")) {
    return "couldNotAccessFile";
  }
  if (error.includes("encode") || error.includes("MP3")) {
    return "failedToCreateOutput";
  }

  // Return unknown error key for unrecognized errors
  return "unknownError";
}

// For backwards compatibility - still used in some places
export function getFriendlyJobError(error) {
  return getErrorKey(error);
}
