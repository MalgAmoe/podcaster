import { createSignal, createResource, Show, For } from "solid-js";
import { api } from "../utils/api";

export function JobHistory() {
  const [isOpen, setIsOpen] = createSignal(false);
  const [jobs, { refetch }] = createResource(
    () => isOpen(),
    async (open) => {
      if (!open) return [];
      const data = await api.getJobHistory();
      return data.jobs || [];
    }
  );

  function formatDate(dateString) {
    const date = new Date(dateString);
    const now = new Date();
    const diffMs = now - date;
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    if (diffDays === 0) {
      return "Today";
    } else if (diffDays === 1) {
      return "Yesterday";
    } else if (diffDays < 7) {
      return `${diffDays} days ago`;
    } else {
      return date.toLocaleDateString();
    }
  }

  function handleDownload(url, filename) {
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  }

  function toggleOpen() {
    const willOpen = !isOpen();
    setIsOpen(willOpen);
    if (willOpen) {
      refetch();
    }
  }

  return (
    <div class="w-full max-w-lg mt-4">
      <div class={`bg-base-200 border border-base-300 ${isOpen() ? "rounded-2xl" : "rounded-xl"}`}>
        <button
          type="button"
          onClick={toggleOpen}
          class="w-full flex items-center justify-between px-4 py-3 hover:bg-base-300/50 rounded-xl transition-colors"
        >
          <span class="font-medium text-base-content/80">Past Munchings</span>
          <svg
            class={`w-5 h-5 text-base-content/60 transition-transform ${isOpen() ? "rotate-180" : ""}`}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
          </svg>
        </button>

        <Show when={isOpen()}>
          <Show when={jobs.loading}>
            <div class="flex items-center justify-center py-8 border-t border-base-300">
              <span class="loading loading-spinner loading-sm text-primary"></span>
            </div>
          </Show>

          <Show when={!jobs.loading && jobs()?.length === 0}>
            <div class="text-center py-8 px-4 border-t border-base-300">
              <p class="text-base-content/60 text-sm">
                No leftovers yet. Your munched files stick around for 7 days.
              </p>
            </div>
          </Show>

          <Show when={!jobs.loading && jobs()?.length > 0}>
            <ul class="border-t border-base-300 divide-y divide-base-300">
              <For each={jobs()}>
                {(job) => (
                  <li class="flex items-center justify-between px-4 py-3 hover:bg-base-300/50 transition-colors">
                    <div class="flex-1 min-w-0 mr-3">
                      <p class="text-sm font-medium truncate" title={job.filename}>
                        {job.filename}
                      </p>
                      <p class="text-xs text-base-content/50">
                        {formatDate(job.created_at)}
                      </p>
                    </div>
                    <button
                      type="button"
                      onClick={() => handleDownload(job.download_url, job.filename)}
                      class="btn btn-sm btn-ghost btn-square"
                      title="Download"
                      aria-label={`Download ${job.filename}`}
                    >
                      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"
                        />
                      </svg>
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </Show>
        </Show>
      </div>
    </div>
  );
}
