import { createContext, useContext, createEffect, onMount, onCleanup } from "solid-js";
import { createStore } from "solid-js/store";
import { Socket } from "phoenix";
import { api, ApiError } from "../utils/api";
import { clearAudioCache } from "../components/WaveformPlayer";
import { getErrorMessage } from "../utils/errors";
import { trimAudioFileForPreview } from "../utils/previewAudio";
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
    previewClip: null,
    submitting: false, // Prevents double-submit
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

  // Upload / preview cancellation handles prevent overlapping guest/user flows
  let uploadXhr = null;
  let previewAbortController = null;
  let previewBlobUrl = null;

  function revokePreviewBlobUrl() {
    if (previewBlobUrl) {
      URL.revokeObjectURL(previewBlobUrl);
      previewBlobUrl = null;
    }
  }

  // Check for existing job on mount
  onMount(async () => {
    try {
      // Restore an existing server-backed job after reload.
      const { job } = await api.getCurrentJob();
      if (job) {
        setStore({
          job,
          filename: job.filename,
          uploadState: "ready",
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
    if (previewAbortController) {
      previewAbortController.abort();
      previewAbortController = null;
    }
    revokePreviewBlobUrl();
    if (channel) channel.leave();
    if (socket) socket.disconnect();
  });

  // Connect to Phoenix channels only for server-backed jobs.
  createEffect(() => {
    const jobId = store.job?.id;

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
    if (jobId && window.userToken && !store.job?.localPreview) {
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
        .receive("error", (e) => { if (import.meta.env.DEV) console.error("Join failed", e); });

      channel.on("job_updated", (payload) => {
        // Merge to preserve fields the server doesn't resend on updates.
        setStore("job", (prev) => ({ ...prev, ...payload.job }));
      });
    }
  });

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
    if (previewAbortController) {
      previewAbortController.abort();
      previewAbortController = null;
    }
    revokePreviewBlobUrl();

    const isGuest = window.isGuest;

    setStore({
      file,
      filename: file.name,
      uploadState: "uploading",
      uploadProgress: 0,
      estimatedSeconds: null,
      previewClip: null,
    });

    try {
      let fileToUpload = file;
      let estimatedSeconds = null;
      let previewClip = null;

      if (isGuest) {
        const trimmed = await trimAudioFileForPreview(file);
        fileToUpload = trimmed.file;
        estimatedSeconds = trimmed.clippedSeconds;
        previewClip = {
          originalFilename: trimmed.originalFilename,
          originalSeconds: trimmed.originalSeconds,
          clippedSeconds: trimmed.clippedSeconds,
          wasTrimmed: trimmed.wasTrimmed,
        };

        setStore({
          file: fileToUpload,
          filename: fileToUpload.name,
          estimatedSeconds,
          previewClip,
          s3Key: null,
          uploadState: "ready",
        });
      } else {
        const { url, key } = await api.presignUpload(fileToUpload.name);

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
          xhr.setRequestHeader("Content-Type", fileToUpload.type || "application/octet-stream");
          xhr.send(fileToUpload);
        });

        setStore({ s3Key: key, uploadState: "ready" });
      }

      // Detect audio duration in background (don't block upload completion)
      if (estimatedSeconds !== null) {
        setStore("estimatedSeconds", estimatedSeconds);
      } else {
        getAudioDurationSeconds(file).then((seconds) => {
          if (seconds !== null) {
            setStore("estimatedSeconds", seconds);
          }
        });
      }
    } catch (err) {
      // Don't show error for aborted uploads
      if (err.message !== "Upload cancelled") {
        setStore({ uploadState: "error" });
        notify({ type: "error", message: err.message });
      }
    }
  }

  async function submitJob() {
    if (!store.filename || !store.file || (!window.isGuest && !store.s3Key)) {
      notify({ type: "error", message: "Please upload a file first" });
      return;
    }

    // Prevent double-submit
    if (store.submitting) return;
    setStore("submitting", true);

    try {
      if (window.isGuest) {
        previewAbortController = new AbortController();
        setStore({
          job: {
            id: "preview",
            localPreview: true,
            status: "processing",
            filename: store.filename,
            progress: {
              stage: "denoise",
              percent_complete: 50,
            },
          },
        });

        const previewBlob = await api.createPreview(store.file, previewAbortController.signal);
        previewAbortController = null;
        revokePreviewBlobUrl();
        previewBlobUrl = URL.createObjectURL(previewBlob);

        setStore({
          submitting: false,
          job: {
            id: "preview",
            localPreview: true,
            status: "completed",
            filename: store.filename,
            download_url: previewBlobUrl,
            progress: {
              stage: "completed",
              percent_complete: 100,
            },
          },
        });
      } else {
        const config = {
          duration_seconds: store.estimatedSeconds || 60,
        };
        const job = await api.createJob(store.s3Key, store.filename, config);
        setStore({ job });
      }
    } catch (err) {
      if (err.name === "AbortError") {
        setStore("submitting", false);
        return;
      }

      // Billing errors get persistent notification with upgrade action
      if (err instanceof ApiError && err.code === "insufficient_seconds") {
        const details = err.details;
        let message = getErrorMessage(err.code);
        if (details?.seconds_available !== undefined && details?.seconds_needed !== undefined) {
          const formatTime = (secs) => {
            const m = Math.floor(secs / 60);
            const s = secs % 60;
            return `${m}m ${s}s`;
          };
          message += ` You need ${formatTime(details.seconds_needed)} but only have ${formatTime(details.seconds_available)} available.`;
        }
        notify({
          type: "error",
          message,
          persistent: true,
          action: { label: "See options", onClick: () => window.location.href = "/account" }
        });
      } else {
        notify({ type: "error", message: err.message, persistent: true });
      }
      // Reset submitting on error so user can retry
      setStore({
        submitting: false,
        job: null,
      });
    }
  }

  async function cancelJob() {
    if (store.job?.localPreview && previewAbortController) {
      previewAbortController.abort();
      previewAbortController = null;
    } else if (store.job?.id) {
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
    revokePreviewBlobUrl();
    if (previewAbortController) {
      previewAbortController.abort();
      previewAbortController = null;
    }

    if (store.job?.id && !store.job.localPreview) {
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
      previewClip: null,
      submitting: false,
      job: null,
    });
  }

  const value = {
    store,
    uploadFile,
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
