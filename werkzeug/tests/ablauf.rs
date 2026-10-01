//! Ablauftests gegen das fertige Programm (Phase 8).
//!
//! Geprueft wird der Weg, auf dem Daten tatsaechlich in den Bestand kommen:
//! uebernehmen, versionieren, Aenderungen erkennen, ableiten (Index und Verweise),
//! pruefen. Ohne Netz - die Zusammenfassungen sind der einzige Weg, der ohne Abruf
//! auskommt und dennoch dieselbe Versionslogik benutzt wie CELLAR und
//! gesetze-im-internet.de.
//!
//! Jeder Test arbeitet in einem eigenen Ordner unter dem Temp-Verzeichnis und
//! raeumt ihn am Ende weg.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const EXE: &str = env!("CARGO_BIN_EXE_kompass");

struct Werkstatt {
    ordner: PathBuf,
}

impl Drop for Werkstatt {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.ordner);
    }
}

impl Werkstatt {
    /// Legt einen Datenordner mit dem Mindestbestand an.
    fn neu(name: &str) -> Werkstatt {
        let ordner = std::env::temp_dir().join(format!(
            "kompass-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&ordner);
        std::fs::create_dir_all(ordner.join("zusammenfassungen")).unwrap();
        let w = Werkstatt { ordner };

        w.schreib("regelwerke.json", r#"{
          "stand": "2026-10-01",
          "regelwerke": [
            { "id": "probe", "kurzname": "Probewerk", "langtitel": "Probewerk fuer die Ablauftests",
              "typ": "Norm", "herausgeber": "Pruefstelle", "verbindlichkeit": "standard",
              "status": "in Kraft", "modus": "zusammenfassung", "tiefe": "mittel",
              "quelle": { "url": "https://beispiel.test/probewerk" }, "themen": ["probethema"] }
          ]
        }"#);
        w.schreib("quellen.json", r#"{
          "stand": "2026-10-01",
          "erlaubte_hosts": ["beispiel.test"],
          "konnektoren": [],
          "feeds": [{ "id": "probe", "name": "Probe", "url": "https://beispiel.test/feed.xml" }],
          "seiten": []
        }"#);
        w.schreib("themen.json", r#"{
          "stand": "2026-10-01",
          "themen": [{ "id": "probethema", "name": "Probethema", "zielbild": "nur fuer den Test" }]
        }"#);
        w
    }

    fn schreib(&self, name: &str, inhalt: &str) {
        std::fs::write(self.ordner.join(name), inhalt).unwrap();
    }

    fn lies(&self, name: &str) -> String {
        std::fs::read_to_string(self.ordner.join(name)).unwrap()
    }

    fn pfad(&self, name: &str) -> PathBuf {
        self.ordner.join(name)
    }

    fn kompass(&self, args: &[&str]) -> Output {
        let aus = Command::new(EXE)
            .args(args)
            .arg("--daten")
            .arg(&self.ordner)
            .output()
            .expect("kompass laeuft");
        println!(
            "--- kompass {args:?}\n{}{}",
            String::from_utf8_lossy(&aus.stdout),
            String::from_utf8_lossy(&aus.stderr)
        );
        aus
    }

    /// Wie `kompass`, bricht aber ab, wenn der Lauf fehlschlaegt.
    fn muss(&self, args: &[&str]) -> String {
        let aus = self.kompass(args);
        assert!(aus.status.success(), "kompass {args:?} ist fehlgeschlagen");
        String::from_utf8_lossy(&aus.stdout).to_string()
    }
}

/// Eine Zusammenfassung mit frei waehlbarem Stand und Text der ersten Fundstelle.
fn zusammenfassung(stand: &str, erster_zweck: &str, weitere: &str) -> String {
    format!(
        r#"{{
  "regelwerk": "probe",
  "bezugsfassung": "Probewerk Fassung 1",
  "stand": "{stand}",
  "herkunft": "Test",
  "geprueft": false,
  "quelle": {{ "name": "Probewerk", "url": "https://beispiel.test/probewerk" }},
  "gruppen": [{{ "kennung": "A", "titel": "Abschnitt A" }}],
  "eintraege": [
    {{ "kennung": "A.1", "titel": "Erster Eintrag", "gruppe": "A", "pfad": "a/1",
      "zweck": "{erster_zweck}", "anforderungen": ["Erstens", "Zweitens"], "nachweise": ["Nachweis"] }},
    {{ "kennung": "A.2", "titel": "Zweiter Eintrag", "gruppe": "A", "pfad": "a/2",
      "zweck": "Zweiter Zweck.", "anforderungen": ["Drittens"] }},
    {{ "kennung": "A.3", "titel": "Dritter Eintrag", "gruppe": "A", "pfad": "a/3",
      "zweck": "Dritter Zweck mit Verweis auf Artikel 6 Absatz 1." }}{weitere}
  ]
}}"#
    )
}

fn json(p: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn erstaufnahme_dann_neue_fassung_mit_erkannten_aenderungen() {
    let w = Werkstatt::neu("fassungen");
    w.schreib(
        "zusammenfassungen/probe.json",
        &zusammenfassung("2026-01-01", "Erster Zweck.", ""),
    );

    // --- Erstaufnahme
    let aus = w.muss(&["zusammenfassungen"]);
    assert!(aus.contains("3 Fundstellen"), "drei Eintraege uebernommen");
    let register = json(&w.pfad("rw/probe/fassungen.json"));
    assert_eq!(register["fassungen"].as_array().unwrap().len(), 1);
    assert_eq!(register["fassungen"][0]["id"], "2026-01-01");
    assert_eq!(register["fassungen"][0]["fundstellen"], 3);
    let aend = json(&w.pfad("aenderungen.json"));
    assert_eq!(aend["ereignisse"][0]["art"], "Erstaufnahme");

    // Bestand und Suchindex sind mit abgeleitet worden.
    assert_eq!(json(&w.pfad("bestand.json"))["bestand"]["probe"]["fassung"]["id"], "2026-01-01");
    assert!(w.pfad("rw/probe/2026-01-01/index-de.json").exists());

    // --- Zweiter Lauf ohne Aenderung: keine zweite Fassung
    w.muss(&["zusammenfassungen"]);
    let register = json(&w.pfad("rw/probe/fassungen.json"));
    assert_eq!(
        register["fassungen"].as_array().unwrap().len(),
        1,
        "gleicher Inhalt - keine neue Fassung"
    );

    // --- Neue Bezugsfassung: ein Eintrag geaendert, einer neu
    w.schreib(
        "zusammenfassungen/probe.json",
        &zusammenfassung(
            "2026-06-30",
            "Erster Zweck, nun anders formuliert.",
            r#",
    { "kennung": "A.4", "titel": "Vierter Eintrag", "gruppe": "A", "pfad": "a/4",
      "zweck": "Vierter Zweck." }"#,
        ),
    );
    let aus = w.muss(&["zusammenfassungen"]);
    assert!(aus.contains("1 neu"), "A.4 ist neu: {aus}");
    assert!(aus.contains("1 geaendert"), "A.1 hat neuen Text: {aus}");

    let register = json(&w.pfad("rw/probe/fassungen.json"));
    let fassungen = register["fassungen"].as_array().unwrap();
    assert_eq!(fassungen.len(), 2);
    assert_eq!(fassungen[1]["id"], "2026-06-30");
    assert_eq!(fassungen[1]["fundstellen"], 4);
    assert_ne!(fassungen[0]["hash"], fassungen[1]["hash"], "Gesamthash unterscheidet sich");

    // Die alte Fassung bleibt unveraendert liegen - das ist der Kern des Archivs.
    assert!(w.pfad("rw/probe/2026-01-01/text-de.json").exists());
    let alt = json(&w.pfad("rw/probe/2026-01-01/text-de.json"));
    let alt_erste = alt.as_object().unwrap().values().next().unwrap();
    assert!(
        serde_json::to_string(alt_erste).unwrap().contains("Erster Zweck."),
        "alte Fassung traegt noch den alten Wortlaut"
    );

    // Aenderungsereignis nennt die betroffenen Fundstellen (Ereignisse werden angehaengt).
    let aend = json(&w.pfad("aenderungen.json"));
    let liste = aend["ereignisse"].as_array().unwrap();
    assert_eq!(liste.len(), 2, "Erstaufnahme und neue Fassung");
    let e = liste.last().unwrap();
    assert_eq!(e["art"], "neue Fassung");
    assert_eq!(e["vorherige"], "2026-01-01");
    assert_eq!(e["neu"].as_array().unwrap().len(), 1);
    assert_eq!(e["geaendert"].as_array().unwrap().len(), 1);
    // Leere Listen stehen gar nicht in der Datei - "entfallen" fehlt also, wenn
    // nichts entfallen ist.
    assert!(e["entfallen"].as_array().is_none_or(|l| l.is_empty()));
}

#[test]
fn kaputte_quelle_ueberschreibt_den_bestand_nicht() {
    let w = Werkstatt::neu("kaputt");
    w.schreib(
        "zusammenfassungen/probe.json",
        &zusammenfassung("2026-01-01", "Erster Zweck.", ""),
    );
    w.muss(&["zusammenfassungen"]);
    let vorher = w.lies("rw/probe/2026-01-01/text-de.json");
    let register_vorher = w.lies("rw/probe/fassungen.json");

    // Die Quelle liefert nur noch einen Bruchteil - genau der Fall, in dem
    // nichts uebernommen werden darf (Qualitaetsanforderung des Auftrags).
    w.schreib(
        "zusammenfassungen/probe.json",
        r#"{ "regelwerk": "probe", "bezugsfassung": "Probewerk Fassung 1", "stand": "2026-09-09",
             "quelle": { "name": "Probewerk", "url": "https://beispiel.test/probewerk" },
             "eintraege": [ { "kennung": "A.1", "titel": "Rest", "pfad": "a/1", "zweck": "Rest." } ] }"#,
    );
    let aus = w.kompass(&["zusammenfassungen"]);
    assert!(!aus.status.success(), "der Lauf muss abbrechen");
    let meldung = String::from_utf8_lossy(&aus.stderr).to_string()
        + &String::from_utf8_lossy(&aus.stdout);
    assert!(meldung.contains("Fundstellen"), "die Meldung nennt den Grund: {meldung}");

    assert_eq!(w.lies("rw/probe/2026-01-01/text-de.json"), vorher, "Text unberuehrt");
    assert_eq!(w.lies("rw/probe/fassungen.json"), register_vorher, "Register unberuehrt");
    assert!(!w.pfad("rw/probe/2026-09-09").exists(), "keine halbe Fassung angelegt");
}

#[test]
fn erzwingen_schreibt_dieselbe_fassung_neu_statt_eine_zu_erfinden() {
    let w = Werkstatt::neu("erzwingen");
    w.schreib(
        "zusammenfassungen/probe.json",
        &zusammenfassung("2026-01-01", "Erster Zweck.", ""),
    );
    w.muss(&["zusammenfassungen"]);

    // Gleicher Stand, anderer Text: so verhaelt sich eine Parserkorrektur.
    w.schreib(
        "zusammenfassungen/probe.json",
        &zusammenfassung("2026-01-01", "Erster Zweck, korrigiert.", ""),
    );
    let aus = w.muss(&["zusammenfassungen"]);
    assert!(aus.contains("neu erzeugt"), "an Ort und Stelle ersetzt: {aus}");
    let register = json(&w.pfad("rw/probe/fassungen.json"));
    assert_eq!(
        register["fassungen"].as_array().unwrap().len(),
        1,
        "keine erfundene zweite Fassung"
    );
    let text = w.lies("rw/probe/2026-01-01/text-de.json");
    assert!(text.contains("korrigiert"));
}

#[test]
fn pruefen_erkennt_falsche_zuordnungen_und_adressen() {
    let w = Werkstatt::neu("pruefen");
    w.schreib(
        "zusammenfassungen/probe.json",
        &zusammenfassung("2026-01-01", "Erster Zweck.", ""),
    );
    w.muss(&["zusammenfassungen"]);

    // Saubere Lage: Zuordnung auf einen vorhandenen Pfad.
    w.schreib(
        "themenzuordnung.json",
        r#"{ "stand": "2026-10-01", "zuordnungen": [{ "rw": "probe", "pfad": "a/1", "themen": ["probethema"] }] }"#,
    );
    let aus = w.muss(&["pruefen"]);
    assert!(aus.contains("Pruefung ohne Befund"), "{aus}");
    assert!(aus.contains("erlaubte Hosts"), "die Leine wird mitgeprueft: {aus}");

    // Pfad, den es nicht gibt, unbekanntes Thema, Feed ausserhalb der Leine.
    w.schreib(
        "themenzuordnung.json",
        r#"{ "stand": "2026-10-01", "zuordnungen": [
             { "rw": "probe", "pfad": "a/99", "themen": ["probethema"] },
             { "rw": "probe", "pfad": "a/1", "themen": ["gibtesnicht"] }] }"#,
    );
    w.schreib("quellen.json", r#"{
      "stand": "2026-10-01",
      "erlaubte_hosts": ["beispiel.test"],
      "konnektoren": [],
      "feeds": [{ "id": "fremd", "name": "Fremd", "url": "https://woanders.test/feed.xml" }],
      "seiten": []
    }"#);
    let aus = w.kompass(&["pruefen"]);
    assert!(!aus.status.success(), "mit Befunden endet der Lauf mit Fehler");
    let alles = String::from_utf8_lossy(&aus.stdout).to_string()
        + &String::from_utf8_lossy(&aus.stderr);
    assert!(alles.contains("a/99"), "fehlender Pfad wird benannt: {alles}");
    assert!(alles.contains("gibtesnicht"), "unbekanntes Thema wird benannt");
    assert!(alles.contains("woanders.test"), "Feed ausserhalb der Leine wird benannt");
}

#[test]
fn lernstand_markiert_geaenderte_fundstellen_zur_pruefung() {
    let w = Werkstatt::neu("lernstand");
    w.schreib(
        "zusammenfassungen/probe.json",
        &zusammenfassung("2026-01-01", "Erster Zweck.", ""),
    );
    w.muss(&["zusammenfassungen"]);
    std::fs::create_dir_all(w.pfad("lernen")).unwrap();
    w.schreib(
        "lernen/lektionen.json",
        r#"{ "lektionen": [{ "id": "l1", "titel": "Probe", "abschnitte": [
             { "titel": "Erster Abschnitt", "fundstellen": [{ "rw": "probe", "pfad": "a/1" }] }] }] }"#,
    );

    // Erster Lauf: die Fundstelle wird erfasst.
    let aus = w.muss(&["lernstand", "--bestaetigen"]);
    assert!(aus.contains("erfasst") || aus.contains("1"), "{aus}");
    assert!(w.pfad("lernen/pruefstand.json").exists());

    // Nichts geaendert: alles aktuell.
    let aus = w.muss(&["lernstand"]);
    assert!(aus.contains("0 geaendert -> zu pruefen"), "ohne Aenderung keine Meldung: {aus}");

    // Neue Fassung mit geaendertem Text an genau dieser Fundstelle.
    w.schreib(
        "zusammenfassungen/probe.json",
        &zusammenfassung("2026-07-01", "Erster Zweck, neu gefasst.", ""),
    );
    w.muss(&["zusammenfassungen"]);
    let aus = w.muss(&["lernstand"]);
    assert!(
        aus.contains("1 geaendert -> zu pruefen"),
        "die Lektion muss zur Pruefung gemeldet werden: {aus}"
    );
    let stand = json(&w.pfad("lernen/pruefstand.json"));
    let eintrag = stand["eintraege"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["schluessel"].as_str().unwrap_or("").contains("l1/abschnitt-1"))
        .expect("die Lektion steht im Pruefstand");
    assert_eq!(eintrag["status"], "zu pruefen");

    // Nach der Bestaetigung ist der neue Stand der maßgebliche.
    w.muss(&["lernstand", "--bestaetigen"]);
    let stand = json(&w.pfad("lernen/pruefstand.json"));
    let eintrag = &stand["eintraege"][0];
    assert_eq!(eintrag["status"], "aktuell");
}

#[test]
fn screening_ohne_quellen_fuehrt_protokoll_und_ruft_nichts_ab() {
    let w = Werkstatt::neu("screening");
    w.schreib(
        "zusammenfassungen/probe.json",
        &zusammenfassung("2026-01-01", "Erster Zweck.", ""),
    );
    w.muss(&["zusammenfassungen"]);
    // Keine Feeds, keine Seiten - und die Leine kennt nur beispiel.test, der
    // SPARQL-Dienst steht also nicht darauf. Damit geht in diesem Test kein
    // einziger Abruf hinaus (die Qualitaetsanforderung verlangt Tests ohne Netz).
    w.schreib("quellen.json", r#"{
      "stand": "2026-10-01",
      "erlaubte_hosts": ["beispiel.test"],
      "konnektoren": [], "feeds": [], "seiten": []
    }"#);
    w.schreib(
        "taxonomie.json",
        r#"{ "schwelle": 8, "ausschluss": [], "begriffe": [
             { "wort": "DORA", "gewicht": 10, "themen": ["probethema"], "regelwerke": ["probe"] }] }"#,
    );

    let aus = w.muss(&["screening"]);
    assert!(aus.contains("0 Meldungen") || aus.contains("0 neu") || aus.contains("Lauf"), "{aus}");
    let laeufe = json(&w.pfad("screening-laeufe.json"));
    let liste = laeufe["laeufe"].as_array().unwrap();
    assert_eq!(liste.len(), 1, "ein Lauf protokolliert");
    let quellen = liste[0]["quellen"].as_array().unwrap();
    let sparql = quellen
        .iter()
        .find(|q| q["quelle"] == "eurlex-folgeakte")
        .expect("der SPARQL-Schritt steht im Protokoll");
    assert_eq!(sparql["status"], "Fehler");
    assert!(
        sparql["fehler"].as_str().unwrap_or("").contains("erlaubte_hosts"),
        "der Grund ist die Leine, nicht das Netz: {}",
        sparql["fehler"]
    );
}

#[test]
fn zeitfenster_haelt_den_taeglichen_lauf_zurueck() {
    let w = Werkstatt::neu("zeitfenster");
    // Drei Stunden entfernt: der Lauf darf nicht starten. Der Cron feuert zweimal
    // (Sommer- und Winterzeit), deshalb entscheidet das Werkzeug selbst.
    let jetzt = chrono::Utc::now().with_timezone(&chrono_tz::Europe::Berlin);
    let spaeter = jetzt + chrono::Duration::hours(3);
    let aus = w.muss(&["screening", "--nur-um", &spaeter.format("%H:%M").to_string()]);
    assert!(aus.contains("Ausserhalb des Zeitfensters"), "{aus}");
    assert!(!w.pfad("screening-laeufe.json").exists(), "kein Lauf protokolliert");
}

#[test]
fn hilfe_nennt_alle_befehle() {
    let w = Werkstatt::neu("hilfe");
    let aus = w.muss(&[]);
    for befehl in ["abruf", "index", "verweise", "screening", "lernstand", "zusammenfassungen", "pruefen"] {
        assert!(aus.contains(befehl), "Befehl {befehl} fehlt in der Hilfe");
    }
    // Unbekanntes zeigt ebenfalls die Hilfe, statt stumm zu bleiben.
    assert!(w.muss(&["quatsch"]).contains("kompass <befehl>"));
}

#[test]
fn index_und_verweise_werden_abgeleitet() {
    let w = Werkstatt::neu("ableiten");
    w.schreib(
        "zusammenfassungen/probe.json",
        &zusammenfassung("2026-01-01", "Erster Zweck.", ""),
    );
    w.muss(&["zusammenfassungen"]);
    w.muss(&["index"]);
    w.muss(&["verweise"]);

    // Suchindex: ein Stamm aus dem Text findet die Fundstelle wieder.
    let index = json(&w.pfad("rw/probe/2026-01-01/index-de.json"));
    assert!(index["dokumente"].as_array().unwrap().len() >= 3);
    assert!(
        index["worte"].as_object().unwrap().keys().any(|k| k.starts_with("zweck")),
        "Stamm aus dem Zwecktext fehlt"
    );
    let verzeichnis = json(&w.pfad("suchindex.json"));
    assert!(
        serde_json::to_string(&verzeichnis).unwrap().contains("probe"),
        "das Regelwerk steht im Index-Verzeichnis"
    );

    // Verweise: "Artikel 6 Absatz 1" zeigt hier ins Leere - es gibt keinen Artikel 6.
    // Genau das ist die Regel: nur belegte Ziele werden uebernommen.
    let v = json(&w.pfad("verweise.json"));
    assert_eq!(v["verweise"].as_array().unwrap().len(), 0);
}
