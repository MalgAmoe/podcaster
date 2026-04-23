import { render } from "solid-js/web";
import { Router, Route } from "@solidjs/router";
import { ProcessProvider } from "./context/ProcessContext";
import { NotificationProvider } from "./context/NotificationContext";
import { NotificationContainer } from "./components/NotificationContainer";
import { ProcessPage } from "./components/ProcessPage";
import { PastMunchingsPage } from "./components/PastMunchingsPage";
import { LandingTrialPage } from "./components/LandingTrialPage";

function Providers(props) {
  return (
    <NotificationProvider>
      <ProcessProvider>
        {props.children}
      </ProcessProvider>
      <NotificationContainer />
    </NotificationProvider>
  );
}

export function mountApp(container, basePath = "/app") {
  if (!container) return;
  render(() => (
    <Router root={Providers} base={basePath}>
      <Route path="/" component={ProcessPage} />
      <Route path="/past-munchings" component={PastMunchingsPage} />
    </Router>
  ), container);
}

export function mountLandingTrial(container) {
  if (!container) return;
  render(() => (
    <Providers>
      <LandingTrialPage />
    </Providers>
  ), container);
}
