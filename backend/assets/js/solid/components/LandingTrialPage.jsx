import { Show, Switch, Match } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { UploadZone } from "./UploadZone";
import { JobProgress } from "./JobProgress";
import { JobComplete } from "./JobComplete";
import { JobFailed } from "./JobFailed";
import { GuestPrompt } from "./GuestPrompt";

export function LandingTrialPage() {
  const { store, submitJob, dismissGuestPrompt } = useProcess();

  return (
    <>
      <div class="panel rounded-3xl border-base-300/70 p-6">
        <Show
          when={window.isGuest}
          fallback={
            <div class="space-y-5">
              <div>
                <p class="text-xs font-semibold uppercase tracking-[0.22em] text-primary/75">
                  Welcome back
                </p>
                <h2 class="mt-3 text-2xl font-bold">
                  Jump straight into your full-length recordings.
                </h2>
                <p class="mt-3 text-sm leading-6 text-base-content/65">
                  The preview here is for first-time visitors. Your uploads and history live inside the app.
                </p>
              </div>
              <div class="flex flex-wrap gap-3">
                <a href="/app" class="btn btn-primary">Open app</a>
                <a href="/app/past-munchings" class="btn btn-ghost">Past Munchings</a>
              </div>
            </div>
          }
        >
          <Show
            when={!store.initializing}
            fallback={
              <div class="flex flex-col items-center justify-center py-10">
                <span class="loading loading-spinner loading-lg text-primary" />
                <p class="mt-4 text-sm text-base-content/60">Loading preview tools...</p>
              </div>
            }
          >
            <Show
              when={store.job}
              fallback={
                <div class="space-y-5">
                  <p class="text-xs font-semibold uppercase tracking-[0.22em] text-primary/75">
                    Free 20-second preview
                  </p>

                  <UploadZone />

                  <button
                    type="button"
                    onClick={submitJob}
                    disabled={store.uploadState !== "ready" || store.submitting}
                    class="btn btn-primary w-full"
                  >
                    <Show when={store.submitting} fallback={"Hear the preview"}>
                      <span class="loading loading-spinner loading-sm"></span>
                      Queueing...
                    </Show>
                  </button>

                  <p class="text-xs leading-5 text-base-content/50">
                    Works best on interviews, podcasts, voice notes, and voiceovers.
                  </p>
                </div>
              }
            >
              <Switch fallback={<JobProgress />}>
                <Match when={store.job?.status === "completed"}>
                  <JobComplete />
                </Match>
                <Match when={store.job?.status === "failed"}>
                  <JobFailed />
                </Match>
              </Switch>
            </Show>
          </Show>
        </Show>
      </div>

      <Show when={window.isGuest && store.guestPrompt}>
        <GuestPrompt prompt={store.guestPrompt} onClose={dismissGuestPrompt} />
      </Show>
    </>
  );
}
