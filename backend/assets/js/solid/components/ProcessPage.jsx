import { Show, Switch, Match } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { UploadZone } from "./UploadZone";
import { ProcessingConfig } from "./ProcessingConfig";
import { JobProgress } from "./JobProgress";
import { JobComplete } from "./JobComplete";
import { JobFailed } from "./JobFailed";
import { StepsIndicator, getCurrentStep } from "./StepsIndicator";

export function ProcessPage() {
  const { store, submitJob } = useProcess();

  return (
    <div class="min-h-[calc(100vh-4rem)] flex flex-col items-center justify-center p-4">
      <Show when={!window.isGuest}>
        <div class="w-full max-w-lg text-center mb-4">
          <a href="/feedback" rel="external" class="text-sm text-base-content/40 hover:text-primary transition-colors">
            Give feedback
          </a>
        </div>
      </Show>
      <div class="card bg-base-200 w-full max-w-lg border border-base-300/60 rounded-3xl shadow-[0_0_60px_-15px_rgba(147,51,234,0.15)]">
        <div class="card-body">
          <Show when={!store.initializing} fallback={
            <div class="flex flex-col items-center justify-center py-12">
              <span class="loading loading-spinner loading-lg text-primary" />
              <p class="text-base-content/60 mt-4">Processing...</p>
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
                    Processing...
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
    </div>
  );
}
