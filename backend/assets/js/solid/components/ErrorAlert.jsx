import { Show, createMemo } from "solid-js";

export function ErrorAlert(props) {
  // Handle both string and object errors
  const errorMessage = createMemo(() => {
    if (!props.message) return null;
    if (typeof props.message === "string") return props.message;
    return props.message.message;
  });

  const isBillingError = createMemo(() => {
    if (!props.message || typeof props.message === "string") return false;
    return props.message.code === "insufficient_minutes";
  });

  const minutesInfo = createMemo(() => {
    if (!isBillingError()) return null;
    const details = props.message.details;
    if (details?.minutes_available !== undefined && details?.minutes_needed !== undefined) {
      return `You need ${details.minutes_needed} minutes but only have ${details.minutes_available} available.`;
    }
    return null;
  });

  return (
    <Show when={errorMessage()}>
      <div role="alert" class="alert alert-error mb-4">
        <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6 shrink-0 stroke-current" fill="none" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <div class="flex-1">
          <span>{errorMessage()}</span>
          <Show when={minutesInfo()}>
            <p class="text-sm opacity-80 mt-1">{minutesInfo()}</p>
          </Show>
        </div>
        <Show when={isBillingError()}>
          <a href="/account" class="btn btn-sm btn-outline border-error-content text-error-content hover:bg-error-content hover:text-error">
            Upgrade
          </a>
        </Show>
      </div>
    </Show>
  );
}
