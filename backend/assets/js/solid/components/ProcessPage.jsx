import { Show, Switch, Match } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { UploadZone } from "./UploadZone";
import { ProcessingConfig } from "./ProcessingConfig";
import { JobProgress } from "./JobProgress";
import { JobComplete } from "./JobComplete";
import { JobFailed } from "./JobFailed";
import { StepsIndicator, getCurrentStep } from "./StepsIndicator";
import { GuestPrompt } from "./GuestPrompt";

export function ProcessPage() {
  const { store, submitJob, dismissGuestPrompt } = useProcess();

  return (
    <div class="min-h-[calc(100vh-4rem)] flex flex-col items-center justify-center p-4">
      <Show when={!window.isGuest}>
        <div class="w-full max-w-lg text-center mb-4">
          <a href="/feedback" rel="external" class="text-sm text-base-content/40 hover:text-primary transition-colors">
            Give feedback
          </a>
        </div>
      </Show>
      <div class="card panel w-full max-w-lg rounded-3xl border-base-300/60">
        <div class="card-body">
          <Show when={!store.initializing} fallback={
            <div class="flex flex-col items-center justify-center py-12">
              <span class="loading loading-spinner loading-lg text-primary" />
              <p class="text-base-content/60 mt-4">Loading...</p>
            </div>
          }>
            <Show when={store.job} fallback={
              <div class="space-y-6">
                <UploadZone />
                <ProcessingConfig />
                <button
                  type="button"
                  onClick={submitJob}
                  disabled={store.uploadState !== "ready" || store.submitting}
                  class="btn btn-primary w-full btn-lg"
                >
                  <Show when={store.submitting} fallback={"MUNCH IT!"}>
                    <span class="loading loading-spinner loading-sm"></span>
                    Queueing...
                  </Show>
                </button>
              </div>
            }>
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
        </div>
      </div>

      <Show when={!store.initializing}>
        <StepsIndicator currentStep={getCurrentStep(store.job)} />
      </Show>

      <Show when={window.isGuest && store.guestPrompt}>
        <GuestPrompt prompt={store.guestPrompt} onClose={dismissGuestPrompt} />
      </Show>
    </div>
  );
}
