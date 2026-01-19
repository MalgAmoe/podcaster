import { createContext, useContext, createEffect, onMount, onCleanup } from "solid-js";
import { createStore } from "solid-js/store";
import { Socket } from "phoenix";
import { api } from "../utils/api";
import { clearAudioCache } from "../components/WaveformPlayer";

const ProcessContext = createContext();

export function ProcessProvider(props) {
  const [store, setStore] = createStore({
    initializing: true, // Loading initial state
    file: null,
    s3Key: null,
    filename: null,
    uploadProgress: 0,
    uploadState: "idle", // idle, uploading, ready, error
    // Processing configuration - each category stores its own mode + strength
    processingConfig: {
      category: "voice", // "voice" | "mixed"
      voice: { mode: "natural", strength: 3 },
      mixed: { mode: "natural", strength: 3 }
    },
    job: null,
    error: null,
  });

  // Channel connection - managed outside reactive system
  let socket = null;
  let channel = null;

  // Upload abort controller - prevents race conditions and orphaned uploads
  let uploadXhr = null;

  // Check for existing job on mount
  onMount(async () => {
    try {
      // Check for existing job (reload recovery)
      const { job } = await api.getCurrentJob();
      if (job) {
        setStore({
          job,
          filename: job.filename,
          uploadState: "ready", // Show job result, not upload form
        });
      }
    } catch (err) {
      console.error("Failed to initialize:", err);
    } finally {
      setStore("initializing", false);
    }
  });

  // Cleanup on component unmount
  onCleanup(() => {
    // Abort any pending upload
    if (uploadXhr) {
      uploadXhr.abort();
      uploadXhr = null;
    }
    if (channel) channel.leave();
    if (socket) socket.disconnect();
  });

  // Connect to job channel when job.id changes (fine-grained tracking)
  createEffect(() => {
    const jobId = store.job?.id;  // Track only job.id, not entire job object

    // Clean up previous connection
    if (channel) {
      channel.leave();
      channel = null;
    }
    if (socket) {
      socket.disconnect();
      socket = null;
    }

    // Connect if we have a job and token
    if (jobId && window.userToken) {
      socket = new Socket("/socket", {
        params: { token: window.userToken }
      });
      socket.connect();

      channel = socket.channel(`job:${jobId}`, {});
      channel.join()
        .receive("error", (e) => console.error("Join failed", e));

      channel.on("job_updated", (payload) => {
        // Merge to preserve fields like original_url that server doesn't send
        setStore("job", (prev) => ({ ...prev, ...payload.job }));
      });
    }
  });

  async function uploadFile(file) {
    // Prevent concurrent uploads
    if (store.uploadState === "uploading") return;

    // Cancel any pending upload
    if (uploadXhr) {
      uploadXhr.abort();
      uploadXhr = null;
    }

    setStore({
      file,
      filename: file.name,
      uploadState: "uploading",
      uploadProgress: 0,
      error: null,
    });

    try {
      const { url, key } = await api.presignUpload(file.name);

      await new Promise((resolve, reject) => {
        const xhr = new XMLHttpRequest();
        uploadXhr = xhr;

        xhr.upload.addEventListener("progress", (e) => {
          if (e.lengthComputable) {
            setStore("uploadProgress", Math.round((e.loaded / e.total) * 100));
          }
        });

        xhr.addEventListener("load", () => {
          uploadXhr = null;
          if (xhr.status >= 200 && xhr.status < 300) {
            resolve();
          } else {
            reject(new Error(`Upload failed: ${xhr.status}`));
          }
        });

        xhr.addEventListener("error", () => {
          uploadXhr = null;
          reject(new Error("Upload failed"));
        });

        xhr.addEventListener("abort", () => {
          uploadXhr = null;
          reject(new Error("Upload cancelled"));
        });

        xhr.open("PUT", url, true);
        xhr.setRequestHeader("Content-Type", file.type || "application/octet-stream");
        xhr.send(file);
      });

      setStore({ s3Key: key, uploadState: "ready" });
    } catch (err) {
      // Don't show error for aborted uploads
      if (err.message !== "Upload cancelled") {
        setStore({ error: err.message, uploadState: "error" });
      }
    }
  }

  // Processing config actions
  function setCategory(category) {
    setStore("processingConfig", "category", category);
  }

  function setMode(mode) {
    const cat = store.processingConfig.category;
    setStore("processingConfig", cat, "mode", mode);
  }

  function setStrength(strength) {
    const cat = store.processingConfig.category;
    setStore("processingConfig", cat, "strength", Math.max(1, Math.min(5, strength)));
  }

  // Computed helpers for current config
  const currentMode = () => store.processingConfig[store.processingConfig.category].mode;
  const currentStrength = () => store.processingConfig[store.processingConfig.category].strength;

  async function submitJob() {
    if (!store.s3Key || !store.filename) {
      setStore("error", "Please upload a file first");
      return;
    }

    try {
      const cat = store.processingConfig.category;
      const config = {
        category: cat,
        mode: store.processingConfig[cat].mode,
        strength: store.processingConfig[cat].strength
      };
      const job = await api.createJob(store.s3Key, store.filename, config);
      setStore({ job, error: null });
    } catch (err) {
      setStore("error", err.message);
    }
  }

  async function cancelJob() {
    if (store.job?.id) {
      try {
        await api.cancelJob(store.job.id);
      } catch (err) {
        console.error("Failed to cancel:", err);
      }
    }
    reset();
  }

  function clearError() {
    setStore("error", null);
  }

  async function reset() {
    // Clear audio buffer cache to free memory
    clearAudioCache();

    // Delete job from server first (cleanup)
    if (store.job?.id) {
      try {
        await api.cancelJob(store.job.id);
      } catch (err) {
        // Ignore - job might already be gone
      }
    }
    setStore({
      file: null,
      s3Key: null,
      filename: null,
      uploadProgress: 0,
      uploadState: "idle",
      job: null,
      error: null,
    });
  }

  const value = {
    store,
    uploadFile,
    setCategory,
    setMode,
    setStrength,
    currentMode,
    currentStrength,
    submitJob,
    cancelJob,
    clearError,
    reset,
  };

  return (
    <ProcessContext.Provider value={value}>
      {props.children}
    </ProcessContext.Provider>
  );
}

export function useProcess() {
  const ctx = useContext(ProcessContext);
  if (!ctx) throw new Error("useProcess must be used within ProcessProvider");
  return ctx;
}
