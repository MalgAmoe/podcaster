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
    // Preserve extra fields like minutes_available, minutes_needed
    this.details = data;
  }
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

  const response = await fetch(path, options);
  const data = await response.json();

  if (!response.ok) {
    throw new ApiError(data, response.status);
  }

  return data;
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
      category: config.category,
      mode: config.mode,
      strength: config.strength,
      ai_clean: config.ai_clean
    });
  },

  async cancelJob(jobId) {
    return request("DELETE", `/api/jobs/${jobId}`);
  },
};
