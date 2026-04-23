import { Show } from "solid-js";

export function GuestPrompt(props) {
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
