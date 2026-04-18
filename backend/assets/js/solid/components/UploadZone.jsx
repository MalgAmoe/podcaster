import { createSignal, Show } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { useNotifications } from "../context/NotificationContext";

export function UploadZone() {
  const { store, uploadFile, reset } = useProcess();
  const { notify } = useNotifications();
  let fileInput;
  const [isDragging, setIsDragging] = createSignal(false);

  const MAX_FILE_SIZE = 2 * 1024 * 1024 * 1024; // 2GB

  const isGuest = window.isGuest;
  const guestExtensions = [".wav", ".mp3"];
  const userExtensions = [".wav", ".mp3", ".flac", ".m4a", ".aac", ".ogg"];
  const validExtensions = isGuest ? guestExtensions : userExtensions;
  const acceptAttr = isGuest
    ? "audio/wav,audio/mpeg,.wav,.mp3"
    : "audio/*,.flac,.wav,.mp3,.m4a,.aac,.ogg";
  const formatHint = isGuest ? "WAV, MP3" : "WAV, MP3, FLAC";
  const invalidFileMessage = isGuest
    ? "Please select a WAV or MP3 file."
    : "Please select an audio file (WAV, MP3, FLAC, etc.)";

  function formatSeconds(totalSeconds) {
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}m ${seconds}s`;
  }

  async function handleFile(file) {
    if (!file) return;

    const ext = file.name.toLowerCase().slice(file.name.lastIndexOf("."));
    const isAudio = validExtensions.includes(ext) || (!isGuest && file.type.startsWith("audio/"));

    if (!isAudio) {
      notify({ type: "error", message: invalidFileMessage });
      return;
    }

    if (file.size > MAX_FILE_SIZE) {
      notify({ type: "error", message: "File too large. Maximum size is 2GB." });
      return;
    }

    await uploadFile(file);
  }

  function handleDrop(e) {
    e.preventDefault();
    setIsDragging(false);
    handleFile(e.dataTransfer.files[0]);
  }

  function handleDragOver(e) {
    e.preventDefault();
    setIsDragging(true);
  }

  function handleDragLeave(e) {
    e.preventDefault();
    setIsDragging(false);
  }

  return (
    <Show
      when={store.uploadState === "uploading" || store.uploadState === "ready"}
      fallback={
        <>
          <input
            ref={el => fileInput = el}
            type="file"
            accept={acceptAttr}
            onChange={(e) => handleFile(e.target.files[0])}
            class="hidden"
            id="audio-file-input"
          />
          <div
            role="button"
            aria-label="Upload audio file. Click or drop a file here."
            tabIndex="0"
            onClick={() => fileInput.click()}
            onKeyDown={(e) => e.key === "Enter" && fileInput.click()}
            onDrop={handleDrop}
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            class={`border-2 border-dashed rounded-3xl p-12 text-center
                   hover:border-primary/60 hover:bg-primary/5 hover:shadow-[inset_0_0_30px_rgba(147,51,234,0.06)] transition-all duration-300
                   cursor-pointer group ${isDragging() ? "border-primary bg-primary/5 shadow-[inset_0_0_30px_rgba(147,51,234,0.06)]" : "border-base-content/15"}`}
          >
            <div class="flex flex-col items-center gap-4">
              <div class="w-20 h-20 flex items-center justify-center group-hover:scale-110 transition-transform">
                <img src="/images/munchy_cow.svg" alt="Munchy Cow" class="w-full h-full" />
              </div>
              <div>
                <p class="font-medium text-base-content text-lg">Feed the cow!</p>
              </div>
              <p class="text-xs text-base-content/40">
                nom nom nom - {formatHint}
                <Show when={isGuest}>
                  {" "}·{" "}
                  <a
                    href="/users/log-in"
                    class="link link-hover text-primary/70"
                    onClick={(e) => e.stopPropagation()}
                    onKeyDown={(e) => e.stopPropagation()}
                  >Sign in</a>
                  {" "}for more formats
                </Show>
              </p>
            </div>
          </div>
        </>
      }
    >
      <div class="flex items-center gap-4 p-4 bg-base-300 rounded-2xl">
        <div class="w-12 h-12 flex items-center justify-center shrink-0">
          <Show
            when={store.uploadState === "ready"}
            fallback={<span class="loading loading-spinner loading-sm text-primary"></span>}
          >
            <img src="/images/munchy_cow_head.svg" alt="Munchy Cow" class="w-full h-full" />
          </Show>
        </div>
        <div class="flex-1 min-w-0">
          <p class="font-medium truncate">{store.filename}</p>
          <Show when={store.uploadState === "ready"}>
            <p class="text-sm text-primary">Ready to munch!</p>
            <Show when={isGuest && store.previewClip}>
              <p class="text-xs text-base-content/50 mt-0.5">
                <Show
                  when={store.previewClip.wasTrimmed}
                  fallback={`Preview clip ready: ${formatSeconds(store.previewClip.clippedSeconds)}`}
                >
                  Preview clipped from {formatSeconds(store.previewClip.originalSeconds)} to {formatSeconds(store.previewClip.clippedSeconds)}
                </Show>
              </p>
            </Show>
            <Show when={store.estimatedSeconds}>
              <p class="text-xs text-base-content/50">~{Math.floor(store.estimatedSeconds / 60)}m {store.estimatedSeconds % 60}s</p>
              <Show when={!window.isGuest && window.userTotalSeconds !== undefined && store.estimatedSeconds > window.userTotalSeconds}>
                <p class="text-xs text-warning mt-0.5">This file is {Math.floor(store.estimatedSeconds / 60)}m {store.estimatedSeconds % 60}s but you only have {Math.floor(window.userTotalSeconds / 60)}m {window.userTotalSeconds % 60}s available.</p>
              </Show>
            </Show>
          </Show>
          <Show when={store.uploadState === "uploading" && isGuest && store.uploadProgress === 0}>
            <p class="text-sm text-base-content/60">Preparing 30s preview clip...</p>
          </Show>
          <Show when={store.uploadState === "uploading" && store.uploadProgress >= 100}>
            <p class="text-sm text-base-content/60">Finalizing...</p>
          </Show>
          <Show when={store.uploadState === "uploading" && store.uploadProgress < 100}>
            <div class="flex items-center gap-2 mt-1">
              <progress class="progress progress-primary flex-1 h-2" value={store.uploadProgress} max="100" />
              <span class="text-xs text-base-content/60 w-8">{store.uploadProgress}%</span>
            </div>
          </Show>
        </div>
        <button type="button" onClick={reset} class="btn btn-ghost btn-sm btn-circle" aria-label="Remove file">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    </Show>
  );
}
