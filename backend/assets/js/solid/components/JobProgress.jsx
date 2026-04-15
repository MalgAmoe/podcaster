import { Show } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { useI18n } from "../context/I18nContext";
import { getStageKey } from "../utils/stages";

export function JobProgress() {
  const { store, cancelJob } = useProcess();
  const { t } = useI18n();

  return (
    <Show when={store.job}>
      <div class="flex flex-col items-center py-8">
        <div class="relative">
          <div
            class="radial-progress text-primary"
            style={{
              "--value": store.job?.progress?.percent_complete || 0,
              "--size": "10rem",
              "--thickness": "0.5rem",
            }}
            role="progressbar"
          >
            <img src="/images/munchy_cow_head.svg" alt={t("munching")} class="w-16 h-16 animate-munch" />
          </div>
          <div class="absolute inset-0 rounded-full bg-primary/5 animate-pulse-slow pointer-events-none" />
        </div>

        <p class="mt-6 text-xl font-bold">
          {t("munchMunchMunch")}
        </p>
        <p class="text-base-content/60 mt-2">{t(getStageKey(store.job?.progress?.stage))}</p>
        <p class="text-base-content/40 text-sm mt-1 truncate max-w-full">{store.job?.filename}</p>

        <button onClick={cancelJob} class="btn btn-ghost btn-sm mt-6 text-error">
          {t("cancel")}
        </button>
      </div>
    </Show>
  );
}
