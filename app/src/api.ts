// The app's commands and the shapes they return (app/src-tauri/src/ui.rs).

import { invoke } from "@tauri-apps/api/core";

export type FlagKind = "double_booked" | "assigned_while_off" | "not_booked" | "unstaffed";

export interface Flag {
  kind: FlagKind;
  label: string;
  date: string;
  message: string;
  person: string | null;
  site: string | null;
  sites: string[];
  chips: number[];
}

export interface Info {
  label: string;
  date: string;
  message: string;
}

export interface Chip {
  index: number;
  kind: "person" | "site";
  text: string;
  reason: string;
}

export interface QuestionView {
  subject: string;
  text: string;
  options: string[];
  current?: string;
  question: { type: string; a?: string; b?: string };
}

export interface Answered {
  subject: string;
  text: string;
  answer: string;
  undid_chip: boolean;
}

export interface PersonCell {
  items: string[];
  mark?: "red" | "amber";
}

export interface WeekView {
  dates: string[];
  working: boolean[];
  people: { name: string; kind: "employee" | "placeholder" | "subcontractor"; cells: PersonCell[] }[];
  jobs: { name: string; cells: { people: string[]; unstaffed: boolean }[] }[];
}

export interface Reading {
  sheet: string;
  layout: string | null;
  layout_options: string[];
  columns: { header: string; role: string }[];
  roles_editable: boolean;
  roles: string[];
  sheets: string[];
}

export interface Report {
  generation: number;
  smart: "off" | "pending" | "done";
  status_line: string;
  name: string;
  error: string | null;
  notes: string[];
  flags: Flag[];
  info: Info[];
  chips: Chip[];
  questions: QuestionView[];
  answered: Answered[];
  week: WeekView;
  reading: Reading;
  workdays: boolean[];
  days_off: string[];
  save_name: string;
  editable: boolean;
}

export interface Mode {
  selfTest: boolean;
  uiTest: boolean;
  version: string;
}

export interface About {
  version: string;
  variant: string;
  model_name: string;
  model_license: string;
  model_status: string;
  built_with_llama: boolean;
}

export interface Table {
  rows: string[][] | null;
  why: string | null;
}

export interface Saved {
  name: string;
  format: "xlsx" | "csv";
}

/** Today in the owner's time zone, as YYYY-MM-DD. */
export function todayIso(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export const api = {
  mode: () => invoke<Mode>("app_mode"),
  start: () => invoke<Report>("start", { todayIso: todayIso() }),
  example: () => invoke<Report>("example", { todayIso: todayIso() }),
  answer: (subject: string, option: string) => invoke<Report>("answer", { subject, option }),
  undoChip: (index: number) => invoke<Report>("undo_chip", { index }),
  forgetAnswer: (subject: string) => invoke<Report>("forget_answer", { subject }),
  setRole: (header: string, role: string) => invoke<Report>("set_role", { header, role }),
  setLayout: (layout: string) => invoke<Report>("set_layout", { layout }),
  setSheet: (sheet: string) => invoke<Report>("set_sheet", { sheet }),
  setWorkdays: (days: boolean[]) => invoke<Report>("set_workdays", { days }),
  toggleDayOff: (date: string) => invoke<Report>("toggle_day_off", { date }),
  table: () => invoke<Table>("table"),
  setTable: (rows: string[][]) => invoke<Report>("set_table", { rows }),
  paste: (text: string) => invoke<Report>("paste", { text }),
  openFile: () => invoke<Report | null>("open_file"),
  save: () => invoke<Saved | null>("save"),
  diagnostics: () => invoke<string>("diagnostics"),
  saveDiagnostics: (text: string) => invoke<string | null>("save_diagnostics", { text }),
  about: () => invoke<About>("about"),
  pageError: (message: string) => invoke("page_error", { message }),
  selfTestDone: (rendered: { flags: number; questions: number; people: number }) =>
    invoke("self_test_done", { rendered }),
};
