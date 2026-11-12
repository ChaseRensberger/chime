import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { createDemoEngine } from "./engine/demo";
import { createNativeEngine } from "./engine/native";
import { isTauri } from "@tauri-apps/api/core";
import { createHistory } from "./engine/history";
import "./globals.css";
import "./app.css";

const engine = isTauri() ? createNativeEngine() : createDemoEngine();
const history = createHistory(engine);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App engine={engine} history={history} />
  </StrictMode>,
);

if (import.meta.hot)
  import.meta.hot.dispose(() => {
    history.dispose();
    engine.dispose();
  });
