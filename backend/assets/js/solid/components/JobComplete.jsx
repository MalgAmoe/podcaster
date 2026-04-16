import { Show, createSignal, createMemo, onCleanup } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { WaveformPlayer } from "./WaveformPlayer";

export function JobComplete() {
  const { store, reset } = useProcess();
  const [activeTrack, setActiveTrack] = createSignal("processed");
  const [isPlaying, setIsPlaying] = createSignal(false);
  const [currentTime, setCurrentTime] = createSignal(0);

  let currentBlobUrl = null;

  const originalUrl = createMemo(() => {
    if (currentBlobUrl) {
      URL.revokeObjectURL(currentBlobUrl);
      currentBlobUrl = null;
    }
    if (store.file) {
      currentBlobUrl = URL.createObjectURL(store.file);
      return currentBlobUrl;
    }
    return store.job?.original_url || null;
  });

  onCleanup(() => {
    if (currentBlobUrl) {
      URL.revokeObjectURL(currentBlobUrl);
    }
  });

  const currentUrl = createMemo(() => {
    return activeTrack() === "original" ? originalUrl() : store.job?.download_url;
  });

  function isValidDownloadUrl(url) {
    try {
      const parsed = new URL(url, window.location.origin);
      return (
        parsed.origin === window.location.origin ||
        parsed.hostname === "localhost" ||
        parsed.hostname === "127.0.0.1" ||
        parsed.hostname.endsWith(".amazonaws.com") ||
        parsed.hostname.endsWith(".r2.cloudflarestorage.com") ||
        parsed.hostname.endsWith(".digitaloceanspaces.com")
      );
    } catch {
      return false;
    }
  }

  function handleDownload() {
    const url = store.job?.download_url;
    if (url && isValidDownloadUrl(url)) {
      window.location.href = url;
    } else if (url) {
      console.error("Invalid download URL origin:", url);
    }
  }

  return (
    <Show when={store.job}>
      <div class="flex flex-col items-center py-6">
        <div class="w-16 h-16 flex items-center justify-center mb-4">
          <img src="/images/munchy_cow_head.svg" alt="" class="w-16 h-16 animate-bounce-soft" />
        </div>

        <h3 class="text-xl font-bold">MOOO!</h3>
        <p class="text-base-content/60 text-sm mt-1">Your audio is ready!</p>
        <p class="text-base-content/40 text-xs mt-1 truncate max-w-full">{store.job?.filename}</p>

        <Show when={store.job?.download_url}>
          <div class="w-full mt-6 p-4 bg-base-300/70 rounded-2xl space-y-4">
            {/* A/B Toggle */}
            <div class="flex justify-center" role="group" aria-label="Audio comparison">
              <div class="inline-flex gap-1 bg-base-100 rounded-full p-1">
                <button
                  type="button"
                  class={`btn btn-toggle btn-sm px-5 rounded-full border-0 ${activeTrack() === "original" ? "bg-base-300 text-base-content shadow-sm" : "btn-ghost text-base-content/50"}`}
                  onClick={() => setActiveTrack("original")}
                  aria-pressed={activeTrack() === "original"}
                >
                  Original
                </button>
                <button
                  type="button"
                  class={`btn btn-toggle btn-sm px-5 rounded-full border-0 ${activeTrack() === "processed" ? "bg-primary text-primary-content shadow-sm" : "btn-ghost text-base-content/50"}`}
                  onClick={() => setActiveTrack("processed")}
                  aria-pressed={activeTrack() === "processed"}
                >
                  Processed
                </button>
              </div>
            </div>

            <Show when={currentUrl()}>
              <WaveformPlayer
                audioUrl={currentUrl()}
                cacheKey={`${store.job.id}-${activeTrack()}`}
                currentTime={currentTime()}
                isPlaying={isPlaying()}
                onTimeUpdate={setCurrentTime}
                onPlayingChange={setIsPlaying}
              />
            </Show>
          </div>
        </Show>

        <div class="flex gap-3 mt-6">
          <button onClick={handleDownload} class="btn btn-primary gap-2" aria-label="Download processed audio">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                    d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
            </svg>
            Download
          </button>
          <button onClick={reset} class="btn btn-ghost" aria-label="Upload another file">
            Feed me more!
          </button>
        </div>
      </div>
    </Show>
  );
}
