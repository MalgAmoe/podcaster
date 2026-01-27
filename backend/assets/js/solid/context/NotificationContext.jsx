import { createContext, useContext, onCleanup } from "solid-js";
import { createStore } from "solid-js/store";

const NotificationContext = createContext();

// Auto-dismiss timeout in ms
const AUTO_DISMISS_TIMEOUT = 5000;

export function NotificationProvider(props) {
  const [notifications, setNotifications] = createStore([]);

  // Component-scoped state to avoid conflicts on remount
  let notificationId = 0;
  const timeoutIds = new Map();

  // Cleanup all timeouts when provider unmounts
  onCleanup(() => {
    for (const timeoutId of timeoutIds.values()) {
      clearTimeout(timeoutId);
    }
    timeoutIds.clear();
  });

  function notify({
    type = "info", // 'error' | 'success' | 'warning' | 'info'
    message,
    persistent = false, // default: auto-dismiss after 5s
    action = null, // { label: string, onClick: () => void }
  }) {
    const id = ++notificationId;

    setNotifications((prev) => [
      ...prev,
      { id, type, message, persistent, action },
    ]);

    // Auto-dismiss non-persistent notifications
    if (!persistent) {
      const timeoutId = setTimeout(() => {
        timeoutIds.delete(id);
        dismiss(id);
      }, AUTO_DISMISS_TIMEOUT);
      timeoutIds.set(id, timeoutId);
    }

    return id;
  }

  function dismiss(id) {
    // Clear timeout if notification is dismissed early
    const timeoutId = timeoutIds.get(id);
    if (timeoutId) {
      clearTimeout(timeoutId);
      timeoutIds.delete(id);
    }
    setNotifications((prev) => prev.filter((n) => n.id !== id));
  }

  function dismissAll() {
    // Clear all pending timeouts
    for (const timeoutId of timeoutIds.values()) {
      clearTimeout(timeoutId);
    }
    timeoutIds.clear();
    setNotifications([]);
  }

  const value = {
    notifications,
    notify,
    dismiss,
    dismissAll,
  };

  return (
    <NotificationContext.Provider value={value}>
      {props.children}
    </NotificationContext.Provider>
  );
}

export function useNotifications() {
  const ctx = useContext(NotificationContext);
  if (!ctx) {
    throw new Error("useNotifications must be used within NotificationProvider");
  }
  return ctx;
}
