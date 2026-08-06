import React from "react";
import ReactDOM from "react-dom/client";
import FloatApp from "./FloatApp";
import SettingsApp from "./SettingsApp";
import "./styles.css";

const params = new URLSearchParams(window.location.search);
const windowKind = params.get("window");

document.documentElement.dataset.window = windowKind === "float" ? "float" : "settings";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{windowKind === "float" ? <FloatApp /> : <SettingsApp />}</React.StrictMode>,
);
