import { Show, Switch, Match } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { UploadZone } from "./UploadZone";
import { ProcessingConfig } from "./ProcessingConfig";
import { JobProgress } from "./JobProgress";
import { JobComplete } from "./JobComplete";
import { JobFailed } from "./JobFailed";
import { StepsIndicator, getCurrentStep } from "./StepsIndicator";

function GuestPrompt(props) {
  const isBusy = () => props.prompt?.kind === "preview_busy";

  return (
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-base-100/65 backdrop-blur-sm">
      <div class="w-full max-w-md rounded-3xl border border-base-300/70 bg-base-200 shadow-[0_0_70px_-20px_rgba(147,51,234,0.28)]">
        <div class="p-6 sm:p-7 text-center">
          <div class="w-20 h-20 mx-auto flex items-center justify-center mb-5">
            <img src="/images/munchy_cow.svg" alt="Munchy Cow" class="w-full h-full" />
          </div>

          <h3 class="text-2xl font-bold">
            <Show when={isBusy()} fallback={"The cow needs a short breather"}>
              The demo is busy right now
            </Show>
          </h3>

          <p class="text-base-content/65 mt-3">
            <Show
              when={isBusy()}
              fallback={
                <>
                  You have been trying the preview a lot recently.
                  <Show when={props.prompt?.retryAfterText}>
                    {" "}Try again in {props.prompt.retryAfterText}.
                  </Show>
                </>
              }
            >
              Lots of people are trying the preview at the moment.
              <Show when={props.prompt?.retryAfterText}>
                {" "}Try again in {props.prompt.retryAfterText}.
              </Show>
            </Show>
          </p>

          <p class="text-sm text-base-content/45 mt-2">
            Sign in if you want the full processing flow instead of waiting on the demo.
          </p>

          <div class="flex flex-col sm:flex-row gap-3 mt-6">
            <button type="button" onClick={props.onClose} class="btn btn-ghost flex-1">
              Maybe later
            </button>
            <a href="/users/log-in" rel="external" class="btn btn-primary flex-1">
              Sign in
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}

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

      <Show when={window.isGuest && store.guestPrompt}>
        <GuestPrompt prompt={store.guestPrompt} onClose={dismissGuestPrompt} />
      </Show>
    </div>
  );
}
