import { Show, Switch, Match } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { UploadZone } from "./UploadZone";
import { PresetSelector } from "./PresetSelector";
import { JobProgress } from "./JobProgress";
import { JobComplete } from "./JobComplete";
import { JobFailed } from "./JobFailed";
import { ErrorAlert } from "./ErrorAlert";
import { StepsIndicator, getCurrentStep } from "./StepsIndicator";

export function ProcessPage() {
  const { store, submitJob } = useProcess();

  return (
    <div class="min-h-[calc(100vh-4rem)] flex flex-col items-center justify-center p-4">
      <div class="card bg-base-200 w-full max-w-lg border border-base-300 rounded-3xl">
        <div class="card-body">
          <Show when={!store.initializing} fallback={
            <div class="flex flex-col items-center justify-center py-12">
              <span class="loading loading-spinner loading-lg text-primary" />
              <p class="text-base-content/60 mt-4">Loading...</p>
            </div>
          }>
            <ErrorAlert message={store.error} />

            <Show when={store.job} fallback={
              <div class="space-y-6">
                <UploadZone />
                <PresetSelector />
                <button
                  type="button"
                  onClick={submitJob}
                  disabled={store.uploadState !== "ready"}
                  class="btn btn-primary w-full btn-lg"
                >
                  MUNCH IT!
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
