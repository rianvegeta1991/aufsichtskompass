//! Konnektor fuer **OSCAL-Kataloge** - zunaechst fuer NIST SP 800-53 Rev. 5.
//!
//! Warum der Volltext hier zulaessig ist: NIST-Veroeffentlichungen sind Werke der
//! US-Bundesregierung und gemeinfrei (so auch der Auftrag, Abschnitt 4.1). Die
//! maschinenlesbare Fassung pflegt NIST selbst im Repository `usnistgov/oscal-content`.
//!
//! Aufbau der Quelle (OSCAL 1.x, Katalogmodell):
//! * `catalog.groups[]` sind die Kontrollfamilien (`ac`, `at`, ...) mit `title`.
//! * `groups[].controls[]` sind die Controls (`ac-1`); darin `controls[]` die
//!   Erweiterungen (`ac-2.1`).
//! * Je Control gibt es `parts[]`: `statement` (die Anforderung, verschachtelte `item`s
//!   mit Marke) und `guidance` (Erlaeuterung). Dazu kommen die Bewertungsteile aus
//!   SP 800-53A, die hier bewusst NICHT uebernommen werden.
//! * `params[]` sind Platzhalter, die im Text als `{{ insert: param, ac-1_prm_1 }}` stehen.
//!
//! Die Platzhalter werden als `[Zuweisung: ...]` bzw. `[Auswahl: ...]` ausgeschrieben, so
//! wie sie im gedruckten Katalog stehen - ohne sie waeren die Saetze unlesbar.
//! Der Text bleibt englisch (Sprache `en`).

use crate::modell::{Art, Block, Knoten, Punkt};
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::Value;
use std::collections::BTreeMap;

const KENNUNG: &str = "Aufsichtskompass/0.1 (private Lernanwendung)";

/// Quellenhinweis, der mit jeder uebernommenen Fassung mitgeht.
pub const HINWEIS: &str = "NIST Special Publication 800-53 Revision 5 (OSCAL-Fassung, Quelle: usnistgov/oscal-content). \
     Werk der US-Bundesregierung, gemeinfrei; englischer Originaltext ohne die Bewertungsverfahren aus SP 800-53A. \
     Dient hier als Referenzkatalog für Mappings, nicht als Rechtsvorschrift.";

pub struct Abruf {
    pub etag: Option<String>,
    pub stand: Option<String>,
    pub json: String,
}

pub fn hole(leine: &crate::netz::Leine, url: &str, etag: Option<&str>) -> Result<Option<Abruf>> {
    let klient = leine.klient(KENNUNG, 240)?;
    let mut kopf = Vec::new();
    if let Some(e) = etag {
        kopf.push(("If-None-Match", e));
    }
    let antwort = leine.senden(&klient, url, &kopf)?;
    if antwort.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(None);
    }
    if !antwort.status().is_success() {
        bail!("{url} antwortete mit {}", antwort.status());
    }
    let kopf = |n: &str| {
        antwort.headers().get(n).and_then(|v| v.to_str().ok()).map(|s| s.to_string())
    };
    let etag = kopf("etag");
    let stand = kopf("last-modified");
    let json = antwort.text()?;
    if json.len() < 100_000 {
        bail!("{url} lieferte nur {} Zeichen - vermutlich kein Katalog", json.len());
    }
    Ok(Some(Abruf { etag, stand, json }))
}

pub struct Geparst {
    pub knoten: Vec<Knoten>,
    pub texte: BTreeMap<String, Vec<Block>>,
    pub version: Option<String>,
    /// Zeitpunkt der letzten Aenderung laut Katalog, als ISO-Zeitpunkt.
    pub stand: Option<String>,
}

pub fn parse(json: &str) -> Result<Geparst> {
    let wurzel: Value = serde_json::from_str(json).context("Katalog ist kein gueltiges JSON")?;
    let katalog = wurzel
        .get("catalog")
        .context("kein 'catalog' im Dokument - Quellstruktur hat sich geaendert")?;

    // Platzhalter einsammeln, bevor Text entsteht: Saetze verweisen auf Parameter
    // irgendwo im Katalog.
    let mut roh: BTreeMap<String, Value> = BTreeMap::new();
    parameter_sammeln(katalog, &mut roh);
    let anzeige = anzeigen_bauen(&roh);
    let ersetzer = Ersetzer::neu(anzeige)?;

    let mut knoten = Vec::new();
    let mut texte: BTreeMap<String, Vec<Block>> = BTreeMap::new();
    let gruppen = katalog["groups"].as_array().context("keine 'groups' im Katalog")?;
    for g in gruppen {
        let id = g["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let mut kinder = Vec::new();
        for c in g["controls"].as_array().into_iter().flatten() {
            kinder.push(control(c, &ersetzer, &mut texte));
        }
        knoten.push(Knoten {
            id: format!("fam_{id}"),
            art: Art::Teil,
            nummer: Some(id.to_uppercase()),
            bez: Some(id.to_uppercase()),
            titel: titel_en(g["title"].as_str()),
            pfad: Some(format!("fam/{id}")),
            text: false,
            kinder,
        });
    }
    if texte.len() < 100 {
        bail!(
            "nur {} Controls mit Text gefunden - Quellstruktur pruefen, Daten bleiben unberuehrt",
            texte.len()
        );
    }
    markiere_text(&mut knoten, &texte);

    let meta = &katalog["metadata"];
    Ok(Geparst {
        knoten,
        texte,
        version: meta["version"].as_str().map(String::from),
        stand: meta["last-modified"].as_str().map(iso_zeitpunkt),
    })
}

/// "2026-05-11T16:01:09.00000-00:00" -> "2026-05-11T16:01:09Z"
fn iso_zeitpunkt(s: &str) -> String {
    let kurz: String = s.chars().take(19).collect();
    format!("{kurz}Z")
}

fn titel_en(t: Option<&str>) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    if let Some(t) = t {
        m.insert("en".to_string(), t.to_string());
    }
    m
}

fn markiere_text(kn: &mut [Knoten], texte: &BTreeMap<String, Vec<Block>>) {
    for k in kn {
        k.text = texte.contains_key(&k.id);
        markiere_text(&mut k.kinder, texte);
    }
}

// ----------------------------------------------------------------- Parameter

fn parameter_sammeln(v: &Value, aus: &mut BTreeMap<String, Value>) {
    for p in v["params"].as_array().into_iter().flatten() {
        if let Some(id) = p["id"].as_str() {
            aus.insert(id.to_string(), p.clone());
        }
    }
    for g in v["groups"].as_array().into_iter().flatten() {
        parameter_sammeln(g, aus);
    }
    for c in v["controls"].as_array().into_iter().flatten() {
        parameter_sammeln(c, aus);
    }
}

/// Wie ein Platzhalter im Fliesstext aussieht: Zuweisung oder Auswahl.
fn anzeigen_bauen(roh: &BTreeMap<String, Value>) -> BTreeMap<String, String> {
    let mut aus = BTreeMap::new();
    for (id, p) in roh {
        let text = if let Some(l) = p["label"].as_str() {
            format!("[Zuweisung: {l}]")
        } else if let Some(sel) = p.get("select") {
            let wahl: Vec<String> = sel["choice"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c.as_str())
                .map(|c| nur_text(c, roh))
                .collect();
            let wie = if sel["how-many"].as_str() == Some("one-or-more") {
                "Auswahl, eine oder mehrere"
            } else {
                "Auswahl"
            };
            format!("[{wie}: {}]", wahl.join(" | "))
        } else {
            "[…]".to_string()
        };
        aus.insert(id.clone(), text);
    }
    aus
}

/// Platzhalter in einer Auswahl durch die blosse Bezeichnung ersetzen (eine Ebene tief).
fn nur_text(s: &str, roh: &BTreeMap<String, Value>) -> String {
    let re = Regex::new(r"\{\{\s*insert:\s*param,\s*([^\s}]+)\s*\}\}").unwrap();
    re.replace_all(s, |c: &regex::Captures| {
        roh.get(&c[1])
            .and_then(|p| p["label"].as_str())
            .map(|l| l.to_string())
            .unwrap_or_else(|| "…".to_string())
    })
    .into_owned()
}

struct Ersetzer {
    anzeige: BTreeMap<String, String>,
    platzhalter: Regex,
    link: Regex,
}

impl Ersetzer {
    fn neu(anzeige: BTreeMap<String, String>) -> Result<Ersetzer> {
        Ok(Ersetzer {
            anzeige,
            platzhalter: Regex::new(r"\{\{\s*insert:\s*param,\s*([^\s}]+)\s*\}\}")?,
            link: Regex::new(r"\[([^\]]+)\]\([^)]*\)")?,
        })
    }

    /// Platzhalter ausschreiben und Markdown-Links auf ihren Text reduzieren.
    fn text(&self, s: &str) -> String {
        let ohne_platzhalter = self.platzhalter.replace_all(s, |c: &regex::Captures| {
            self.anzeige.get(&c[1]).cloned().unwrap_or_else(|| "[…]".to_string())
        });
        let ohne_link = self.link.replace_all(&ohne_platzhalter, "$1");
        ohne_link.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

// ------------------------------------------------------------------- Controls

/// Die Marke eines Controls oder einer Aufzaehlung: Eigenschaft `label` ohne `class`
/// (die mit `class` sind Sonderformen wie "AC-01" mit fuehrender Null).
fn marke(v: &Value) -> String {
    let props = v["props"].as_array();
    props
        .into_iter()
        .flatten()
        .find(|p| p["name"] == "label" && p.get("class").is_none())
        .and_then(|p| p["value"].as_str())
        .unwrap_or("")
        .to_string()
}

fn control(c: &Value, ers: &Ersetzer, texte: &mut BTreeMap<String, Vec<Block>>) -> Knoten {
    let id = c["id"].as_str().unwrap_or("").to_string();
    let label = marke(c);
    let mut bloecke: Vec<Block> = Vec::new();

    for teil in c["parts"].as_array().into_iter().flatten() {
        match teil["name"].as_str() {
            Some("statement") => {
                if let Some(pr) = teil["prose"].as_str() {
                    bloecke.push(Block::P { t: ers.text(pr) });
                }
                let punkte: Vec<Punkt> = teil["parts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|p| p["name"] == "item")
                    .map(|p| punkt(p, ers))
                    .collect();
                if !punkte.is_empty() {
                    bloecke.push(Block::Liste { p: punkte });
                }
            }
            Some("guidance") => {
                if let Some(pr) = teil["prose"].as_str() {
                    bloecke.push(Block::P { t: format!("Discussion: {}", ers.text(pr)) });
                }
            }
            // Bewertungsziele und -methoden gehoeren zu SP 800-53A - nicht Teil dieses Katalogs.
            _ => {}
        }
    }

    // Zurueckgezogene Controls: sagen, wohin sie aufgegangen sind.
    let zurueck = c["props"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|p| p["name"] == "status" && p["value"] == "withdrawn");
    if zurueck {
        let ziele: Vec<String> = c["links"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|l| matches!(l["rel"].as_str(), Some("incorporated-into") | Some("moved-to")))
            .filter_map(|l| l["href"].as_str())
            .map(label_aus_href)
            .collect();
        let mut t = "Withdrawn".to_string();
        if !ziele.is_empty() {
            t.push_str(&format!(" – incorporated into or moved to: {}", ziele.join(", ")));
        }
        bloecke.insert(0, Block::P { t });
    }

    // Querverweise auf verwandte Controls: das ist, was Mappings brauchen.
    let verwandt: Vec<String> = c["links"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|l| l["rel"] == "related")
        .filter_map(|l| l["href"].as_str())
        .map(label_aus_href)
        .collect();
    if !verwandt.is_empty() {
        bloecke.push(Block::P { t: format!("Related controls: {}", verwandt.join(", ")) });
    }

    if !bloecke.is_empty() {
        texte.insert(id.clone(), bloecke);
    }

    let kinder: Vec<Knoten> = c["controls"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| control(e, ers, texte))
        .collect();

    Knoten {
        id: id.clone(),
        art: Art::Control,
        nummer: Some(label.clone()),
        bez: Some(label),
        titel: titel_en(c["title"].as_str()),
        pfad: Some(format!("c/{id}")),
        text: false,
        kinder,
    }
}

fn punkt(p: &Value, ers: &Ersetzer) -> Punkt {
    Punkt {
        m: marke(p),
        t: p["prose"].as_str().map(|s| ers.text(s)).unwrap_or_default(),
        u: p["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|x| x["name"] == "item")
            .map(|x| punkt(x, ers))
            .collect(),
    }
}

/// "#ac-2.1" -> "AC-2(1)", "#ac-2" -> "AC-2"
fn label_aus_href(href: &str) -> String {
    let id = href.trim_start_matches('#');
    let gross = id.to_uppercase();
    match gross.split_once('.') {
        Some((a, b)) => format!("{a}({b})"),
        None => gross,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kleiner Katalog mit allem, was der Parser koennen muss. Die Gruppe wird mit
    /// vielen Fuellcontrols aufgefuellt, damit die Mindestzahl von 100 erreicht ist.
    fn katalog() -> String {
        let mut fuell = String::new();
        for i in 100..=210 {
            fuell.push_str(&format!(
                r#",{{"id":"ac-{i}","title":"Control {i}","props":[{{"name":"label","value":"AC-{i}"}}],
                   "parts":[{{"name":"statement","prose":"Do thing {i}."}}]}}"#
            ));
        }
        format!(
            r##"{{"catalog":{{
          "metadata":{{"title":"Probekatalog","version":"5.2.0","last-modified":"2026-05-11T16:01:09.00000-00:00"}},
          "groups":[{{"id":"ac","title":"Access Control","controls":[
            {{"id":"ac-1","title":"Policy and Procedures",
              "props":[{{"name":"label","value":"AC-01","class":"zero-padded"}},{{"name":"label","value":"AC-1"}}],
              "params":[
                {{"id":"ac-1_prm_1","label":"organization-defined personnel or roles"}},
                {{"id":"ac-1_prm_2","select":{{"how-many":"one-or-more","choice":["organization-level","mission-level","{{{{ insert: param, ac-1_prm_1 }}}}"]}}}}],
              "links":[{{"href":"#pm-9","rel":"related"}},{{"href":"#ac-2.1","rel":"related"}}],
              "parts":[
                {{"name":"statement","parts":[
                  {{"id":"ac-1_smt.a","name":"item","props":[{{"name":"label","value":"a."}}],
                    "prose":"Develop and send to {{{{ insert: param, ac-1_prm_1 }}}} at {{{{ insert: param, ac-1_prm_2 }}}}:",
                    "parts":[{{"name":"item","props":[{{"name":"label","value":"1."}}],"prose":"A policy that addresses [scope](#sc-1) and roles."}}]}}]}},
                {{"name":"guidance","prose":"Policy and procedures reflect [applicable laws](https://example.test/x)."}},
                {{"name":"assessment-method","prose":"NICHT UEBERNEHMEN"}}],
              "controls":[
                {{"id":"ac-1.1","title":"Enhancement","props":[{{"name":"label","value":"AC-1(1)"}}],
                  "parts":[{{"name":"statement","prose":"Enhanced."}}]}}]}},
            {{"id":"ac-99","title":"Old control",
              "props":[{{"name":"label","value":"AC-99"}},{{"name":"status","value":"withdrawn"}}],
              "links":[{{"href":"#ac-2.1","rel":"incorporated-into"}}]}}
            {fuell}
          ]}}]}}}}"##
        )
    }

    #[test]
    fn familie_control_und_erweiterung_haengen_ineinander() {
        let g = parse(&katalog()).unwrap();
        assert_eq!(g.version.as_deref(), Some("5.2.0"));
        assert_eq!(g.stand.as_deref(), Some("2026-05-11T16:01:09Z"));
        assert_eq!(g.knoten.len(), 1);
        let fam = &g.knoten[0];
        assert_eq!(fam.pfad.as_deref(), Some("fam/ac"));
        assert_eq!(fam.bez.as_deref(), Some("AC"));
        assert_eq!(fam.titel.get("en").map(String::as_str), Some("Access Control"));
        let c = &fam.kinder[0];
        assert_eq!(c.pfad.as_deref(), Some("c/ac-1"));
        // Die Marke ohne `class` gewinnt gegen die mit fuehrender Null.
        assert_eq!(c.bez.as_deref(), Some("AC-1"));
        assert_eq!(c.kinder[0].pfad.as_deref(), Some("c/ac-1.1"));
        assert_eq!(c.kinder[0].bez.as_deref(), Some("AC-1(1)"));
        assert!(c.text && c.kinder[0].text, "beide tragen Text");
    }

    #[test]
    fn platzhalter_werden_ausgeschrieben_und_links_reduziert() {
        let g = parse(&katalog()).unwrap();
        let b = &g.texte["ac-1"];
        let Block::Liste { p } = &b[0] else { panic!("zuerst die Aufzaehlung: {b:?}") };
        assert_eq!(p[0].m, "a.");
        assert_eq!(
            p[0].t,
            "Develop and send to [Zuweisung: organization-defined personnel or roles] at \
             [Auswahl, eine oder mehrere: organization-level | mission-level | organization-defined personnel or roles]:"
        );
        // verschachtelter Punkt, Markdown-Link auf den Linktext reduziert
        assert_eq!(p[0].u[0].m, "1.");
        assert_eq!(p[0].u[0].t, "A policy that addresses scope and roles.");
        // Erlaeuterung mit Link ebenso
        let Block::P { t } = &b[1] else { panic!() };
        assert_eq!(t, "Discussion: Policy and procedures reflect applicable laws.");
    }

    #[test]
    fn bewertungsteile_bleiben_draussen_und_verwandte_controls_stehen_drin() {
        let g = parse(&katalog()).unwrap();
        let alles: String = g.texte["ac-1"].iter().map(|b| b.nur_text()).collect::<Vec<_>>().join(" ");
        assert!(!alles.contains("NICHT UEBERNEHMEN"));
        assert!(alles.contains("Related controls: PM-9, AC-2(1)"), "{alles}");
    }

    #[test]
    fn zurueckgezogene_controls_nennen_ihr_ziel() {
        let g = parse(&katalog()).unwrap();
        let t = g.texte["ac-99"][0].nur_text();
        assert_eq!(t, "Withdrawn – incorporated into or moved to: AC-2(1)");
    }

    #[test]
    fn fremde_oder_leere_quelle_bricht_ab() {
        assert!(parse("kein json").is_err());
        assert!(parse(r#"{"anders": 1}"#).is_err());
        // Zu wenige Controls: der Bestand darf nicht mit einem Rumpf ueberschrieben werden.
        assert!(parse(r#"{"catalog":{"groups":[{"id":"ac","title":"x","controls":[]}]}}"#).is_err());
    }

    #[test]
    fn hrefs_werden_zu_marken() {
        assert_eq!(label_aus_href("#ac-2"), "AC-2");
        assert_eq!(label_aus_href("#ac-2.13"), "AC-2(13)");
    }
}
