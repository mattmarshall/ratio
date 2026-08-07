import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App.js";

// One client for the app. `retry: 1` rather than the default 3: this console
// talks to a Lambda a few metres away, and three retries on a genuine 404 turns
// a clear "no such fund" into four seconds of spinner.
const client = new QueryClient({
  defaultOptions: { queries: { retry: 1 } },
});

const root = document.getElementById("root");
if (!root) throw new Error("no #root");

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>
  </StrictMode>,
);
