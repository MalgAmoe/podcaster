import { createSignal, createResource, Show, For } from "solid-js";
import dayjs from "dayjs";
import { api } from "../utils/api";

export function PastMunchingsPage() {
  const [jobs, { refetch }] = createResource(async () => {
    try {
      const data = await api.getJobHistory();
      return data.jobs || [];
    } catch (err) {
      if (import.meta.env.DEV) console.error("Failed to load job history:", err);
      return [];
    }
  });

  function formatDate(dateString) {
    const date = dayjs(dateString);
    const now = dayjs();
    const time = date.format("HH:mm");

    // Compare calendar days (start of day comparison)
    const diffDays = now.startOf("day").diff(date.startOf("day"), "day");

    if (diffDays === 0) {
      return `Today at ${time}`;
    } else if (diffDays === 1) {
      return `Yesterday at ${time}`;
    } else if (diffDays < 7) {
      return `${diffDays} days ago at ${time}`;
    } else {
      return `${date.format("MMM D, YYYY")} at ${time}`;
    }
  }

  const [downloading, setDownloading] = createSignal(null);

  async function handleDownload(jobId, filename) {
    if (downloading()) return;
    setDownloading(jobId);
    try {
      const response = await api.getDownloadUrl(jobId);
      if (response?.url) {
        window.location.href = response.url;
      } else {
        console.error("No URL in response:", response);
      }
    } catch (err) {
      console.error("Failed to get download URL:", err);
    } finally {
      setDownloading(null);
    }
  }

  return (
    <div class="flex flex-col items-center justify-start p-4 pt-8">
      <div class="w-full max-w-lg">
        <h1 class="text-2xl font-bold text-base-content mb-6">Past Munchings</h1>
        <p class="text-base-content/60 text-sm mb-6">
          Your processed files from the last 7 days.
        </p>

        <Show when={jobs.loading}>
          <div class="flex items-center justify-center py-12">
            <span class="loading loading-spinner loading-lg text-primary"></span>
          </div>
        </Show>

        <Show when={!jobs.loading && jobs()?.length === 0}>
          <div class="card bg-base-200 border border-base-300 rounded-2xl">
            <div class="card-body text-center py-12">
              <p class="text-base-content/60">
                No munchings yet. Your processed files will appear here for 7 days.
              </p>
            </div>
          </div>
        </Show>

        <Show when={!jobs.loading && jobs()?.length > 0}>
          <div class="card bg-base-200 border border-base-300 rounded-2xl overflow-hidden">
            <ul class="divide-y divide-base-300">
              <For each={jobs()}>
                {(job) => (
                  <li class="flex items-center justify-between px-4 py-4 hover:bg-base-300/50 transition-colors">
                    <div class="flex-1 min-w-0 mr-3">
                      <p class="font-medium truncate" title={job.filename}>
                        {job.filename}
                      </p>
                      <p class="text-sm text-base-content/50">
                        {formatDate(job.created_at)}
                      </p>
                    </div>
                    <button
                      type="button"
                      onClick={() => handleDownload(job.id, job.filename)}
                      class="btn btn-primary btn-sm gap-2"
                      classList={{ "loading": downloading() === job.id }}
                      disabled={downloading() !== null}
                    >
                      <Show when={downloading() !== job.id}>
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"
                          />
                        </svg>
                        Download
                      </Show>
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </div>
        </Show>
      </div>
    </div>
  );
}
