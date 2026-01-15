import React from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";

let root = null;

export function mountApp(container) {
  if (!container) {
    console.error("React mount container not found");
    return;
  }

  root = createRoot(container);
  root.render(<App />);
}

export function unmountApp() {
  if (root) {
    root.unmount();
    root = null;
  }
}
