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
