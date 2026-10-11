"""H16: long list copied from Google Sheets and pasted, Spanish headers, Mon-Sat."""
from common import *

FID, SLUG = "H16", "paste-gsheets-spanish-long-list"
README = ("Paste: a long list copied from Google Sheets (tabs, LF) with Spanish headers (Fecha, Empleado, "
          "Obra, Horario), Mon-Sat: Spanish statuses, times on some rows (7-3:30, 7a-11a, 12-3:30, 7-12) "
          "with a split day, one man written 'Paco Rios' on most rows and 'Francisco Rios' on one, a "
          "copied row left behind next to a vacation row, TBD beside a staffed crew, Shop.")

HOURS = {"7-3:30": ("07:00", "15:30"), "7a-11a": ("07:00", "11:00"), "12-3:30": ("12:00", "15:30"),
         "7-12": ("07:00", "12:00")}


def build():
    Mo, Tu, We, Th, Fr, Sa = days("2026-10-26", 6)
    BF, CH, RV = "Brookfield Apts bldg C", "Cedar Hills HOA", "Rancho Verde clubhouse"
    E, L, P, R, S_, T, Ur, Wi, Y = ("Efrén Castro", "Lázaro Méndez", "Paco Ríos", "Rigoberto Soto", "Saúl Herrera",
                                    "Teo Vargas", "Uriel Gómez", "Wilfredo Paz", "Yahir Luna")
    rows = [
        (Mo, E, BF, "7-3:30"), (Mo, L, BF, "7-3:30"), (Mo, P, BF, "7-3:30"),
        (Mo, R, CH, ""), (Mo, S_, CH, ""), (Mo, T, CH, ""),
        (Mo, Ur, RV, ""), (Mo, Wi, RV, ""), (Mo, Y, "Shop", ""),

        (Tu, E, BF, ""), (Tu, L, BF, ""), (Tu, P, BF, ""),
        (Tu, R, CH, ""), (Tu, S_, CH, ""), (Tu, T, "enfermo", ""),
        (Tu, Ur, RV, ""), (Tu, Wi, RV, ""), (Tu, Y, RV, ""),

        (We, E, BF, "7a-11a"), (We, E, RV, "12-3:30"), (We, L, BF, ""), (We, P, BF, ""),
        (We, "Francisco Ríos", CH, ""), (We, R, CH, ""), (We, S_, CH, ""), (We, T, CH, ""),
        (We, Ur, RV, ""), (We, Wi, RV, ""),

        (Th, E, BF, ""), (Th, L, BF, ""), (Th, P, BF, ""),
        (Th, R, CH, ""), (Th, S_, "vacaciones", ""), (Th, T, CH, ""), (Th, "TBD", CH, ""),
        (Th, Ur, RV, ""), (Th, Wi, RV, ""), (Th, Y, RV, ""),

        (Fr, E, BF, ""), (Fr, L, BF, ""), (Fr, P, BF, ""),
        (Fr, R, CH, ""), (Fr, S_, "vacaciones", ""), (Fr, T, CH, ""), (Fr, S_, CH, ""),
        (Fr, Wi, RV, ""), (Fr, Y, "Shop", ""),

        (Sa, E, BF, "7-12"), (Sa, L, BF, "7-12"), (Sa, P, "descanso", ""), (Sa, R, "descanso", ""),
        (Sa, S_, "vacaciones", ""), (Sa, T, "descanso", ""), (Sa, Ur, "descanso", ""), (Sa, Wi, "descanso", ""),
        (Sa, Y, "descanso", ""),
    ]
    sheet = [["Fecha", "Empleado", "Obra", "Horario"]]
    recs = []
    for d, emp, obra, hrs in rows:
        sheet.append([md(d, year=True), emp, obra, hrs])
        if emp.lower() in UNSTAFFED:
            recs.append(U(d, obra))
        elif obra.lower() in STATUS:
            recs.append(S(emp, d, STATUS[obra.lower()]))
        else:
            st, en = HOURS[hrs] if hrs else (None, None)
            recs.append(W(emp, d, obra, start=st, end=en))
    text = csv_text(sheet, delimiter="\t", lineterminator="\n").rstrip("\n")
    d = fixture_dir(FID, SLUG)
    write_bytes(os.path.join(d, "paste.txt"), text.encode("utf-8"))

    flags = [
        flag_db("Francisco Ríos", We, [BF, CH]),
        flag_awo(S_, Fr, [CH], "vacation"),
        flag_nb(Y, We),
        flag_nb(Ur, Fr),
        flag_un(CH, Th),
    ]
    infos = [info("split_day", E, We, [BF, RV])]
    expected = dict(workdays=MON_SAT, layout="long_list", paste=True, assignments=recs, flags=flags, info=infos,
                    must_not_ask=[{"type": "paste_as_table"}])
    meta = dict(window=None, roster=None, person_canon={"Paco Ríos": "Francisco Ríos"}, internal=["Shop"],
                placeholders=[], subs=[])
    return README, "paste.txt", expected, meta
