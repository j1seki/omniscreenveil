import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { captureClientErrors } from "./lib/clientDiagnostics";
import "./index.css";

captureClientErrors();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode><App /></React.StrictMode>,
);
