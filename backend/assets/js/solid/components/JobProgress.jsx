import { Show } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { getFriendlyStage } from "../utils/stages";

export function JobProgress() {
  const { store, cancelJob } = useProcess();

  return (
    <Show when={store.job}>
      <div class="flex flex-col items-center py-8">
        <div
          class="radial-progress text-primary text-2xl font-bold"
          style={{
            "--value": store.job?.progress?.percent_complete || 0,
            "--size": "10rem",
            "--thickness": "0.5rem",
          }}
          role="progressbar"
        >
          {store.job?.progress?.percent_complete || 0}%
        </div>

        <p class="mt-6 text-xl font-bold">
          <span class="inline-block animate-munch">🐄</span> *munch munch munch*
        </p>
        <p class="text-base-content/60 mt-2">{getFriendlyStage(store.job?.progress?.stage)}</p>
        <p class="text-base-content/40 text-sm mt-1 truncate max-w-full">{store.job?.filename}</p>

        <button onClick={cancelJob} class="btn btn-ghost btn-sm mt-6 text-error">
          Cancel
        </button>
      </div>
    </Show>
  );
}
