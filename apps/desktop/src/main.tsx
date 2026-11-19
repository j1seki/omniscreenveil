import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { IN_TAURI } from "./api";
import App from "./App";
import Surface from "./Surface";
import "./index.css";

const surfaceOnly = IN_TAURI
  ? getCurrentWindow().label === "surface"
  : new URLSearchParams(window.location.search).get("surface") === "1";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  surfaceOnly ? (
    <Surface />
  ) : (
    <React.StrictMode>
      <App />
    </React.StrictMode>
  ),
);
