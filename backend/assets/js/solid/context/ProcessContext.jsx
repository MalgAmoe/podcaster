import { createContext, useContext, createEffect, onMount, onCleanup } from "solid-js";
import { createStore } from "solid-js/store";
import { Socket } from "phoenix";
import { api, ApiError } from "../utils/api";
import { clearAudioCache } from "../components/WaveformPlayer";
import { getFriendlyJobError } from "../utils/errors";
import { useNotifications } from "./NotificationContext";

const ProcessContext = createContext();

export function ProcessProvider(props) {
  const { notify } = useNotifications();

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
      voice: { mode: "natural", strength: 2, aiClean: false },
      mixed: { mode: "natural", strength: 2, aiClean: false }
    },
    job: null,
  });

  // Channel connection - managed outside reactive system
  let socket = null;
  let channel = null;

  // WebSocket reconnection state
  let reconnectAttempts = 0;
  let wasConnected = false;
  let connectionLostNotificationId = null;
  const MAX_RECONNECT_ATTEMPTS = 3;
  const INITIAL_RECONNECT_DELAY = 1000; // 1s, 2s, 4s with exponential backoff

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

    // Reset reconnection state when job changes
    reconnectAttempts = 0;
    wasConnected = false;
    if (connectionLostNotificationId) {
      connectionLostNotificationId = null;
    }

    // Connect if we have a job and token
    if (jobId && window.userToken) {
      socket = new Socket("/socket", {
        params: { token: window.userToken },
        reconnectAfterMs: (tries) => {
          // Exponential backoff: 1s, 2s, 4s
          return Math.min(INITIAL_RECONNECT_DELAY * Math.pow(2, tries - 1), 10000);
        }
      });

      // Track socket connection state
      socket.onOpen(() => {
        if (wasConnected && reconnectAttempts > 0) {
          // Successfully reconnected
          if (connectionLostNotificationId) {
            // We'll let the auto-dismiss handle it, but show success
          }
          notify({ type: "success", message: "Connection restored" });
        }
        wasConnected = true;
        reconnectAttempts = 0;
        connectionLostNotificationId = null;
      });

      socket.onClose(() => {
        if (wasConnected) {
          reconnectAttempts++;

          if (reconnectAttempts === 1) {
            // First disconnect - show warning
            connectionLostNotificationId = notify({
              type: "warning",
              message: "Connection lost. Reconnecting..."
            });
          }

          if (reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) {
            // Max retries exceeded - show persistent error
            notify({
              type: "error",
              message: "Unable to connect. Please refresh the page.",
              persistent: true
            });
          }
        }
      });

      socket.onError(() => {
        // Socket errors are followed by close, so we handle in onClose
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
        setStore({ uploadState: "error" });
        notify({ type: "error", message: err.message });
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
    setStore("processingConfig", cat, "strength", Math.max(1, Math.min(3, strength)));
  }

  function setAiClean(enabled) {
    const cat = store.processingConfig.category;
    setStore("processingConfig", cat, "aiClean", enabled);
  }

  // Computed helpers for current config
  const currentMode = () => store.processingConfig[store.processingConfig.category].mode;
  const currentStrength = () => store.processingConfig[store.processingConfig.category].strength;
  const currentAiClean = () => store.processingConfig[store.processingConfig.category].aiClean;

  async function submitJob() {
    if (!store.s3Key || !store.filename) {
      notify({ type: "error", message: "Please upload a file first" });
      return;
    }

    try {
      const cat = store.processingConfig.category;
      const catConfig = store.processingConfig[cat];
      const config = {
        category: cat,
        mode: catConfig.mode,
        strength: catConfig.strength,
        // Only send ai_clean for voice category where it can be toggled
        ai_clean: cat === "voice" ? catConfig.aiClean : undefined
      };
      const job = await api.createJob(store.s3Key, store.filename, config);
      setStore({ job });
    } catch (err) {
      // Billing errors get persistent notification with upgrade action
      if (err instanceof ApiError && err.code === "insufficient_minutes") {
        const details = err.details;
        let message = getFriendlyJobError(err.code);
        if (details?.minutes_available !== undefined && details?.minutes_needed !== undefined) {
          message += ` You need ${details.minutes_needed} minutes but only have ${details.minutes_available} available.`;
        }
        notify({
          type: "error",
          message,
          persistent: true,
          action: { label: "Upgrade", onClick: () => window.location.href = "/account" }
        });
      } else {
        notify({ type: "error", message: err.message, persistent: true });
      }
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
    });
  }

  const value = {
    store,
    uploadFile,
    setCategory,
    setMode,
    setStrength,
    setAiClean,
    currentMode,
    currentStrength,
    currentAiClean,
    submitJob,
    cancelJob,
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
