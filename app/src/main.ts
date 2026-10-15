// The one window: toolbar, week grid, jobs, findings, questions and "What I
// read". The page holds no schedule logic: every change goes to the app,
// which answers with a fresh report to draw.

import { listen } from "@tauri-apps/api/event";
import { api, type Chip, type Mode, type QuestionView, type Report } from "./api";
import { APP_NAME, COMPANY, CONTACT, WEBSITE } from "./brand";
import { h, shortDate, sr } from "./dom";

const DAY_LETTERS = ["M", "T", "W", "Th", "F", "Sa", "Su"];
const DAY_NAMES = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const PRESETS: [string, boolean[]][] = [
  ["Mon–Fri", [true, true, true, true, true, false, false]],
  ["Mon–Sat", [true, true, true, true, true, true, false]],
  ["All 7", [true, true, true, true, true, true, true]],
];

let report: Report | null = null;
let mode: Mode | null = null;
let tableOpen = false;
let tableRows: string[][] | null = null;
let tableWhy: string | null = null;
let tableTimer: number | undefined;

const $ = (id: string) => document.getElementById(id) as HTMLElement;

// ------------------------------------------------------------------ actions

/** Runs a command that returns a report, with a busy state and errors shown. */
async function act(run: () => Promise<Report | null | undefined>): Promise<void> {
  document.body.dataset.busy = "1";
  try {
    const next = await run();
    if (next) show(next);
  } catch (e) {
    toast(String(e), true);
  } finally {
    delete document.body.dataset.busy;
  }
}

function toast(message: string, problem = false): void {
  const t = $("toast");
  t.textContent = message;
  t.className = problem ? "toast problem" : "toast";
  t.hidden = false;
  window.clearTimeout(Number(t.dataset.timer));
  t.dataset.timer = String(window.setTimeout(() => (t.hidden = true), problem ? 9000 : 5000));
}

async function openFile(): Promise<void> {
  await act(async () => {
    const r = await api.openFile();
    if (r) resetTable(r);
    return r;
  });
}

async function save(): Promise<void> {
  document.body.dataset.busy = "1";
  try {
    const saved = await api.save();
    if (saved) {
      toast(
        saved.format === "csv"
          ? `Saved the findings as ${saved.name}.`
          : `Saved ${saved.name}. Open it here again to pick up where you left off.`,
      );
    }
  } catch (e) {
    toast(`Couldn't save: ${e}`, true);
  } finally {
    delete document.body.dataset.busy;
  }
}

async function pasteText(text: string): Promise<void> {
  await act(async () => {
    const r = await api.paste(text);
    resetTable(r);
    return r;
  });
}

function resetTable(r: Report | null): void {
  if (!r) return;
  tableRows = null;
  tableWhy = null;
  tableOpen = r.name === "Example week";
  delete $("table-body").dataset.built;
}

// ------------------------------------------------------------------ drawing

/** Keeps keyboard focus on "the same" control across a redraw. */
function withFocus(draw: () => void): void {
  const active = document.activeElement as HTMLElement | null;
  const key = active?.dataset.key;
  const panel = active?.closest("section")?.id;
  draw();
  if (!key) return;
  const again = document.querySelector<HTMLElement>(`[data-key="${CSS.escape(key)}"]`);
  if (again) {
    again.focus();
    return;
  }
  // The control went away (an answered question): stay in its panel.
  const next = panel ? $(panel).querySelector<HTMLElement>("button, select, input") : null;
  (next ?? $(panel ?? "findings-panel").querySelector<HTMLElement>("h2"))?.focus();
}

function show(next: Report): void {
  // A late model pass for an older change is dropped.
  if (report && next.generation < report.generation) return;
  if (report && next.generation === report.generation && next.smart !== "done") return;
  report = next;
  withFocus(() => {
    drawToolbar(next);
    drawStatus(next);
    drawGrid(next);
    drawJobs(next);
    drawFindings(next);
    drawQuestions(next);
    drawReading(next);
    drawTable(next);
  });
}

function drawToolbar(r: Report): void {
  const days = $("workdays");
  days.replaceChildren(
    h("span", { class: "label", id: "workdays-label" }, "Working days"),
    ...r.workdays.map((on, i) =>
      h(
        "button",
        {
          class: "day",
          "aria-pressed": on ? "true" : "false",
          "aria-label": `${DAY_NAMES[i]}: ${on ? "working day" : "day off"}`,
          title: `${DAY_NAMES[i]}: click to make it ${on ? "a day off" : "a working day"}`,
          "data-key": `day-${i}`,
          onclick: () => {
            const days = [...r.workdays];
            days[i] = !days[i];
            void act(() => api.setWorkdays(days));
          },
        },
        DAY_LETTERS[i],
      ),
    ),
    ...PRESETS.map(([label, days]) =>
      h(
        "button",
        {
          class: "preset",
          "data-key": `preset-${label}`,
          "aria-pressed": days.every((d, i) => d === r.workdays[i]) ? "true" : "false",
          onclick: () => void act(() => api.setWorkdays(days)),
        },
        label,
      ),
    ),
  );
}

function drawStatus(r: Report): void {
  $("source-name").textContent = r.name;
  const smart = $("smart-status");
  smart.textContent =
    r.smart === "pending" ? "Smart matching is checking the unsure items…" : r.status_line;
  smart.dataset.state = r.smart;
  $("problem-banner").hidden = !r.error;
  $("problem-banner").textContent = r.error ?? "";
}

function cellLines(items: string[]): Node[] {
  const out: Node[] = [];
  items.forEach((t, i) => {
    if (i) out.push(h("br"));
    out.push(document.createTextNode(t));
  });
  return out;
}

function drawGrid(r: Report): void {
  const w = r.week;
  const panel = $("grid-body");
  if (!w.people.length) {
    panel.replaceChildren(
      h("p", { class: "empty" }, r.error ? "Nothing to show yet." : "No people found in this schedule."),
    );
    return;
  }
  const head = h(
    "tr",
    {},
    h("th", { scope: "col", class: "corner" }, "Person"),
    ...w.dates.map((d, i) => {
      const off = r.days_off.includes(d);
      return h(
        "th",
        { scope: "col", class: w.working[i] ? "" : "nonworking" },
        h("span", { class: "date" }, shortDate(d)),
        h(
          "button",
          {
            class: "dayoff",
            "aria-pressed": off ? "true" : "false",
            "data-key": `off-${d}`,
            title: off
              ? "Marked a day off for everyone. Click to make it a working day again."
              : "Mark this date a day off for everyone (a holiday)",
            onclick: () => void act(() => api.toggleDayOff(d)),
          },
          off ? "Day off ✓" : "Day off?",
        ),
      );
    }),
  );
  const rows = w.people.map((p) =>
    h(
      "tr",
      { "data-person-row": p.name },
      h(
        "th",
        { scope: "row" },
        p.name,
        p.kind !== "employee" ? h("span", { class: "kind" }, p.kind === "placeholder" ? " (role)" : " (sub)") : null,
      ),
      ...p.cells.map((c, i) =>
        h(
          "td",
          { class: [c.mark ?? "", w.working[i] ? "" : "nonworking"].join(" ").trim() },
          c.mark === "red" ? sr("Problem: ") : c.mark === "amber" ? sr("Not booked: ") : null,
          ...cellLines(c.items),
          c.mark === "amber" && !c.items.length ? h("span", { class: "nothing", "aria-hidden": "true" }, "—") : null,
        ),
      ),
    ),
  );
  panel.replaceChildren(
    h("table", { class: "week" }, h("thead", {}, head), h("tbody", {}, ...rows)),
  );
}

function drawJobs(r: Report): void {
  const w = r.week;
  const panel = $("jobs-body");
  if (!w.jobs.length) {
    panel.replaceChildren(h("p", { class: "empty" }, "No jobs found."));
    return;
  }
  const head = h(
    "tr",
    {},
    h("th", { scope: "col", class: "corner" }, "Job"),
    ...w.dates.map((d, i) => h("th", { scope: "col", class: w.working[i] ? "" : "nonworking" }, shortDate(d))),
  );
  const rows = w.jobs.map((j) =>
    h(
      "tr",
      {},
      h("th", { scope: "row" }, j.name),
      ...j.cells.map((c, i) =>
        c.unstaffed
          ? h("td", { class: "unstaffed" }, sr("Unstaffed: "), "Nobody")
          : h("td", { class: w.working[i] ? "" : "nonworking" }, c.people.join(", ")),
      ),
    ),
  );
  panel.replaceChildren(h("table", { class: "week jobs" }, h("thead", {}, head), h("tbody", {}, ...rows)));
}

function chipButton(c: Chip): HTMLElement {
  return h(
    "span",
    { class: "chip" },
    c.text,
    h(
      "button",
      {
        class: "undo",
        "data-key": `chip-${c.text}`,
        "aria-label": `Undo: ${c.text}`,
        title: `Why: ${c.reason}. Undo if they're not the same.`,
        onclick: () => void act(() => api.undoChip(c.index)),
      },
      "Undo",
    ),
  );
}

function drawFindings(r: Report): void {
  $("findings-count").textContent = r.error ? "" : `(${r.flags.length})`;
  const list = $("findings-body");
  if (r.error) {
    list.replaceChildren(h("p", { class: "empty" }, "Open or paste a schedule to check it."));
    return;
  }
  const items = r.flags.map((f) =>
    h(
      "li",
      { class: `finding ${f.kind}`, "data-flag": f.kind },
      h("span", { class: "tag" }, f.label),
      h("p", {}, f.message),
      f.chips.length
        ? h("div", { class: "chips" }, ...f.chips.map((i) => r.chips[i]).filter(Boolean).map(chipButton))
        : null,
    ),
  );
  const body: Node[] = [];
  if (!r.flags.length) {
    body.push(h("p", { class: "good" }, "No problems found. Nobody is double-booked, booked while off, idle or missing."));
  } else {
    body.push(h("ul", { class: "findings" }, ...items));
  }
  if (r.info.length) {
    body.push(
      h(
        "details",
        { class: "info" },
        h("summary", {}, `Also noticed (${r.info.length}), not problems`),
        h("ul", {}, ...r.info.map((i) => h("li", {}, h("strong", {}, `${i.label}: `), i.message))),
      ),
    );
  }
  list.replaceChildren(...body);
}

function currentNote(q: QuestionView): string {
  if (q.current) return `For now I'm using: ${q.current}.`;
  if (q.question.type === "same_person" || q.question.type === "same_site") {
    return "For now I'm keeping them apart.";
  }
  if (q.question.type === "weekday_mismatch") return "For now I'm using the date.";
  if (q.question.type === "fill_down") return "For now I'm filling the blanks.";
  return "";
}

function drawQuestions(r: Report): void {
  $("questions-count").textContent = `(${r.questions.length})`;
  const body: Node[] = [];
  if (!r.questions.length) {
    body.push(h("p", { class: "empty" }, "Nothing to ask. Everything was clear."));
  }
  for (const q of r.questions) {
    body.push(
      h(
        "div",
        { class: "question", "data-question": q.subject, role: "group", "aria-label": q.text },
        h("p", {}, q.text),
        h("p", { class: "current" }, currentNote(q)),
        q.options.length
          ? h(
              "div",
              { class: "answers" },
              ...q.options.map((o) =>
                h(
                  "button",
                  {
                    class: o === "Yes" ? "primary" : "",
                    "data-key": `q-${q.subject}-${o}`,
                    onclick: () => void act(() => api.answer(q.subject, o)),
                  },
                  o,
                ),
              ),
            )
          : null,
      ),
    );
  }
  if (r.answered.length) {
    body.push(
      h(
        "details",
        { class: "answered" },
        h("summary", {}, `Your answers (${r.answered.length})`),
        h(
          "ul",
          {},
          ...r.answered.map((a) =>
            h(
              "li",
              {},
              h("span", {}, `${a.text} `),
              h("strong", {}, a.undid_chip ? "Undone." : a.answer),
              " ",
              h(
                "button",
                {
                  class: "link",
                  "data-key": `forget-${a.subject}`,
                  onclick: () => void act(() => api.forgetAnswer(a.subject)),
                },
                a.undid_chip ? "Put back" : "Change",
              ),
            ),
          ),
        ),
      ),
    );
  }
  $("questions-body").replaceChildren(...body);
}

function select(
  label: string,
  options: string[],
  value: string,
  key: string,
  onchange: (v: string) => void,
): HTMLElement {
  const s = h(
    "select",
    { "aria-label": label, "data-key": key },
    ...options.map((o) => h("option", { value: o, selected: o === value }, o)),
  );
  s.addEventListener("change", () => onchange(s.value));
  return s;
}

function drawReading(r: Report): void {
  const rd = r.reading;
  const rows: Node[] = [];
  if (r.error) {
    $("read-body").replaceChildren(h("p", { class: "empty" }, "Nothing read yet."));
    return;
  }
  if (rd.sheets.length > 1) {
    rows.push(
      h("div", { class: "row" }, h("span", {}, "Sheet"), select("Sheet", rd.sheets, rd.sheet, "sheet", (v) => void act(() => api.setSheet(v)))),
    );
  }
  const layouts = rd.layout_options.length ? rd.layout_options : rd.layout ? [rd.layout] : [];
  if (rd.layout) {
    rows.push(
      h(
        "div",
        { class: "row" },
        h("span", {}, "Layout"),
        layouts.length > 1
          ? select("Layout", layouts, rd.layout, "layout", (v) => void act(() => api.setLayout(v)))
          : h("span", { class: "value" }, rd.layout),
      ),
    );
  }
  // A grid's date columns read as one run: "Mon 10/12 to Fri 10/16".
  const columns: { header: string; role: string }[] = [];
  for (const c of rd.columns) {
    const day = !rd.roles_editable && /^(Day|Date)\b/.test(c.role);
    const last = columns[columns.length - 1] as { header: string; role: string; first?: string } | undefined;
    if (day && last && last.role === "Days") {
      last.header = `${last.first} to ${c.header}`;
    } else if (day) {
      columns.push({ header: c.header, role: "Days", first: c.header } as { header: string; role: string });
    } else {
      columns.push(c);
    }
  }
  if (columns.length) {
    rows.push(
      h(
        "table",
        { class: "columns" },
        h("thead", {}, h("tr", {}, h("th", { scope: "col" }, "Column"), h("th", { scope: "col" }, "Read as"))),
        h(
          "tbody",
          {},
          ...columns.map((c) =>
            h(
              "tr",
              {},
              h("th", { scope: "row" }, c.header || "(no header)"),
              h(
                "td",
                {},
                rd.roles_editable
                  ? select(`What "${c.header}" holds`, rd.roles, c.role, `role-${c.header}`, (v) =>
                      void act(() => api.setRole(c.header, v)),
                    )
                  : c.role,
              ),
            ),
          ),
        ),
      ),
    );
  }
  const unattached = r.chips.filter((c) => !r.flags.some((f) => f.chips.includes(c.index)));
  if (unattached.length) {
    rows.push(h("h3", {}, "Names and jobs I matched"), h("div", { class: "chips" }, ...unattached.map(chipButton)));
  }
  if (r.notes.length) {
    rows.push(h("ul", { class: "notes" }, ...r.notes.map((n) => h("li", {}, n))));
  }
  $("read-body").replaceChildren(...rows);
}

// ------------------------------------------------------------ editable table

async function loadTable(): Promise<void> {
  const t = await api.table();
  tableRows = t.rows;
  tableWhy = t.why;
}

function queueTableSave(): void {
  window.clearTimeout(tableTimer);
  tableTimer = window.setTimeout(() => {
    if (tableRows) void act(() => api.setTable(tableRows!));
  }, 400);
}

/** Draws the editable table. A report alone doesn't rebuild it, so typing
 * keeps its place; opening, a new schedule, or adding or deleting a row do. */
function drawTable(r: Report, rebuild = false): void {
  const panel = $("table-body");
  const toggle = $("table-toggle") as HTMLButtonElement;
  toggle.hidden = !r.editable && !tableOpen;
  toggle.setAttribute("aria-expanded", tableOpen ? "true" : "false");
  toggle.textContent = tableOpen ? "Hide the table" : "Edit the schedule";
  if (!tableOpen) {
    panel.replaceChildren();
    delete panel.dataset.built;
    return;
  }
  if (panel.dataset.built && !rebuild) return;
  if (tableRows === null && tableWhy === null) {
    panel.replaceChildren(h("p", { class: "empty" }, "Loading…"));
    void loadTable().then(() => report && withFocus(() => drawTable(report!, true)));
    return;
  }
  panel.dataset.built = "1";
  if (!tableRows) {
    panel.replaceChildren(h("p", { class: "empty" }, tableWhy ?? ""));
    return;
  }
  const rows = tableRows;
  const width = Math.max(1, ...rows.map((x) => x.length));
  const header = rows[0] ?? [];
  const body = rows.map((row, ri) =>
    h(
      "tr",
      {},
      ...Array.from({ length: width }, (_, ci) => {
        const input = h("input", {
          value: row[ci] ?? "",
          "aria-label": `Row ${ri + 1}${header[ci] && ri > 0 ? `, ${header[ci]}` : `, column ${ci + 1}`}`,
          "data-key": `cell-${ri}-${ci}`,
        });
        input.addEventListener("input", () => {
          while (rows[ri].length <= ci) rows[ri].push("");
          rows[ri][ci] = input.value;
          queueTableSave();
        });
        return h("td", {}, input);
      }),
      h(
        "td",
        { class: "rowtools" },
        h(
          "button",
          {
            class: "link",
            "aria-label": `Delete row ${ri + 1}`,
            "data-key": `del-${ri}`,
            onclick: () => {
              rows.splice(ri, 1);
              queueTableSave();
              drawTable(r, true);
            },
          },
          "Delete",
        ),
      ),
    ),
  );
  panel.replaceChildren(
    h("p", { class: "hint" }, "Type to change a name, job or day. Results update as you type."),
    h("div", { class: "scroll" }, h("table", { class: "edit" }, h("tbody", {}, ...body))),
    h(
      "button",
      {
        "data-key": "add-row",
        onclick: () => {
          rows.push(Array.from({ length: width }, () => ""));
          drawTable(r, true);
          $("table-body").querySelector<HTMLInputElement>(`[data-key="cell-${rows.length - 1}-0"]`)?.focus();
        },
      },
      "Add a row",
    ),
  );
}

// ------------------------------------------------------------------ dialogs

function openPasteDialog(): void {
  const d = $("paste-dialog") as HTMLDialogElement;
  const area = $("paste-text") as HTMLTextAreaElement;
  area.value = "";
  d.showModal();
  area.focus();
}

async function openAbout(): Promise<void> {
  const a = await api.about();
  const lines: Node[] = [
    h("p", {}, `${APP_NAME} ${a.version}, a free tool from ${COMPANY}.`),
    CONTACT ? h("p", {}, CONTACT) : null,
    WEBSITE ? h("p", {}, WEBSITE) : null,
    h(
      "p",
      { class: "privacy" },
      "Nothing leaves this computer. The app has no internet code at all. To check, turn Wi-Fi off: everything still works.",
    ),
    h("h3", {}, a.model_name ? "Smart matching" : "Unsure names and sites"),
    h("p", {}, a.model_status),
    a.model_name ? h("p", {}, `Model: ${a.model_name} (license: ${a.model_license})`) : null,
    a.built_with_llama ? h("p", {}, "Built with Llama.") : null,
    h("h3", {}, "Licenses"),
    h(
      "ul",
      {},
      h("li", {}, "llama.cpp: MIT License"),
      h("li", {}, "Tauri: MIT or Apache License 2.0"),
      a.model_name ? h("li", {}, `${a.model_name}: ${a.model_license}`) : null,
      h("li", {}, "The full license texts ship in the app's Licenses folder."),
    ),
  ].filter(Boolean) as Node[];
  $("about-body").replaceChildren(...lines);
  ($("about-dialog") as HTMLDialogElement).showModal();
}

async function openDiagnostics(): Promise<void> {
  const text = await api.diagnostics();
  $("diag-text").textContent = text;
  ($("about-dialog") as HTMLDialogElement).close();
  ($("diag-dialog") as HTMLDialogElement).showModal();
}

// ------------------------------------------------------------------ wiring

function isEditable(t: EventTarget | null): boolean {
  return t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement;
}

function wire(): void {
  $("open").addEventListener("click", () => void openFile());
  $("paste").addEventListener("click", openPasteDialog);
  $("save").addEventListener("click", () => void save());
  $("about").addEventListener("click", () => void openAbout());
  $("example").addEventListener("click", () =>
    void act(async () => {
      const r = await api.example();
      resetTable(r);
      return r;
    }),
  );
  $("table-toggle").addEventListener("click", () => {
    tableOpen = !tableOpen;
    if (report) withFocus(() => drawTable(report!, true));
  });
  $("paste-go").addEventListener("click", () => {
    const text = ($("paste-text") as HTMLTextAreaElement).value;
    ($("paste-dialog") as HTMLDialogElement).close();
    if (text.trim()) void pasteText(text);
  });
  $("diag-button").addEventListener("click", () => void openDiagnostics());
  $("diag-save").addEventListener("click", async () => {
    try {
      const name = await api.saveDiagnostics($("diag-text").textContent ?? "");
      if (name) {
        ($("diag-dialog") as HTMLDialogElement).close();
        toast(`Saved ${name}.`);
      }
    } catch (e) {
      toast(`Couldn't save: ${e}`, true);
    }
  });
  for (const b of document.querySelectorAll<HTMLButtonElement>("[data-close]")) {
    b.addEventListener("click", () => (b.closest("dialog") as HTMLDialogElement).close());
  }

  // Cmd/Ctrl+V anywhere outside a text field checks the clipboard's schedule.
  document.addEventListener("paste", (e) => {
    if (isEditable(e.target)) return;
    const text = e.clipboardData?.getData("text/plain") ?? "";
    if (text.trim()) {
      e.preventDefault();
      void pasteText(text);
    }
  });
  document.addEventListener("keydown", (e) => {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === "o") {
      e.preventDefault();
      void openFile();
    } else if (mod && e.key.toLowerCase() === "s") {
      e.preventDefault();
      void save();
    }
  });
  // A file dragged in from the desktop is opened by the app itself.
  void listen<boolean>("drag", (e) => ($("drop-overlay").hidden = !e.payload));
  void listen<Report>("report", (e) => {
    if (e.payload.name !== report?.name) resetTable(e.payload);
    show(e.payload);
  });
  void listen<string>("problem", (e) => toast(e.payload, true));
  void listen<string>("model", () => {
    if (report) drawStatus(report);
  });
}

// Script errors go to the app's log (and fail a self-test at once).
window.addEventListener("error", (e) => void api.pageError(`${e.message} at ${e.filename}:${e.lineno}`));
window.addEventListener("unhandledrejection", (e) => void api.pageError(String(e.reason)));

async function start(): Promise<void> {
  document.title = APP_NAME;
  $("brand-from").textContent = `a free tool from ${COMPANY}`;
  wire();
  mode = await api.mode();
  const first = await api.start();
  tableOpen = first.name === "Example week";
  show(first);
  if (mode.selfTest) {
    await api.selfTestDone({
      flags: document.querySelectorAll("[data-flag]").length,
      questions: document.querySelectorAll("[data-question]").length,
      people: document.querySelectorAll("[data-person-row]").length,
    });
  }
}

void start();
