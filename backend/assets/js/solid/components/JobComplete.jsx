import { Show } from "solid-js";
import { useProcess } from "../context/ProcessContext";

export function JobComplete() {
  const { store, reset } = useProcess();

  function handleDownload() {
    if (store.job?.download_url) {
      window.location.href = store.job.download_url;
    }
  }

  return (
    <Show when={store.job}>
      <div class="flex flex-col items-center py-6">
        <div class="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center mb-4">
          <svg class="w-8 h-8 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
          </svg>
        </div>

        <h3 class="text-xl font-bold">
          MOOO! <span class="inline-block animate-bounce-soft">🐄</span>
        </h3>
        <p class="text-base-content/60 text-sm mt-1">Your audio is ready!</p>
        <p class="text-base-content/40 text-xs mt-1 truncate max-w-full">{store.job?.filename}</p>

        <div class="flex gap-3 mt-6">
          <button onClick={handleDownload} class="btn btn-primary gap-2">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                    d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
            </svg>
            Download
          </button>
          <button onClick={reset} class="btn btn-ghost">
            Feed me more!
          </button>
        </div>
      </div>
    </Show>
  );
}
