import { createContext, useContext } from "solid-js";
import { createStore } from "solid-js/store";

const NotificationContext = createContext();

let notificationId = 0;

// Auto-dismiss timeout in ms
const AUTO_DISMISS_TIMEOUT = 5000;

export function NotificationProvider(props) {
  const [notifications, setNotifications] = createStore([]);

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
      setTimeout(() => {
        dismiss(id);
      }, AUTO_DISMISS_TIMEOUT);
    }

    return id;
  }

  function dismiss(id) {
    setNotifications((prev) => prev.filter((n) => n.id !== id));
  }

  function dismissAll() {
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
