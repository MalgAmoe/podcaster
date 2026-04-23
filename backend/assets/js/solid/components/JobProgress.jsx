import { Show } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { getStageLabel } from "../utils/stages";

export function JobProgress() {
  const { store, cancelJob } = useProcess();
  const previewStageLabel = () =>
    store.job?.preview_status === "queued" || store.job?.progress?.stage === "waiting"
      ? "Waiting for a demo slot..."
      : "Processing preview...";

  return (
    <Show when={store.job}>
      <div class="flex flex-col items-center py-8">
        <Show
          when={!store.job?.localPreview}
          fallback={
            <div class="relative w-24 h-24 rounded-full bg-primary/10 flex items-center justify-center">
              <img src="/images/munchy_cow_head.svg" alt="Munching..." class="w-16 h-16 animate-munch" />
              <div class="absolute inset-0 rounded-full bg-primary/5 animate-pulse-slow pointer-events-none" />
            </div>
          }
        >
          <div class="relative">
            <div
              class="radial-progress text-primary"
              style={{
                "--value": store.job?.progress?.percent_complete || 0,
                "--size": "10rem",
                "--thickness": "0.5rem",
              }}
              role="progressbar"
            >
              <img src="/images/munchy_cow_head.svg" alt="Munching..." class="w-16 h-16 animate-munch" />
            </div>
            <div class="absolute inset-0 rounded-full bg-primary/5 animate-pulse-slow pointer-events-none" />
          </div>
        </Show>

        <p class="mt-6 text-xl font-bold">
          *munch munch munch*
        </p>
        <p class="text-base-content/60 mt-2">
          <Show
            when={!store.job?.localPreview}
            fallback={previewStageLabel()}
          >
            {getStageLabel(store.job?.progress?.stage)}
          </Show>
        </p>
        <p class="text-base-content/40 text-sm mt-1 truncate max-w-full">{store.job?.filename}</p>

        <button onClick={cancelJob} class="btn btn-ghost btn-sm mt-6 text-error">
          Cancel
        </button>
      </div>
    </Show>
  );
}
