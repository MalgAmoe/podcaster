import { render } from "solid-js/web";
import { App } from "./App";

export function mountApp(container) {
  if (!container) return;
  render(() => <App />, container);
}
