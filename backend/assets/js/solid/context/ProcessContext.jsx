import { createContext, useContext, createEffect, onMount, onCleanup } from "solid-js";
import { createStore } from "solid-js/store";
import { Socket } from "phoenix";
import { api } from "../utils/api";

const ProcessContext = createContext();

export function ProcessProvider(props) {
  const [store, setStore] = createStore({
    file: null,
    s3Key: null,
    filename: null,
    uploadProgress: 0,
    uploadState: "idle", // idle, uploading, ready, error
    presets: ["podcast", "broadcast", "gentle"],
    selectedPreset: "podcast",
    job: null,
    error: null,
  });

  // Channel connection - managed outside reactive system
  let socket = null;
  let channel = null;

  // Load presets and check for existing job on mount
  onMount(async () => {
    try {
      // Load presets
      const { presets } = await api.getPresets();
      setStore("presets", presets);

      // Check for existing job (reload recovery)
      const { job } = await api.getCurrentJob();
      if (job) {
        console.log("Recovered job:", job.id, job.status);
        setStore({
          job,
          filename: job.filename,
          uploadState: "ready", // Show job result, not upload form
        });
      }
    } catch (err) {
      console.error("Failed to initialize:", err);
    }
  });

  // Cleanup on component unmount
  onCleanup(() => {
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
        .receive("ok", () => console.log("Joined job:" + jobId))
        .receive("error", (e) => console.error("Join failed", e));

      channel.on("job_updated", (payload) => {
        // Merge to preserve fields like original_url that server doesn't send
        setStore("job", (prev) => ({ ...prev, ...payload.job }));
      });
    }
  });

  async function uploadFile(file) {
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

        xhr.upload.addEventListener("progress", (e) => {
          if (e.lengthComputable) {
            setStore("uploadProgress", Math.round((e.loaded / e.total) * 100));
          }
        });

        xhr.addEventListener("load", () => {
          if (xhr.status >= 200 && xhr.status < 300) {
            resolve();
          } else {
            reject(new Error(`Upload failed: ${xhr.status}`));
          }
        });

        xhr.addEventListener("error", () => reject(new Error("Upload failed")));

        xhr.open("PUT", url, true);
        xhr.setRequestHeader("Content-Type", file.type || "application/octet-stream");
        xhr.send(file);
      });

      setStore({ s3Key: key, uploadState: "ready" });
    } catch (err) {
      setStore({ error: err.message, uploadState: "error" });
    }
  }

  function selectPreset(preset) {
    setStore("selectedPreset", preset);
  }

  async function submitJob() {
    if (!store.s3Key || !store.filename) {
      setStore("error", "Please upload a file first");
      return;
    }

    try {
      const job = await api.createJob(store.s3Key, store.filename, store.selectedPreset);
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

  function reset() {
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
    selectPreset,
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
