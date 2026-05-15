import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("ASR Pro root element is missing.");
}

const appRoot = rootElement;

function dismissStartupLoaderWhenMounted() {
  const loader = document.getElementById("app-loading-state");
  if (!loader) return;

  const removeLoader = () => {
    loader.setAttribute("aria-hidden", "true");
    requestAnimationFrame(() => {
      loader.remove();
    });
  };

  if (appRoot.childElementCount > 0) {
    removeLoader();
    return;
  }

  const observer = new MutationObserver(() => {
    if (appRoot.childElementCount === 0) return;

    observer.disconnect();
    removeLoader();
  });

  observer.observe(appRoot, { childList: true });
}

ReactDOM.createRoot(appRoot).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

dismissStartupLoaderWhenMounted();
