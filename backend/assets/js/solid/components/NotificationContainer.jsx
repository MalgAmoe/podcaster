import { For, Show } from "solid-js";
import { Portal } from "solid-js/web";
import { useNotifications } from "../context/NotificationContext";

// Icon components for each notification type
function ErrorIcon() {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" />
    </svg>
  );
}

function SuccessIcon() {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
    </svg>
  );
}

function WarningIcon() {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
    </svg>
  );
}

function InfoIcon() {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
    </svg>
  );
}

function getIcon(type) {
  switch (type) {
    case "error": return <ErrorIcon />;
    case "success": return <SuccessIcon />;
    case "warning": return <WarningIcon />;
    default: return <InfoIcon />;
  }
}

function getAlertClass(type) {
  switch (type) {
    case "error": return "alert-error";
    case "success": return "alert-success";
    case "warning": return "alert-warning";
    default: return "alert-info";
  }
}

function NotificationItem(props) {
  const { dismiss } = useNotifications();

  return (
    <div
      role="alert"
      class={`alert ${getAlertClass(props.notification.type)} shadow-lg animate-slide-in`}
    >
      {getIcon(props.notification.type)}
      <span class="flex-1">{props.notification.message}</span>

      <div class="flex gap-2">
        <Show when={props.notification.action}>
          <button
            type="button"
            class="btn btn-sm btn-outline"
            onClick={() => {
              props.notification.action.onClick();
              dismiss(props.notification.id);
            }}
          >
            {props.notification.action.label}
          </button>
        </Show>

        <Show when={props.notification.persistent}>
          <button
            type="button"
            class="btn btn-sm btn-ghost btn-circle"
            onClick={() => dismiss(props.notification.id)}
            aria-label="Dismiss notification"
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </Show>
      </div>
    </div>
  );
}

export function NotificationContainer() {
  const { notifications } = useNotifications();

  return (
    <Portal>
      <div
        class="fixed top-20 right-4 z-50 flex flex-col gap-2 max-w-md w-full pointer-events-none"
        aria-live="polite"
        aria-label="Notifications"
      >
        <For each={notifications}>
          {(notification) => (
            <div class="pointer-events-auto">
              <NotificationItem notification={notification} />
            </div>
          )}
        </For>
      </div>
    </Portal>
  );
}
