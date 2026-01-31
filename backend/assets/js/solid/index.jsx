import { render } from "solid-js/web";
import { Router, Route } from "@solidjs/router";
import { ProcessProvider } from "./context/ProcessContext";
import { NotificationProvider } from "./context/NotificationContext";
import { NotificationContainer } from "./components/NotificationContainer";
import { ProcessPage } from "./components/ProcessPage";
import { PastMunchingsPage } from "./components/PastMunchingsPage";

function App(props) {
  return (
    <NotificationProvider>
      <ProcessProvider>
        {props.children}
      </ProcessProvider>
      <NotificationContainer />
    </NotificationProvider>
  );
}

export function mountApp(container) {
  if (!container) return;
  render(() => (
    <Router root={App} base="/app">
      <Route path="/" component={ProcessPage} />
      <Route path="/past-munchings" component={PastMunchingsPage} />
    </Router>
  ), container);
}
