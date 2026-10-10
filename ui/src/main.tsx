import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { App } from "./App.tsx";
import "./styles.css";

const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity, retry: false, refetchOnWindowFocus: false } } });
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={client}><App /></QueryClientProvider>
  </StrictMode>,
);
