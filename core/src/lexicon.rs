//! Word lists, English and Spanish. Every lookup takes a `text::key`
//! (lowercase, accents removed, whitespace collapsed).

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::model::{Part, Status};

fn word_set(words: &[&'static str]) -> HashSet<&'static str> {
    words.iter().copied().collect()
}

fn data_set(data: &'static str) -> HashSet<String> {
    data.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(crate::text::key)
        .collect()
}

static FIRST_NAMES: LazyLock<HashSet<String>> =
    LazyLock::new(|| data_set(include_str!("../data/first_names.txt")));
static SURNAMES: LazyLock<HashSet<String>> =
    LazyLock::new(|| data_set(include_str!("../data/surnames.txt")));

pub fn is_first_name(key: &str) -> bool {
    FIRST_NAMES.contains(key)
}

pub fn is_surname(key: &str) -> bool {
    SURNAMES.contains(key)
}

/// Status words as a whole cell (after time and part words are removed).
pub fn status_word(key: &str) -> Option<Status> {
    let key = key.trim_matches(|c: char| c == '.' || c == '!' || c == '*');
    Some(match key {
        "off" | "day off" | "dayoff" | "x" | "xx" | "-" | "--" | "—" | "–" | "n/a" | "na"
        | "not working" | "no work" | "libre" | "dia libre" | "descanso" | "rdo" => Status::Off,
        "pto" | "p.t.o" | "personal" | "personal day" | "permiso" => Status::Pto,
        "vac" | "vac." | "vacation" | "vacay" | "vacaciones" | "vacacion" => Status::Vacation,
        "sick" | "sick day" | "ill" | "enfermo" | "enferma" | "incapacidad" => Status::Sick,
        "holiday" | "hol" | "feriado" | "festivo" | "dia festivo" => Status::Holiday,
        "out" | "away" | "unavailable" | "ausente" | "fuera" | "leave" | "loa" | "jury duty"
        | "funeral" => Status::Out,
        _ => return None,
    })
}

/// "TBD", "open", "?", "need guy", "need 1" — an explicitly unstaffed job-day.
pub fn is_unstaffed_marker(key: &str) -> bool {
    static WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
        word_set(&[
            "tbd",
            "tba",
            "t.b.d",
            "t.b.d.",
            "open",
            "?",
            "??",
            "???",
            "unassigned",
            "nobody",
            "no one",
            "vacant",
            "falta",
            "pendiente",
            "por asignar",
            "need guy",
            "need a guy",
            "need help",
            "need crew",
            "need someone",
            "need man",
            "need body",
        ])
    });
    if WORDS.contains(key) {
        return true;
    }
    static NEED: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"^needs?\s+(\d+|a|one|two|three|guy|guys|man|men|help|crew|someone|body|bodies)(\s+(guy|guys|man|men|more|people))?$")
            .expect("valid regex")
    });
    NEED.is_match(key)
}

/// In a job grid: the job isn't running that day.
pub fn is_inactive_marker(key: &str) -> bool {
    matches!(
        key,
        "x" | "xx"
            | "-"
            | "--"
            | "—"
            | "–"
            | "n/a"
            | "na"
            | "no work"
            | "closed"
            | "done"
            | "complete"
            | "completed"
            | "finished"
            | "on hold"
            | "hold"
            | "rain"
            | "rain day"
            | "no"
            | "none"
            | "cerrado"
            | "terminado"
    )
}

pub fn part_word(key: &str) -> Option<Part> {
    Some(match key {
        "am" | "a.m." | "a.m" | "morning" | "am only" | "mornings" | "manana" | "por la manana" => {
            Part::Am
        }
        "pm" | "p.m." | "p.m" | "afternoon" | "pm only" | "tarde" | "por la tarde" => Part::Pm,
        "half" | "half day" | "1/2 day" | "1/2" | "½ day" | "½" | "medio dia" | "half-day" => {
            Part::Half
        }
        _ => return None,
    })
}

/// Shop, yard, office, warehouse: the company's own places.
pub fn is_internal_location(key: &str) -> bool {
    let key = key.strip_prefix("the ").unwrap_or(key);
    matches!(
        key,
        "shop"
            | "yard"
            | "office"
            | "warehouse"
            | "main office"
            | "home office"
            | "shop/yard"
            | "yard/shop"
            | "taller"
            | "bodega"
            | "oficina"
            | "the yard"
            | "storage"
    )
}

/// "drive to Fresno", "travel": counted as internal for flag purposes.
pub fn is_travel(key: &str) -> bool {
    key == "travel"
        || key == "driving"
        || key == "travel day"
        || key.starts_with("drive to ")
        || key.starts_with("travel to ")
        || key.starts_with("driving to ")
        || key.starts_with("viaje")
        || key == "en route"
}

pub static STREET_SUFFIXES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    word_set(&[
        "st",
        "street",
        "ave",
        "av",
        "avenue",
        "rd",
        "road",
        "blvd",
        "boulevard",
        "dr",
        "drive",
        "ln",
        "lane",
        "ct",
        "court",
        "way",
        "pl",
        "place",
        "hwy",
        "highway",
        "pkwy",
        "parkway",
        "cir",
        "circle",
        "ter",
        "terrace",
        "trl",
        "trail",
        "loop",
        "sq",
        "square",
        "pike",
        "calle",
        "avenida",
        "camino",
        "ste",
        "suite",
    ])
});

pub static JOB_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    word_set(&[
        "job",
        "jobs",
        "remodel",
        "reno",
        "renovation",
        "project",
        "kitchen",
        "bath",
        "bathroom",
        "addition",
        "deck",
        "roof",
        "reroof",
        "siding",
        "framing",
        "foundation",
        "basement",
        "garage",
        "fence",
        "patio",
        "porch",
        "demo",
        "demolition",
        "repair",
        "repairs",
        "service",
        "install",
        "installation",
        "punch",
        "punchlist",
        "ti",
        "buildout",
        "build",
        "new",
        "construction",
        "apartments",
        "apts",
        "apt",
        "plaza",
        "school",
        "church",
        "hospital",
        "clinic",
        "store",
        "mall",
        "center",
        "centre",
        "tower",
        "building",
        "bldg",
        "residence",
        "res",
        "house",
        "home",
        "homes",
        "condo",
        "condos",
        "townhomes",
        "townhouse",
        "unit",
        "units",
        "lot",
        "phase",
        "estates",
        "village",
        "ranch",
        "park",
        "bridge",
        "lofts",
        "suites",
        "hotel",
        "motel",
        "restaurant",
        "bank",
        "retail",
        "shell",
        "site",
        "jobsite",
        "obra",
        "casa",
        "proyecto",
        "cocina",
        "bano",
        "remodelacion",
        "subdivision",
        "lots",
        "elementary",
        "high",
        "middle",
        "library",
        "station",
        "dental",
        "medical",
        "office",
        "offices",
        "warehouse",
        "barn",
        "shed",
        "pool",
        "mansion",
        "manor",
        "commons",
        "crossing",
        "landing",
        "point",
        "heights",
        "hills",
        "ridge",
        "creek",
        "lake",
        "valley",
        "gardens",
        "court",
        "square",
        "dealership",
        "starbucks",
        "walgreens",
        "cvs",
        "target",
        "costco",
        "tenant",
        "improvement",
        "drywall",
        "paint",
        "painting",
        "tile",
        "flooring",
        "concrete",
        "slab",
        "pour",
        "excavation",
        "grading",
        "trim",
        "cabinets",
        "windows",
        "doors",
        "gutters",
        "stucco",
        "plumbing",
        "electrical",
        "hvac",
        "rough",
        "finish",
        "inspection",
        "walkthrough",
        "estimate",
        "bid",
    ])
});

/// Tokens that mark a company name ("Delta Electric LLC").
pub fn is_company_token(token: &str) -> bool {
    matches!(
        token,
        "llc"
            | "inc"
            | "co"
            | "corp"
            | "corporation"
            | "company"
            | "ltd"
            | "lp"
            | "llp"
            | "pllc"
            | "enterprises"
            | "electric"
            | "electrical"
            | "plumbing"
            | "hvac"
            | "mechanical"
            | "roofing"
            | "concrete"
            | "drywall"
            | "painting"
            | "construction"
            | "services"
            | "service"
            | "contractors"
            | "contracting"
            | "builders"
            | "excavation"
            | "excavating"
            | "landscaping"
            | "flooring"
            | "glass"
            | "insulation"
            | "masonry"
            | "steel"
            | "welding"
            | "heating"
            | "cooling"
            | "solar"
            | "sprinkler"
            | "paving"
            | "fabrication"
            | "supply"
            | "rentals"
            | "rental"
            | "group"
            | "bros"
            | "brothers"
            | "sons"
            | "and sons"
    )
}

/// The strong company markers: a name with one of these is a company.
pub fn is_legal_suffix(token: &str) -> bool {
    matches!(
        token,
        "llc" | "inc" | "corp" | "corporation" | "ltd" | "lp" | "llp" | "pllc" | "co"
    )
}

pub fn is_placeholder(key: &str) -> bool {
    static PLACEHOLDER: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"^(helper|laborer|labor|laborers|foreman|apprentice|journeyman|carpenter|electrician|plumber|painter|operator|temp|temp worker|new guy|new hire|extra|extra guy|extra man|sub|ayudante|peon|maestro|trabajador|lead|lead carpenter|tech|technician|installer|framer|roofer|finisher|drywaller|mason)(\s*#?\s*\d+)?$",
        )
        .expect("valid regex")
    });
    PLACEHOLDER.is_match(key)
}

/// "Crew A", "Crew 2", "Team Blue", "Framing crew", "Cuadrilla 1".
pub fn crew_name(key: &str) -> bool {
    static CREW: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"^((crew|team|cuadrilla|equipo|grupo)\s*#?\s*[a-z0-9]{1,8}|[a-z]{1,12}\s+(crew|team))$")
            .expect("valid regex")
    });
    CREW.is_match(key)
}

pub fn is_total_label(key: &str) -> bool {
    key == "total"
        || key == "totals"
        || key == "sum"
        || key == "count"
        || key == "headcount"
        || key == "grand total"
        || key.starts_with("total ")
        || key.starts_with("totals ")
        || key.starts_with("# of ")
        || key.starts_with("number of ")
        || key == "subtotal"
        || key.starts_with("total:")
}

pub fn is_roster_title(key: &str) -> bool {
    let key = key.trim_end_matches(':');
    matches!(
        key,
        "roster"
            | "employees"
            | "employee list"
            | "crew list"
            | "crew roster"
            | "staff"
            | "team"
            | "personnel"
            | "our crew"
            | "empleados"
            | "personal"
            | "lista de empleados"
            | "workers"
            | "employee roster"
            | "team members"
            | "crew members"
            | "people"
            | "names"
            | "available"
            | "all employees"
    )
}

pub fn is_crew_roster_title(key: &str) -> bool {
    let key = key.trim_end_matches(':');
    matches!(
        key,
        "crews"
            | "crew assignments"
            | "crew list"
            | "crew roster"
            | "crew members"
            | "who is on each crew"
            | "cuadrillas"
            | "equipos"
            | "teams"
    )
}

/// A column header's role, when the header word is a known synonym.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderRole {
    Person,
    Site,
    Date,
    Day,
    Time,
    Start,
    End,
    Status,
    Note,
    Crew,
    Hours,
    JobNumber,
}

pub fn header_role(key: &str) -> Option<HeaderRole> {
    use HeaderRole::*;
    let key = key.trim_end_matches([':', '.', '#', ' ']).trim();
    Some(match key {
        "name" | "names" | "employee" | "employees" | "emp" | "employee name" | "tech"
        | "techs" | "technician" | "crew member" | "worker" | "workers" | "man" | "guy" | "who"
        | "staff" | "person" | "lead" | "helper" | "installer" | "assigned to" | "assigned"
        | "empleado" | "trabajador" | "nombre" | "tecnico" | "quien" | "foreman" | "laborer"
        | "carpenter" | "crew 1" | "crew 2" | "crew 3" | "crew 4" | "worker 1" | "worker 2"
        | "worker 3" | "man 1" | "man 2" | "man 3" | "tech 1" | "tech 2" | "helper 1"
        | "helper 2" | "employee 1" | "employee 2" | "personal" | "people" => Person,
        "job" | "jobs" | "site" | "jobsite" | "job site" | "project" | "address" | "customer"
        | "client" | "location" | "where" | "job name" | "job/site" | "site/job" | "job / site"
        | "project name" | "obra" | "proyecto" | "trabajo" | "direccion" | "cliente" | "sitio"
        | "lugar" | "ubicacion" | "job location" | "job address" | "customer/job" => Site,
        "date" | "fecha" | "work date" | "day/date" | "date/day" => Date,
        "day" | "weekday" | "dia" | "day of week" => Day,
        "time" | "shift" | "horario" | "hora" | "turno" | "time slot" | "hours (time)"
        | "start-end" | "start - end" => Time,
        "start" | "start time" | "in" | "time in" | "from" | "entrada" | "inicio" | "desde"
        | "begin" | "begins" | "starts" | "start at" | "clock in" | "arrive" | "arrival"
        | "hora de entrada" | "comienzo" => Start,
        "end" | "end time" | "time out" | "to" | "until" | "salida" | "fin" | "hasta"
        | "finish" | "ends" | "clock out" | "leave" | "stop" | "quit" | "depart"
        | "hora de salida" | "termina" => End,
        "status" | "estado" | "off?" | "availability" => Status,
        "notes" | "note" | "comments" | "comment" | "remarks" | "memo" | "details"
        | "description" | "task" | "tasks" | "work" | "scope" | "notas" | "nota"
        | "comentarios" | "tarea" | "instructions" => Note,
        "crew" | "team" | "cuadrilla" | "equipo" => Crew,
        "hours" | "hrs" | "total hours" | "horas" | "hours worked" | "reg" | "ot" => Hours,
        "job #" | "job no" | "job number" | "job num" | "job no." | "#" | "no." | "po" | "wo"
        | "work order" => JobNumber,
        _ => return None,
    })
}

/// English and Spanish nicknames → full first names (Phase 3 matching).
pub static NICKNAMES: LazyLock<HashMap<&'static str, &'static [&'static str]>> =
    LazyLock::new(|| {
        let pairs: &[(&str, &[&str])] = &[
            ("mike", &["michael"]),
            ("mikey", &["michael"]),
            ("mick", &["michael"]),
            ("bob", &["robert"]),
            ("bobby", &["robert"]),
            ("rob", &["robert"]),
            ("robbie", &["robert"]),
            ("bert", &["robert", "albert"]),
            ("bill", &["william"]),
            ("billy", &["william"]),
            ("will", &["william"]),
            ("willy", &["william"]),
            ("liam", &["william"]),
            ("jim", &["james"]),
            ("jimmy", &["james"]),
            ("jamie", &["james"]),
            ("tony", &["anthony", "antonio"]),
            ("dave", &["david"]),
            ("davey", &["david"]),
            ("dan", &["daniel"]),
            ("danny", &["daniel"]),
            ("tom", &["thomas"]),
            ("tommy", &["thomas"]),
            (
                "chris",
                &[
                    "christopher",
                    "christian",
                    "cristian",
                    "christina",
                    "christine",
                ],
            ),
            ("matt", &["matthew"]),
            ("pat", &["patrick", "patricia"]),
            ("rick", &["richard", "ricardo", "eric", "frederick"]),
            ("ricky", &["richard", "ricardo"]),
            ("rich", &["richard"]),
            ("dick", &["richard"]),
            ("steve", &["steven", "stephen"]),
            ("joe", &["joseph", "jose"]),
            ("joey", &["joseph"]),
            ("jeff", &["jeffrey"]),
            ("jerry", &["gerald", "jerome"]),
            ("larry", &["lawrence"]),
            ("gary", &["gerald"]),
            ("terry", &["terrence"]),
            ("ken", &["kenneth"]),
            ("kenny", &["kenneth"]),
            ("ron", &["ronald"]),
            ("ronnie", &["ronald"]),
            ("don", &["donald"]),
            ("al", &["albert", "alan", "alfred", "alberto", "alfredo"]),
            ("alex", &["alexander", "alejandro", "alexis"]),
            ("andy", &["andrew", "andres"]),
            ("ben", &["benjamin"]),
            ("charlie", &["charles"]),
            ("chuck", &["charles"]),
            ("ed", &["edward", "eduardo"]),
            ("eddie", &["edward", "eduardo"]),
            ("frank", &["francis", "francisco"]),
            ("fred", &["frederick"]),
            ("greg", &["gregory"]),
            ("hank", &["henry"]),
            ("jack", &["john"]),
            ("jake", &["jacob"]),
            ("jon", &["jonathan", "john"]),
            ("johnny", &["john"]),
            ("josh", &["joshua"]),
            ("kev", &["kevin"]),
            ("len", &["leonard"]),
            ("lenny", &["leonard"]),
            ("lou", &["louis", "luis"]),
            ("manny", &["manuel"]),
            ("marty", &["martin"]),
            ("nate", &["nathan", "nathaniel"]),
            ("nick", &["nicholas", "nicolas"]),
            ("pete", &["peter", "pedro"]),
            ("phil", &["phillip", "philip"]),
            ("ray", &["raymond", "ramon"]),
            ("randy", &["randall"]),
            ("sam", &["samuel"]),
            ("stan", &["stanley"]),
            ("ted", &["theodore", "edward"]),
            ("tim", &["timothy"]),
            ("vic", &["victor"]),
            ("vince", &["vincent"]),
            ("walt", &["walter"]),
            ("wes", &["wesley"]),
            ("zach", &["zachary"]),
            ("pepe", &["jose"]),
            ("chepe", &["jose"]),
            ("paco", &["francisco"]),
            ("pancho", &["francisco"]),
            ("chuy", &["jesus"]),
            ("chucho", &["jesus"]),
            ("nacho", &["ignacio"]),
            ("lalo", &["eduardo"]),
            ("memo", &["guillermo"]),
            ("beto", &["alberto", "roberto"]),
            ("tono", &["antonio"]),
            ("chava", &["salvador"]),
            ("chema", &["jose maria"]),
            ("goyo", &["gregorio"]),
            ("lupe", &["guadalupe"]),
            ("nando", &["fernando"]),
            ("quique", &["enrique"]),
            ("rafa", &["rafael"]),
        ];
        pairs.iter().map(|(k, v)| (*k, *v)).collect()
    });

pub fn is_month_word(key: &str) -> Option<u32> {
    let key = key.trim_end_matches('.');
    Some(match key {
        "jan" | "january" | "ene" | "enero" => 1,
        "feb" | "february" | "febrero" => 2,
        "mar" | "march" | "marzo" => 3,
        "apr" | "april" | "abr" | "abril" => 4,
        "may" | "mayo" => 5,
        "jun" | "june" | "junio" => 6,
        "jul" | "july" | "julio" => 7,
        "aug" | "august" | "ago" | "agosto" => 8,
        "sep" | "sept" | "september" | "septiembre" | "setiembre" => 9,
        "oct" | "october" | "octubre" => 10,
        "nov" | "november" | "noviembre" => 11,
        "dec" | "december" | "dic" | "diciembre" => 12,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_load() {
        assert!(is_first_name("michael"));
        assert!(is_first_name("jose"));
        assert!(is_first_name("chuy"));
        assert!(is_surname("garcia"));
        assert!(!is_first_name("monday"));
    }

    #[test]
    fn vocab() {
        assert_eq!(status_word("pto"), Some(Status::Pto));
        assert_eq!(status_word("vac"), Some(Status::Vacation));
        assert!(is_unstaffed_marker("need 1"));
        assert!(is_unstaffed_marker("needs 2 guys"));
        assert!(is_unstaffed_marker("tbd"));
        assert!(is_placeholder("helper 1"));
        assert!(is_placeholder("laborer #2"));
        assert!(crew_name("crew a"));
        assert!(crew_name("framing crew"));
        assert_eq!(header_role("empleado"), Some(HeaderRole::Person));
        assert_eq!(header_role("obra"), Some(HeaderRole::Site));
    }
}
