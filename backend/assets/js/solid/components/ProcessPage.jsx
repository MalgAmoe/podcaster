import { Show, Switch, Match } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { useI18n } from "../context/I18nContext";
import { UploadZone } from "./UploadZone";
import { ProcessingConfig } from "./ProcessingConfig";
import { JobProgress } from "./JobProgress";
import { JobComplete } from "./JobComplete";
import { JobFailed } from "./JobFailed";
import { StepsIndicator, getCurrentStep } from "./StepsIndicator";

function SignInCard() {
  const locale = document.documentElement.lang || "en";
  const loginPath = locale === "en" ? "/users/log-in" : `/${locale}/users/log-in`;

  return (
    <div class="flex flex-col items-center py-8">
      <div class="w-16 h-16 flex items-center justify-center mb-4">
        <img src="/images/munchy_cow_head.svg" alt="" class="w-16 h-16" />
      </div>
      <h3 class="text-xl font-bold mb-2">Enjoying the cow?</h3>
      <p class="text-base-content/60 text-center mb-6">
        Sign in to keep processing files and access your past munchings.
      </p>
      <a href={loginPath} rel="external" class="btn btn-primary btn-lg">
        Sign in
      </a>
      <p class="text-xs text-base-content/40 mt-3">Free. No credit card. Just your email.</p>
    </div>
  );
}

export function ProcessPage() {
  const { store, submitJob } = useProcess();
  const { t } = useI18n();

  // Check if guest limit reached (from server or client state)
  const guestBlocked = () =>
    store.guestLimitReached ||
    (window.isGuest && (window.completedJobsCount || 0) >= 3);

  return (
    <div class="min-h-[calc(100vh-4rem)] flex flex-col items-center justify-center p-4">
      <div class="card bg-base-200 w-full max-w-lg border border-base-300 rounded-3xl">
        <div class="card-body">
          <Show when={!store.initializing} fallback={
            <div class="flex flex-col items-center justify-center py-12">
              <span class="loading loading-spinner loading-lg text-primary" />
              <p class="text-base-content/60 mt-4">{t("processing")}</p>
            </div>
          }>
            <Show when={guestBlocked() && !store.job}>
              <SignInCard />
            </Show>
            <Show when={!guestBlocked() || store.job}>
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
                    <Show when={store.submitting} fallback={t("munchIt")}>
                      <span class="loading loading-spinner loading-sm"></span>
                      {t("processing")}
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
          </Show>
        </div>
      </div>

      <Show when={!store.initializing}>
        <StepsIndicator currentStep={getCurrentStep(store.job)} />
      </Show>
    </div>
  );
}
