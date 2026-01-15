import { Show } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { getFriendlyJobError } from "../utils/errors";

export function JobFailed() {
  const { store, reset } = useProcess();

  return (
    <Show when={store.job}>
      <div class="flex flex-col items-center py-8">
        <div class="w-20 h-20 rounded-full bg-error/10 flex items-center justify-center mb-6">
          <svg class="w-10 h-10 text-error" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </div>

        <h3 class="text-2xl font-bold text-error">The cow choked!</h3>
        <p class="text-base-content/60 mt-2 text-center px-4">
          {getFriendlyJobError(store.job?.error)}
        </p>

        <button onClick={reset} class="btn btn-primary mt-8">
          Feed her again
        </button>
      </div>
    </Show>
  );
}
