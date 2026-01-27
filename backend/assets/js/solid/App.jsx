import { ProcessProvider } from "./context/ProcessContext";
import { NotificationProvider } from "./context/NotificationContext";
import { ProcessPage } from "./components/ProcessPage";
import { NotificationContainer } from "./components/NotificationContainer";

export function App() {
  return (
    <NotificationProvider>
      <ProcessProvider>
        <ProcessPage />
      </ProcessProvider>
      <NotificationContainer />
    </NotificationProvider>
  );
}
