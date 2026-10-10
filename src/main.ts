import { getCurrentWebview } from "@tauri-apps/api/webview";
import { mount } from "svelte";
import App from "./App.svelte";
import { dropPaths } from "./lib/actions";
import { dropHover } from "./lib/dropHover.svelte";
import { installMenu } from "./lib/menu";
import "./styles.css";

const app = mount(App, { target: document.getElementById("app")! });

void installMenu();
void getCurrentWebview().onDragDropEvent((event) => {
  const p = event.payload;
  if (p.type === "enter" || p.type === "over") dropHover.active = true;
  else if (p.type === "leave") dropHover.active = false;
  else if (p.type === "drop") {
    dropHover.active = false;
    void dropPaths(p.paths);
  }
});

export default app;
