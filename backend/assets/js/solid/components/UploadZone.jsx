import { createSignal, Show } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { useNotifications } from "../context/NotificationContext";

export function UploadZone() {
  const { store, uploadFile, reset } = useProcess();
  const { notify } = useNotifications();
  let fileInput;
  const [isDragging, setIsDragging] = createSignal(false);

  const MAX_FILE_SIZE = 500 * 1024 * 1024; // 500MB

  async function handleFile(file) {
    if (!file) return;

    const validExtensions = [".wav", ".mp3", ".flac", ".m4a", ".aac", ".ogg", ".opus"];
    const ext = file.name.toLowerCase().slice(file.name.lastIndexOf("."));
    const isAudio = file.type.startsWith("audio/") || validExtensions.includes(ext);

    if (!isAudio) {
      notify({
        type: "error",
        message: "Please select an audio file (WAV, MP3, FLAC, etc.)"
      });
      return;
    }

    if (file.size > MAX_FILE_SIZE) {
      notify({
        type: "error",
        message: "File too large. Maximum size is 500MB."
      });
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
            accept="audio/*,.flac,.wav,.mp3,.m4a,.aac,.ogg,.opus"
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
                   hover:border-primary hover:bg-primary/5 transition-all duration-200
                   cursor-pointer group ${isDragging() ? "border-primary bg-primary/5" : "border-base-300"}`}
          >
            <div class="flex flex-col items-center gap-4">
              <div class="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center group-hover:bg-primary/20 transition-colors">
                <svg class="w-8 h-8 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                        d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                </svg>
              </div>
              <div>
                <p class="font-medium text-base-content text-lg">Feed the cow!</p>
                <p class="text-sm text-base-content/60 mt-1">She's VERY hungry for your audio</p>
              </div>
              <p class="text-xs text-base-content/40">nom nom nom - WAV, MP3, FLAC</p>
            </div>
          </div>
        </>
      }
    >
      <div class="flex items-center gap-4 p-4 bg-base-300 rounded-2xl">
        <div class="w-12 h-12 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
          <Show
            when={store.uploadState === "ready"}
            fallback={<span class="loading loading-spinner loading-sm text-primary"></span>}
          >
            <svg class="w-6 h-6 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                    d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3" />
            </svg>
          </Show>
        </div>
        <div class="flex-1 min-w-0">
          <p class="font-medium truncate">{store.filename}</p>
          <Show when={store.uploadState === "ready"}>
            <p class="text-sm text-primary">Ready to munch!</p>
            <Show when={store.estimatedMinutes}>
              <p class="text-xs text-base-content/50">~{store.estimatedMinutes} min</p>
            </Show>
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
