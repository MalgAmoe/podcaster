const getCSRFToken = () => {
  const meta = document.querySelector("meta[name='csrf-token']");
  return meta ? meta.getAttribute("content") : "";
};

// Custom error class that preserves full API response details
export class ApiError extends Error {
  constructor(data, status) {
    super(data.error || `HTTP ${status}`);
    this.name = "ApiError";
    this.status = status;
    this.code = data.error;
    // Preserve extra fields like seconds_available, seconds_needed
    this.details = data;
  }
}

// Retry configuration
const MAX_RETRIES = 3;
const INITIAL_DELAY_MS = 1000;

// Exponential backoff delay: 1s, 2s, 4s
function getRetryDelay(attempt) {
  return INITIAL_DELAY_MS * Math.pow(2, attempt);
}

// Check if error is retryable (5xx or network error)
function isRetryable(response, isNetworkError) {
  if (isNetworkError) return true;
  if (!response) return false;
  return response.status >= 500 && response.status < 600;
}

async function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

async function request(method, path, body = null) {
  const options = {
    method,
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    credentials: "same-origin",
  };

  if (body) {
    options.body = JSON.stringify(body);
  }

  let lastError = null;

  for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
    try {
      const response = await fetch(path, options);

      // Try to parse JSON, but handle non-JSON responses gracefully
      let data;
      const contentType = response.headers.get("content-type");
      if (contentType && contentType.includes("application/json")) {
        data = await response.json();
      } else {
        // Server returned non-JSON (likely an error page)
        const text = await response.text();
        data = { error: `Server error (${response.status})` };
        if (import.meta.env.DEV) console.error("Non-JSON response:", text.slice(0, 200));
      }

      if (!response.ok) {
        // Don't retry 4xx errors - they're client errors
        if (response.status >= 400 && response.status < 500) {
          throw new ApiError(data, response.status);
        }

        // Retry 5xx errors
        if (isRetryable(response, false)) {
          lastError = new ApiError(data, response.status);
          if (attempt < MAX_RETRIES - 1) {
            await sleep(getRetryDelay(attempt));
            continue;
          }
          throw lastError;
        }

        throw new ApiError(data, response.status);
      }

      return data;
    } catch (err) {
      // Network errors (fetch throws)
      if (err.name === "TypeError" || err.message === "Failed to fetch") {
        lastError = new Error("Network error. Please check your connection.");
        if (attempt < MAX_RETRIES - 1) {
          await sleep(getRetryDelay(attempt));
          continue;
        }
        throw lastError;
      }

      // JSON parse errors - server returned malformed response
      if (err.name === "SyntaxError") {
        lastError = new Error("Server returned an invalid response. Please try again.");
        if (attempt < MAX_RETRIES - 1) {
          await sleep(getRetryDelay(attempt));
          continue;
        }
        throw lastError;
      }

      // Re-throw ApiError (already handled above for retry logic)
      throw err;
    }
  }

  throw lastError || new Error("Request failed after retries");
}

export const api = {
  async getCurrentJob() {
    return request("GET", "/api/jobs/current");
  },

  async presignUpload(filename) {
    return request("POST", "/api/presign-upload", { filename });
  },

  async createJob(s3Key, filename, config) {
    return request("POST", "/api/jobs", {
      s3_key: s3Key,
      filename,
      strength: config.strength,
      ai_clean: config.ai_clean,
      duration_seconds: config.duration_seconds,
    });
  },

  async cancelJob(jobId) {
    return request("DELETE", `/api/jobs/${jobId}`);
  },

  async dismissJob(jobId) {
    return request("POST", `/api/jobs/${jobId}/dismiss`);
  },

  async getJobHistory() {
    return request("GET", "/api/jobs/history");
  },

  async getDownloadUrl(jobId) {
    return request("GET", `/api/jobs/${jobId}/download_url`);
  },
};
