import { createContext, useContext, createEffect, onMount, onCleanup } from "solid-js";
import { createStore } from "solid-js/store";
import { Socket } from "phoenix";
import { api, ApiError } from "../utils/api";
import { clearAudioCache } from "../components/WaveformPlayer";
import { getErrorKey } from "../utils/errors";
import { t, tt } from "../utils/translate";
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
    estimatedSeconds: null, // Detected audio duration in seconds
    submitting: false, // Prevents double-submit
    // Processing configuration
    processingConfig: {
      strength: 2,
      aiClean: false,
      mono: false,
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
      // Server excludes dismissed jobs, so we just restore if one exists
      const { job } = await api.getCurrentJob();
      if (job) {
        setStore({
          job,
          filename: job.filename,
          uploadState: "ready", // Show job result, not upload form
        });
      }
    } catch (err) {
      if (import.meta.env.DEV) console.error("Failed to initialize:", err);
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
          notify({ type: "success", message: t("connectionRestored") });
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
              message: t("connectionLost")
            });
          }

          if (reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) {
            // Max retries exceeded - show persistent error
            notify({
              type: "error",
              message: t("unableToConnect"),
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
        .receive("error", (e) => { if (import.meta.env.DEV) console.error("Join failed", e); });

      channel.on("job_updated", (payload) => {
        // Merge to preserve fields like original_url that server doesn't send
        setStore("job", (prev) => ({ ...prev, ...payload.job }));
      });
    }
  });

  // Get audio duration in seconds using Web Audio API
  async function getAudioDurationSeconds(file) {
    try {
      const audioContext = new (window.AudioContext || window.webkitAudioContext)();
      const arrayBuffer = await file.arrayBuffer();
      const audioBuffer = await audioContext.decodeAudioData(arrayBuffer);
      audioContext.close();
      return Math.ceil(audioBuffer.duration); // seconds, rounded up
    } catch (err) {
      if (import.meta.env.DEV) console.error("Failed to detect audio duration:", err);
      return null;
    }
  }

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
      estimatedSeconds: null,
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

      // Detect audio duration in background (don't block upload completion)
      getAudioDurationSeconds(file).then((seconds) => {
        if (seconds !== null) {
          setStore("estimatedSeconds", seconds);
        }
      });
    } catch (err) {
      // Don't show error for aborted uploads
      if (err.message !== "Upload cancelled") {
        setStore({ uploadState: "error" });
        notify({ type: "error", message: err.message });
      }
    }
  }

  // Processing config actions
  function setStrength(strength) {
    setStore("processingConfig", "strength", Math.max(1, Math.min(3, strength)));
  }

  function setAiClean(enabled) {
    setStore("processingConfig", "aiClean", enabled);
  }

  function setMono(enabled) {
    setStore("processingConfig", "mono", enabled);
  }

  // Computed helpers for current config
  const currentStrength = () => store.processingConfig.strength;
  const currentAiClean = () => store.processingConfig.aiClean;
  const currentMono = () => store.processingConfig.mono;

  async function submitJob() {
    if (!store.s3Key || !store.filename) {
      notify({ type: "error", message: t("uploadFirst") });
      return;
    }

    // Prevent double-submit
    if (store.submitting) return;
    setStore("submitting", true);

    try {
      const config = {
        strength: store.processingConfig.strength,
        ai_clean: store.processingConfig.aiClean,
        mono: store.processingConfig.mono,
        // Send duration for billing (default to 60s if not detected)
        duration_seconds: store.estimatedSeconds || 60,
      };
      const job = await api.createJob(store.s3Key, store.filename, config);
      setStore({ job });
    } catch (err) {
      // Billing errors get persistent notification with upgrade action
      if (err instanceof ApiError && err.code === "insufficient_seconds") {
        const details = err.details;
        let message = t(getErrorKey(err.code));
        if (details?.seconds_available !== undefined && details?.seconds_needed !== undefined) {
          // Format as Xm Ys
          const formatTime = (secs) => {
            const m = Math.floor(secs / 60);
            const s = secs % 60;
            return `${m}m ${s}s`;
          };
          message += tt("needTime", { needed: formatTime(details.seconds_needed), available: formatTime(details.seconds_available) });
        }
        // Get locale for locale-aware redirect
        const locale = document.documentElement.lang || "en";
        const accountPath = locale === "en" ? "/account" : `/${locale}/account`;
        notify({
          type: "error",
          message,
          persistent: true,
          action: { label: t("upgrade"), onClick: () => window.location.href = accountPath }
        });
      } else {
        notify({ type: "error", message: err.message, persistent: true });
      }
      // Reset submitting on error so user can retry
      setStore("submitting", false);
    }
  }

  async function cancelJob() {
    if (store.job?.id) {
      try {
        await api.cancelJob(store.job.id);
      } catch (err) {
        if (import.meta.env.DEV) console.error("Failed to cancel:", err);
      }
    }
    reset();
  }

  async function reset() {
    // Clear audio buffer cache to free memory
    clearAudioCache();

    if (store.job?.id) {
      if (store.job.status === "completed") {
        // Mark completed jobs as dismissed server-side so they don't restore on reload
        // Job remains in DB for job history
        try {
          await api.dismissJob(store.job.id);
        } catch (err) {
          // Ignore - job might already be gone
        }
      } else {
        // Cancel in-progress or failed jobs
        try {
          await api.cancelJob(store.job.id);
        } catch (err) {
          // Ignore - job might already be gone
        }
      }
    }
    setStore({
      file: null,
      s3Key: null,
      filename: null,
      uploadProgress: 0,
      uploadState: "idle",
      estimatedSeconds: null,
      submitting: false,
      job: null,
    });
  }

  const value = {
    store,
    uploadFile,
    setStrength,
    setAiClean,
    setMono,
    currentStrength,
    currentAiClean,
    currentMono,
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
