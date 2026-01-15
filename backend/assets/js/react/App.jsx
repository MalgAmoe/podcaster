import React from "react";
import { ProcessProvider } from "./context/ProcessContext";
import { ProcessPage } from "./components/ProcessPage";

export function App() {
  return (
    <ProcessProvider>
      <ProcessPage />
    </ProcessProvider>
  );
}
