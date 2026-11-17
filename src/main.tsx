import {constructionTimingEnabled,recordConstructionRender} from './constructionTiming';
import React from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./style.css";

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {constructionTimingEnabled()?<React.Profiler id="App" onRender={recordConstructionRender}><App/></React.Profiler>:<App/>}
  </React.StrictMode>,
);
