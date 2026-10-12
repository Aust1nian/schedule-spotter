"""RECOVERY_CARD.pdf from RECOVERY_CARD.md: one page, US Letter.

Usage: python3 tools/release/recovery_card.py   (needs pandoc, weasyprint and pdfinfo)
"""
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
CSS = """
@page { size: letter; margin: 0.45in 0.5in; }
body { font: 9.6pt/1.32 "DejaVu Sans", "Helvetica", sans-serif; color: #1b1d1f; }
h1 { font-size: 15pt; margin: 0 0 3pt; }
h1 + p { color: #555b61; font-size: 8pt; margin: 0 0 4pt; }
h2 { font-size: 11pt; margin: 7pt 0 2pt; border-bottom: 1px solid #c8cacc; }
p { margin: 2pt 0; }
ul { margin: 2pt 0; padding-left: 13pt; }
li { margin: 1.5pt 0; }
code { font-family: "DejaVu Sans Mono", monospace; font-size: 7.6pt; }
"""


def main():
    md = (ROOT / "RECOVERY_CARD.md").read_text(encoding="utf-8")
    # The note about how the PDF is made is for the repo, not the card.
    md = re.sub(r"^For Tight Line salespeople.*$", "For Tight Line salespeople helping an owner install Schedule Spotter.", md, flags=re.M)
    body = subprocess.run(["pandoc", "-f", "gfm", "-t", "html"], input=md, capture_output=True,
                          text=True, check=True).stdout
    html = f"<!doctype html><html><head><meta charset='utf-8'><title>Schedule Spotter: recovery card</title><style>{CSS}</style></head><body>{body}</body></html>"
    out = ROOT / "RECOVERY_CARD.pdf"
    subprocess.run(["weasyprint", "-", str(out)], input=html.encode(), check=True)
    info = subprocess.run(["pdfinfo", str(out)], capture_output=True, text=True, check=True).stdout
    pages = int(re.search(r"^Pages:\s+(\d+)", info, re.M).group(1))
    if pages != 1:
        sys.exit(f"recovery card: {pages} pages; it must fit on one")
    print(f"recovery card: {out.name}, 1 page")


if __name__ == "__main__":
    main()
