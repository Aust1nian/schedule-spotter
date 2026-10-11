import { invoke } from "@tauri-apps/api/core";

interface Mode {
  selfTest: boolean;
  version: string;
  modelStatus: string;
}

interface Flag {
  kind: "double_booked" | "assigned_while_off" | "not_booked" | "unstaffed";
  message: string;
}

interface Report {
  flags: Flag[];
}

function render(root: HTMLElement, mode: Mode, report: Report): void {
  root.replaceChildren();
  const title = document.createElement("h1");
  title.textContent = "Schedule Spotter";
  const status = document.createElement("p");
  status.className = "status";
  status.textContent = mode.modelStatus;
  const list = document.createElement("ul");
  for (const flag of report.flags) {
    const item = document.createElement("li");
    item.dataset.flag = flag.kind;
    item.textContent = flag.message;
    list.append(item);
  }
  root.append(title, status, list);
}

async function start(): Promise<void> {
  const root = document.getElementById("app");
  if (!root) return;
  const mode = await invoke<Mode>("app_mode");
  const report = await invoke<Report>("run_example");
  render(root, mode, report);
  if (mode.selfTest) {
    const renderedFlags = root.querySelectorAll("[data-flag]").length;
    await invoke("self_test_done", { renderedFlags });
  }
}

void start();
