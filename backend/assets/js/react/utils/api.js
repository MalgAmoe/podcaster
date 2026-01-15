// API client for React frontend

const getCSRFToken = () => {
  const meta = document.querySelector("meta[name='csrf-token']");
  return meta ? meta.getAttribute("content") : "";
};

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
    throw new Error(data.error || `HTTP ${response.status}`);
  }

  return data;
}

export const api = {
  // GET /api/presets
  async getPresets() {
    return request("GET", "/api/presets");
  },

  // POST /api/presign-upload
  async presignUpload(filename) {
    return request("POST", "/api/presign-upload", { filename });
  },

  // POST /api/jobs
  async createJob(s3Key, filename, preset) {
    return request("POST", "/api/jobs", {
      s3_key: s3Key,
      filename,
      preset,
    });
  },

  // DELETE /api/jobs/:id
  async cancelJob(jobId) {
    return request("DELETE", `/api/jobs/${jobId}`);
  },

  // GET /api/user
  async getCurrentUser() {
    return request("GET", "/api/user");
  },
};
