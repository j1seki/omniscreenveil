import { createRoot } from "react-dom/client";
import Surface from "./Surface";
import { captureClientErrors } from "./lib/clientDiagnostics";
import "./index.css";

captureClientErrors();

createRoot(document.getElementById("root")!).render(<Surface />);
