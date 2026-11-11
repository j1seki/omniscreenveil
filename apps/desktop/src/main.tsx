import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Surface from "./Surface";
import "./styles.css";

const surfaceOnly = new URLSearchParams(window.location.search).get("surface") === "1";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  surfaceOnly ? (
    <Surface />
  ) : (
    <React.StrictMode>
      <App />
    </React.StrictMode>
  ),
);
