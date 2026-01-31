import { render } from "solid-js/web";
import { Router, Route } from "@solidjs/router";
import { I18nProvider } from "./context/I18nContext";
import { ProcessProvider } from "./context/ProcessContext";
import { NotificationProvider } from "./context/NotificationContext";
import { NotificationContainer } from "./components/NotificationContainer";
import { ProcessPage } from "./components/ProcessPage";
import { PastMunchingsPage } from "./components/PastMunchingsPage";

function App(props) {
  return (
    <I18nProvider>
      <NotificationProvider>
        <ProcessProvider>
          {props.children}
        </ProcessProvider>
        <NotificationContainer />
      </NotificationProvider>
    </I18nProvider>
  );
}

export function mountApp(container, basePath = "/app") {
  if (!container) return;
  render(() => (
    <Router root={App} base={basePath}>
      <Route path="/" component={ProcessPage} />
      <Route path="/past-munchings" component={PastMunchingsPage} />
    </Router>
  ), container);
}
