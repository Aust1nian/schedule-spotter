"""Rendered files re-read with stdlib csv / openpyxl / odfpy / python-docx contain every
expected assignment's person and site string."""

import csv
import io

ENC = {"utf-8": "utf-8", "utf-8-sig": "utf-8-sig", "cp1252": "cp1252", "utf-16-le-bom": "utf-16"}
DELIM = {"comma": ",", "semicolon": ";", "tab": "\t", "pipe": "|"}


def cell_texts(case) -> list[str]:
    r = case.truth["render"]
    name = case.expected["input"]
    data = case.files[name]
    out: list[str] = []
    if r["encoding"]:
        if r["encoding"] == "utf-16-le-bom":
            assert data[:2] == b"\xff\xfe"
        if r["encoding"] == "utf-8-sig":
            assert data[:3] == b"\xef\xbb\xbf"
        text = data.decode(ENC[r["encoding"]])
        for row in csv.reader(io.StringIO(text, newline=""), delimiter=DELIM[r["delimiter"]]):
            out += row
    elif name.endswith(".xlsx"):
        import openpyxl

        wb = openpyxl.load_workbook(io.BytesIO(data))
        for row in wb.worksheets[0].iter_rows(values_only=True):
            out += [v for v in row if isinstance(v, str)]
    elif name.endswith(".ods"):
        from odf import teletype
        from odf.opendocument import load
        from odf.table import TableCell

        doc = load(io.BytesIO(data))
        for tc in doc.spreadsheet.getElementsByType(TableCell):
            ps = [teletype.extractText(p) for p in tc.childNodes]
            out.append("\n".join(ps))
    elif name.endswith(".docx"):
        import docx

        d = docx.Document(io.BytesIO(data))
        out += [p.text for p in d.paragraphs]
        for t in d.tables:
            for row in t.rows:
                out += [c.text for c in row.cells]
    else:  # .xls: no reader in the allowed deps; checked ad hoc with xlrd (see README)
        return []
    return [" ".join(s.split()) for s in out]


def test_every_person_and_site_string_is_in_the_file(cases):
    checked = 0
    for c in cases:
        texts = cell_texts(c)
        if not texts and c.expected["input"].endswith(".xls"):
            continue
        blob = "\n".join(texts)
        for a in c.expected["assignments"]:
            if "person" in a:
                assert a["person"] in blob, (c.seed, a)
            if "site" in a:
                assert a["site"] in blob, (c.seed, a)
        checked += 1
    assert checked > 350


def test_cp1252_files_decode_and_utf16_has_bom(cases):
    for c in cases:
        if c.truth["render"]["encoding"] == "cp1252":
            c.files[c.expected["input"]].decode("cp1252")
