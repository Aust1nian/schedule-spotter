#!/usr/bin/env python3
"""UI tests: drive the real app through WebDriver (tauri-driver), Windows and
Linux (spec: "UI tests"). Standard library only.

Replays every category N fixture's `steps` through the window the way an
owner would (buttons, chips, dropdowns, day toggles), then checks the
findings on screen against `after_steps`. N06 (`"ui": "keyboard"`) is done
with the keyboard alone. Also checks the built-in example, editing the
table, and a paste.

The app must be built with `--features ui-test`: WebDriver can't drive the
system open/save dialogs, so that build adds two commands that open and save
a given path. Shipped builds don't have them.

Usage:
  ui_tests.py --app PATH [--driver tauri-driver] [--native-driver PATH]
              [--shots DIR] [--only N03]
"""

import argparse
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
FIXTURES = ROOT / "fixtures" / "handmade"
DAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]


class WebDriverError(Exception):
    pass


class Session:
    def __init__(self, base: str, app: str):
        self.base = base
        caps = {"capabilities": {"alwaysMatch": {"tauri:options": {"application": app}}}}
        r = self._req("POST", "/session", caps)
        self.id = r["sessionId"]

    def _req(self, method: str, path: str, body=None):
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request(self.base + path, data=data, method=method,
                                     headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=120) as resp:
                return json.loads(resp.read() or b"{}").get("value")
        except urllib.error.HTTPError as e:
            raise WebDriverError(f"{method} {path}: {e.read()[:500]!r}") from None

    def cmd(self, method: str, path: str, body=None):
        return self._req(method, f"/session/{self.id}{path}", body)

    def js(self, script: str, *args):
        return self.cmd("POST", "/execute/sync", {"script": script, "args": list(args)})

    def js_async(self, script: str, *args):
        return self.cmd("POST", "/execute/async", {"script": script, "args": list(args)})

    def invoke(self, command: str, payload: dict):
        """Calls an app command; returns [ok, value]."""
        return self.js_async(
            "const [cmd, args, done] = arguments;"
            "window.__TAURI_INTERNALS__.invoke(cmd, args)"
            ".then(v => done([true, v]), e => done([false, String(e)]));",
            command, payload)

    def screenshot(self, path: Path) -> None:
        import base64
        png = self.cmd("GET", "/screenshot")
        path.write_bytes(base64.b64decode(png))

    def keys(self, *keys: str) -> None:
        """Presses keys on whatever has focus (W3C actions)."""
        actions = []
        for k in keys:
            actions += [{"type": "keyDown", "value": k}, {"type": "keyUp", "value": k}]
        self.cmd("POST", "/actions", {"actions": [{"type": "key", "id": "kb", "actions": actions}]})
        self.cmd("DELETE", "/actions")

    def close(self) -> None:
        try:
            self._req("DELETE", f"/session/{self.id}")
        except Exception:
            pass


TAB, ENTER, SPACE = "", "", ""


def wait(s: Session, script: str, *args, timeout=30.0, what="condition"):
    end = time.time() + timeout
    last = None
    while time.time() < end:
        last = s.js(script, *args)
        if last:
            return last
        time.sleep(0.1)
    raise AssertionError(f"timed out waiting for {what} (last: {last!r})")


def idle(s: Session) -> None:
    """Waits until no command is running and the findings are drawn."""
    wait(s, "return !document.body.dataset.busy && document.getElementById('findings-count') !== null",
         what="the app to settle")
    time.sleep(0.6)  # table edits are debounced 400 ms
    wait(s, "return !document.body.dataset.busy", what="the app to settle")


def on_screen(s: Session) -> dict:
    return s.js("""
      return {
        flags: [...document.querySelectorAll('[data-flag]')].map(e => [e.dataset.flag, e.querySelector('p').textContent]),
        questions: [...document.querySelectorAll('[data-question]')].map(e => e.querySelector('p').textContent),
        people: [...document.querySelectorAll('[data-person-row]')].map(e => e.dataset.personRow),
        name: document.getElementById('source-name').textContent,
      };""")


def norm(t: str) -> str:
    return " ".join(t.lower().split())


def click(s: Session, script: str, *args, what: str) -> None:
    """Clicks the element `script` returns."""
    found = s.js(f"const el = (() => {{ {script} }})(); if (!el) return false; el.scrollIntoView({{block: 'center'}}); el.click(); return true;", *args)
    if not found:
        raise AssertionError(f"can't find {what}")
    idle(s)


def question_button(spec: dict, option: str) -> tuple[str, list]:
    names = [spec.get("a", ""), spec.get("b", "")]
    return ("""
      const [names, option] = arguments;
      const card = [...document.querySelectorAll('[data-question]')].find(c =>
        names.every(n => c.textContent.toLowerCase().includes(n.toLowerCase())));
      return card && [...card.querySelectorAll('button')].find(b => b.textContent === option);
    """, [names, option])


def step_by_mouse(s: Session, step: dict, tmp: Path) -> None:
    (op, arg), = step.items()
    if op == "answer":
        option = {"yes": "Yes", "no": "No"}.get(arg["answer"], arg["answer"])
        script, args = question_button(arg["question"], option)
        click(s, script, *args, what=f"question button {option} for {arg['question']}")
    elif op == "undo_merge":
        click(s, """
          const text = `Treating ${arguments[0]} as ${arguments[1]}`;
          const chip = [...document.querySelectorAll('.chip')].find(c => c.textContent.includes(text));
          return chip && chip.querySelector('button');
        """, arg["from"], arg["into"], what=f"chip {arg}")
    elif op == "column_role":
        ok = s.js("""
          const [header, role] = arguments;
          const sel = [...document.querySelectorAll('#read-body select')].find(x => x.getAttribute('aria-label') === `What "${header}" holds`);
          if (!sel) return false;
          sel.value = role; sel.dispatchEvent(new Event('change')); return true;
        """, arg["header"], arg["role"])
        if not ok:
            raise AssertionError(f"no role dropdown for {arg['header']}")
        idle(s)
    elif op == "workdays":
        want = [d in arg for d in DAYS]
        for i, on in enumerate(want):
            pressed = s.js("return document.querySelector(`[data-key=\"day-${arguments[0]}\"]`).getAttribute('aria-pressed') === 'true'", i)
            if pressed != on:
                click(s, "return document.querySelector(`[data-key=\"day-${arguments[0]}\"]`)", i, what=f"day {i}")
    elif op == "day_off":
        click(s, "return document.querySelector(`[data-key=\"off-${arguments[0]}\"]`)", arg, what=f"day off {arg}")
    elif op == "save_reopen":
        path = tmp / arg
        path.parent.mkdir(parents=True, exist_ok=True)
        ok, v = s.invoke("test_save", {"path": str(path)})
        if not ok:
            raise AssertionError(f"save failed: {v}")
        ok, v = s.invoke("test_open", {"path": str(path)})
        if not ok:
            raise AssertionError(f"reopen failed: {v}")
        idle(s)
    else:
        raise AssertionError(f"step {op} has no UI action")


def tab_to(s: Session, check: str, *args, limit=300) -> None:
    """Presses Tab until `check` (JS, given the focused element as arguments[0]) is true."""
    for _ in range(limit):
        if s.js(f"const el = document.activeElement; return (() => {{ {check} }})();", *args):
            return
        s.keys(TAB)
    raise AssertionError(f"Tab never reached the control ({check.strip()[:80]})")


def step_by_keyboard(s: Session, step: dict) -> None:
    (op, arg), = step.items()
    if op == "answer":
        option = {"yes": "Yes", "no": "No"}.get(arg["answer"], arg["answer"])
        names = [arg["question"].get("a", ""), arg["question"].get("b", "")]
        tab_to(s, """
          const [names, option] = arguments;
          const card = el && el.closest('[data-question]');
          return !!card && el.textContent === option && names.every(n => card.textContent.toLowerCase().includes(n.toLowerCase()));
        """, names, option)
        s.keys(ENTER)
    elif op == "day_off":
        tab_to(s, "return el && el.dataset.key === `off-${arguments[0]}`;", arg)
        s.keys(SPACE)
    else:
        raise AssertionError(f"step {op} has no keyboard action")
    idle(s)


def check_after(s: Session, expected: dict, problems: list) -> None:
    target = expected.get("after_steps", {})
    shown = on_screen(s)
    if "flags" in target:
        want = sorted(f["type"] for f in target["flags"])
        got = sorted(k for k, _ in shown["flags"])
        if want != got:
            problems.append(f"findings on screen {got}, expected {want}")
        for f in target["flags"]:
            names = [x for x in [f.get("person"), f.get("site")] if x]
            if not any(k == f["type"] and all(norm(n) in norm(m) for n in names) for k, m in shown["flags"]):
                problems.append(f"no finding on screen for {f}")
    for spec in target.get("must_not_ask", []):
        names = [x for x in [spec.get("a"), spec.get("b")] if x]
        for q in shown["questions"]:
            if names and all(norm(n) in norm(q) for n in names):
                problems.append(f"still asked: {q}")
            if not names and spec["type"] == "same_site" and "same job" in q:
                problems.append(f"still asked: {q}")


def run_fixture(s: Session, fixture: Path, shots: Path | None, tmp: Path) -> list:
    expected = json.loads((fixture / "expected.json").read_text(encoding="utf-8"))
    problems: list[str] = []
    settings = expected.get("settings", {})
    ok, v = s.invoke("test_open", {"path": str(fixture / expected.get("input", "input.csv"))})
    if not ok:
        return [f"open failed: {v}"]
    idle(s)
    # The fixture's working days (default all seven), set the way the owner
    # would; the app keeps the last schedule's days for a new one.
    step_by_mouse(s, {"workdays": settings.get("workdays", DAYS)}, tmp)
    want = sorted(f["type"] for f in expected.get("flags", []))
    got = sorted(k for k, _ in on_screen(s)["flags"])
    if "flags" in expected and want != got:
        problems.append(f"before steps: findings on screen {got}, expected {want}")
    keyboard = expected.get("ui") == "keyboard"
    if keyboard:
        # Start from the top of the page, as a keyboard user would.
        s.js("document.activeElement && document.activeElement.blur(); window.scrollTo(0, 0);")
    for step in expected.get("steps", []):
        try:
            if keyboard:
                step_by_keyboard(s, step)
            else:
                step_by_mouse(s, step, tmp)
        except (AssertionError, WebDriverError) as e:
            problems.append(f"step {step}: {e}")
            break
    check_after(s, expected, problems)
    if shots:
        s.screenshot(shots / f"{fixture.name}.png")
    return problems


def test_example(s: Session, shots: Path | None) -> list:
    problems = []
    s.js("document.getElementById('example').click()")
    idle(s)
    shown = on_screen(s)
    kinds = {k for k, _ in shown["flags"]}
    for k in ["double_booked", "assigned_while_off", "not_booked", "unstaffed"]:
        if k not in kinds:
            problems.append(f"example: no {k} finding")
    if not shown["questions"]:
        problems.append("example: no question")
    if not s.js("return document.querySelectorAll('.finding .chip').length > 0"):
        problems.append("example: no 'treating A as B' chip")
    if not s.js("return document.querySelectorAll('td.red').length > 0 && document.querySelectorAll('td.amber').length > 0 && document.querySelectorAll('td.unstaffed').length > 0"):
        problems.append("example: the grid is missing red, amber or striped cells")
    # The toolbar stays on screen at 1280x800.
    s.cmd("POST", "/window/rect", {"width": 1280, "height": 800})
    s.js("window.scrollTo(0, 10000)")
    top = s.js("return document.querySelector('.toolbar').getBoundingClientRect().top")
    if top is None or top < -1:
        problems.append(f"toolbar scrolled away (top {top})")
    s.js("window.scrollTo(0, 0)")
    smallest = s.js("""
      let min = 99;
      for (const el of document.querySelectorAll('body *')) {
        if (!el.childNodes.length || ![...el.childNodes].some(n => n.nodeType === 3 && n.textContent.trim())) continue;
        const st = getComputedStyle(el);
        if (st.display === 'none' || el.closest('.sr')) continue;
        min = Math.min(min, parseFloat(st.fontSize));
      }
      return min;""")
    if smallest < 15:
        problems.append(f"text smaller than 15 px on screen ({smallest} px)")
    if shots:
        s.screenshot(shots / "example.png")
    return problems


def test_edit_table(s: Session, shots: Path | None) -> list:
    """Editing the example table directly updates the results."""
    problems = []
    s.js("document.getElementById('example').click()")
    idle(s)
    before = len(on_screen(s)["flags"])
    # Luis Ortiz has nothing on Thursday in the example; give him a job.
    ok = s.js("""
      const rows = [...document.querySelectorAll('#table-body tr')];
      const r = rows.findIndex(tr => tr.querySelector('input').value === 'Luis Ortiz');
      if (r < 0) return false;
      const input = rows[r].querySelectorAll('input')[4];
      input.focus(); input.value = 'Smith remodel'; input.dispatchEvent(new Event('input'));
      return true;""")
    if not ok:
        return ["edit: the example table isn't editable"]
    time.sleep(0.8)
    idle(s)
    after = on_screen(s)["flags"]
    if any("Luis Ortiz has nothing" in m for _, m in after):
        problems.append("edit: Luis Ortiz is still not booked after the edit")
    if len(after) != before - 1:
        problems.append(f"edit: {before} findings became {len(after)}, expected {before - 1}")
    if shots:
        s.screenshot(shots / "edit-table.png")
    return problems


def test_paste(s: Session, shots: Path | None) -> list:
    """Paste dialog: cells copied from a spreadsheet are checked at once."""
    problems = []
    text = ("Name\tMon 10/12/2026\tTue 10/13/2026\n"
            "Mike Baker\t1420 Johnson St\t1420 Johnson St / Smith remodel\n"
            "Dave Kim\tSmith remodel\tSmith remodel\n")
    click(s, "return document.getElementById('paste')", what="Paste button")
    s.js("const t = document.getElementById('paste-text'); t.value = arguments[0];", text)
    click(s, "return document.getElementById('paste-go')", what="Check this schedule")
    shown = on_screen(s)
    if shown["name"] != "Pasted schedule":
        problems.append(f"paste: checking {shown['name']!r}")
    if [k for k, _ in shown["flags"]] != ["double_booked"]:
        problems.append(f"paste: findings {shown['flags']}")
    if shots:
        s.screenshot(shots / "paste.png")
    return problems


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--app", required=True)
    ap.add_argument("--driver", default="tauri-driver")
    ap.add_argument("--native-driver")
    ap.add_argument("--shots")
    ap.add_argument("--only")
    args = ap.parse_args()
    shots = Path(args.shots) if args.shots else None
    if shots:
        shots.mkdir(parents=True, exist_ok=True)

    port = free_port()
    cmd = [args.driver, "--port", str(port)]
    if args.native_driver:
        cmd += ["--native-driver", args.native_driver]
    driver = subprocess.Popen(cmd)
    base = f"http://127.0.0.1:{port}"
    for _ in range(100):
        try:
            urllib.request.urlopen(base + "/status", timeout=1)
            break
        except Exception:
            time.sleep(0.2)
    failures = 0
    try:
        s = Session(base, str(Path(args.app).resolve()))
        try:
            wait(s, "return document.querySelectorAll('[data-flag]').length > 0", timeout=60,
                 what="the example to show")
            tests = []
            if not args.only:
                tests += [("example", lambda: test_example(s, shots)),
                          ("edit-table", lambda: test_edit_table(s, shots)),
                          ("paste", lambda: test_paste(s, shots))]
            with tempfile.TemporaryDirectory(prefix="spotter ui ") as tmp:
                for fixture in sorted(FIXTURES.glob("N*")):
                    if args.only and not fixture.name.startswith(args.only):
                        continue
                    tests.append((fixture.name, lambda f=fixture: run_fixture(s, f, shots, Path(tmp))))
                for name, test in tests:
                    try:
                        problems = test()
                    except (AssertionError, WebDriverError) as e:
                        problems = [str(e)]
                    status = "PASS" if not problems else "FAIL"
                    print(f"{status} {name}")
                    for p in problems:
                        print(f"    {p}")
                    failures += bool(problems)
        finally:
            s.close()
    finally:
        driver.terminate()
        try:
            driver.wait(10)
        except subprocess.TimeoutExpired:
            driver.kill()
    print(f"\n{'all UI tests passed' if not failures else f'{failures} UI test(s) failed'}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
