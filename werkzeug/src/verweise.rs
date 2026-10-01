//! Findet die Verweise, die in den Rechtstexten selbst stehen.
//!
//! Das ist der belastbarste Teil der Interdependenz-Analyse: er wird nicht bewertet,
//! sondern **belegt**. Jede gefundene Beziehung traegt die Textstelle mit, aus der sie
//! stammt, und wird nur uebernommen, wenn das Ziel im Bestand tatsaechlich existiert -
//! ein Verweis ins Leere waere schlimmer als gar keiner.
//!
//! Erkannt werden:
//!   * `Artikel 6 Absatz 1` - im selben Regelwerk,
//!   * `Artikel 6 Absatz 1 der Verordnung (EU) 2022/2554` - in einem anderen,
//!   * `§ 23 Absatz 1` - im selben deutschen Gesetz.
//!
//! Die fachlichen Beziehungen (entspricht, konkretisiert, lex specialis, Spannungsfeld)
//! stehen dagegen redaktionell in `daten/beziehungen.json` - die kann kein Muster finden.

use crate::modell::{Knoten, Struktur, Textdatei};

use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize)]
pub struct Verweis {
    pub von: Stelle,
    pub nach: Stelle,
    /// Immer "verweist" - andere Typen sind Auslegung und damit redaktionell.
    pub typ: &'static str,
    pub quelle: &'static str,
    pub konfidenz: f32,
    pub geprueft: bool,
    /// Der Wortlaut, auf dem der Verweis beruht.
    pub beleg: String,
}

#[derive(Serialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stelle {
    pub rw: String,
    pub pfad: String,
}

pub struct Bestand {
    /// Regelwerk -> vorhandene Pfade.
    pub pfade: BTreeMap<String, BTreeSet<String>>,
    /// "2022/2554" -> Regelwerks-Id (aus den CELEX-Nummern des Katalogs).
    pub nach_nummer: BTreeMap<String, String>,
}

/// Baut die Suchhilfen aus Katalog und Strukturen.
pub fn bestand(
    katalog: &serde_json::Value,
    strukturen: &BTreeMap<String, Struktur>,
) -> Bestand {
    let mut pfade = BTreeMap::new();
    for (id, s) in strukturen {
        let mut menge = BTreeSet::new();
        sammle_pfade(&s.knoten, &mut menge);
        pfade.insert(id.clone(), menge);
    }

    // CELEX 32022R2554 -> "2022/2554", 32016R0679 -> "2016/679"
    let celex = Regex::new(r"^3(\d{4})[RLD](\d{4})$").unwrap();
    let mut nach_nummer = BTreeMap::new();
    if let Some(liste) = katalog["regelwerke"].as_array() {
        for r in liste {
            let (Some(id), Some(c)) = (r["id"].as_str(), r["quelle"]["celex"].as_str()) else {
                continue;
            };
            if let Some(m) = celex.captures(c) {
                let jahr = &m[1];
                let nummer = m[2].trim_start_matches('0');
                nach_nummer.insert(format!("{jahr}/{nummer}"), id.to_string());
            }
        }
    }
    Bestand { pfade, nach_nummer }
}

fn sammle_pfade(kn: &[Knoten], aus: &mut BTreeSet<String>) {
    for k in kn {
        if let Some(p) = &k.pfad {
            aus.insert(p.clone());
        }
        sammle_pfade(&k.kinder, aus);
    }
}

pub struct Muster {
    artikel: Regex,
    paragraf: Regex,
    rechtsakt: Regex,
}

impl Muster {
    pub fn neu() -> Self {
        Muster {
            artikel: Regex::new(r"Artikel\s+(\d{1,3}[a-z]?)(?:\s+Absatz\s+(\d{1,3}[a-z]?))?").unwrap(),
            paragraf: Regex::new(r"§\s*(\d{1,3}[a-z]?)(?:\s+Absatz\s+(\d{1,3}[a-z]?))?").unwrap(),
            rechtsakt: Regex::new(
                r"(?:Verordnung|Richtlinie)\s+\(E[UG]\)\s+(?:Nr\.\s*)?(\d{1,4}/\d{2,4})",
            )
            .unwrap(),
        }
    }
}

/// Durchsucht ein Regelwerk und liefert alle belegten Verweise.
pub fn finde(
    rw: &str,
    struktur: &Struktur,
    text: &Textdatei,
    bestand: &Bestand,
    muster: &Muster,
) -> Vec<Verweis> {
    // Kennung -> Pfad, um von der Fundstelle auf ihren Deep-Link zu kommen.
    let mut pfad_zu_id = BTreeMap::new();
    sammle_ids(&struktur.knoten, &mut pfad_zu_id);

    let mut gefunden: BTreeSet<(Stelle, Stelle, String)> = BTreeSet::new();
    let deutsches_gesetz = bestand
        .pfade
        .get(rw)
        .is_some_and(|p| p.iter().any(|x| x.starts_with("par/")));

    for (id, fundstelle) in text {
        let Some(von_pfad) = pfad_zu_id.get(id) else { continue };
        let ganz = fundstelle
            .b
            .iter()
            .map(|b| b.nur_text())
            .collect::<Vec<_>>()
            .join(" ");

        for m in muster.artikel.captures_iter(&ganz) {
            let ganzer = m.get(0).unwrap();
            let ziel_rw = fremdes_regelwerk(&ganz, ganzer.end(), bestand, muster)
                .unwrap_or_else(|| rw.to_string());
            if let Some(v) = bauen(
                rw, von_pfad, &ziel_rw, "art", &m[1], m.get(2).map(|x| x.as_str()), bestand,
                belegtext(&ganz, ganzer.start(), ganzer.end()),
            ) {
                gefunden.insert(v);
            }
        }

        if deutsches_gesetz {
            for m in muster.paragraf.captures_iter(&ganz) {
                let ganzer = m.get(0).unwrap();
                // Paragrafen anderer Gesetze koennte man nur ueber den Gesetzesnamen
                // zuordnen - das waere geraten. Deshalb nur das eigene Gesetz.
                if let Some(v) = bauen(
                    rw, von_pfad, rw, "par", &m[1], m.get(2).map(|x| x.as_str()), bestand,
                    belegtext(&ganz, ganzer.start(), ganzer.end()),
                ) {
                    gefunden.insert(v);
                }
            }
        }
    }

    gefunden
        .into_iter()
        .map(|(von, nach, beleg)| Verweis {
            von,
            nach,
            typ: "verweist",
            quelle: "automatisch aus dem Text",
            konfidenz: 0.9,
            geprueft: false,
            beleg,
        })
        .collect()
}

fn sammle_ids(kn: &[Knoten], aus: &mut BTreeMap<String, String>) {
    for k in kn {
        if let Some(p) = &k.pfad {
            aus.insert(k.id.clone(), p.clone());
        }
        sammle_ids(&k.kinder, aus);
    }
}

/// Steht hinter der Fundstelle ein anderer Rechtsakt ("der Verordnung (EU) 2022/2554")?
fn fremdes_regelwerk(
    text: &str,
    ab: usize,
    bestand: &Bestand,
    muster: &Muster,
) -> Option<String> {
    let rest = &text[ab..grenze(text, ab + 90)];
    let m = muster.rechtsakt.captures(rest)?;
    // Nur wenn der Rechtsakt unmittelbar folgt - sonst gehoert er zu einem anderen Satzteil.
    if m.get(0)?.start() > 40 {
        return None;
    }
    bestand.nach_nummer.get(&m[1]).cloned()
}

// Acht Angaben, aber jede gehoert zur Sache: woher, wohin, welche Art, welche
// Nummer, welcher Absatz, was im Bestand existiert und der Beleg.
#[allow(clippy::too_many_arguments)]
fn bauen(
    von_rw: &str,
    von_pfad: &str,
    nach_rw: &str,
    art: &str,
    nummer: &str,
    absatz: Option<&str>,
    bestand: &Bestand,
    beleg: String,
) -> Option<(Stelle, Stelle, String)> {
    let vorhandene = bestand.pfade.get(nach_rw)?;
    let grob = format!("{art}/{nummer}");
    let fein = absatz.map(|a| format!("{grob}/abs/{a}"));
    // Feinster vorhandener Treffer; gibt es den Absatz nicht, bleibt es beim Artikel.
    let ziel = match fein {
        Some(f) if vorhandene.contains(&f) => f,
        _ if vorhandene.contains(&grob) => grob,
        _ => return None,
    };
    if von_rw == nach_rw && (von_pfad == ziel || von_pfad.starts_with(&format!("{ziel}/"))) {
        return None; // Verweis auf sich selbst
    }
    Some((
        Stelle { rw: von_rw.to_string(), pfad: von_pfad.to_string() },
        Stelle { rw: nach_rw.to_string(), pfad: ziel },
        beleg,
    ))
}

/// Kurzer Ausschnitt um die Fundstelle - als Beleg im Kontext-Panel.
fn belegtext(text: &str, start: usize, ende: usize) -> String {
    let a = text[..start]
        .char_indices()
        .rev()
        .take(45)
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0);
    let b = text[ende..]
        .char_indices()
        .take(55)
        .last()
        .map(|(i, _)| ende + i)
        .unwrap_or(ende);
    let mut s = String::new();
    if a > 0 {
        s.push('…');
    }
    s.push_str(text[a..b].trim());
    if b < text.len() {
        s.push('…');
    }
    s
}

/// Naechstniedrigere Zeichengrenze - die Texte sind UTF-8, ein roher Byte-Schnitt
/// zerlegt sonst Umlaute und Anfuehrungszeichen.
fn grenze(text: &str, bis: usize) -> usize {
    let mut i = bis.min(text.len());
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modell::{Art, Block, Fundstelle, Quelle};

    fn knoten(id: &str, pfad: &str, kinder: Vec<Knoten>) -> Knoten {
        Knoten {
            id: id.into(),
            art: Art::Artikel,
            nummer: None,
            bez: None,
            titel: BTreeMap::new(),
            pfad: Some(pfad.into()),
            text: kinder.is_empty(),
            kinder,
        }
    }

    fn struktur(rw: &str, knoten: Vec<Knoten>) -> Struktur {
        Struktur {
            regelwerk: rw.into(),
            fassung: "2024-01-01".into(),
            abgerufen: "2024-01-01T00:00:00Z".into(),
            quelle: Quelle {
                name: "Probe".into(),
                url: "https://beispiel.test".into(),
                celex: None,
                hinweis: String::new(),
            },
            sprachen: vec!["de".into()],
            knoten,
        }
    }

    fn text(stellen: &[(&str, &str)]) -> Textdatei {
        stellen
            .iter()
            .map(|(id, t)| {
                (id.to_string(), Fundstelle { h: "x".into(), b: vec![Block::P { t: t.to_string() }] })
            })
            .collect()
    }

    /// Zwei Regelwerke: ein EU-Rechtsakt mit Artikeln und ein Gesetz mit Paragrafen.
    fn welt() -> (Bestand, Muster) {
        let katalog = serde_json::json!({ "regelwerke": [
            { "id": "dora", "quelle": { "celex": "32022R2554" } },
            { "id": "dsgvo", "quelle": { "celex": "32016R0679" } },
            { "id": "vag", "quelle": {} },
        ]});
        let mut strukturen = BTreeMap::new();
        strukturen.insert(
            "dora".to_string(),
            struktur("dora", vec![
                knoten("a6", "art/6", vec![knoten("a6_1", "art/6/abs/1", vec![])]),
                knoten("a28", "art/28", vec![]),
            ]),
        );
        strukturen.insert(
            "dsgvo".to_string(),
            struktur("dsgvo", vec![knoten("a32", "art/32", vec![])]),
        );
        strukturen.insert(
            "vag".to_string(),
            struktur("vag", vec![
                knoten("p23", "par/23", vec![]),
                knoten("p26", "par/26", vec![knoten("p26_1", "par/26/abs/1", vec![])]),
            ]),
        );
        (bestand(&katalog, &strukturen), Muster::neu())
    }

    #[test]
    fn celex_wird_zur_rechtsaktnummer() {
        let (b, _) = welt();
        assert_eq!(b.nach_nummer.get("2022/2554").map(String::as_str), Some("dora"));
        // fuehrende Nullen fallen weg: 32016R0679 -> 2016/679
        assert_eq!(b.nach_nummer.get("2016/679").map(String::as_str), Some("dsgvo"));
        assert!(!b.nach_nummer.values().any(|v| v == "vag"), "ohne CELEX kein Eintrag");
    }

    #[test]
    fn verweis_im_eigenen_regelwerk_bis_auf_den_absatz() {
        let (b, m) = welt();
        let s = struktur("dora", vec![knoten("a28", "art/28", vec![])]);
        let t = text(&[("a28", "Die Stellen wenden Artikel 6 Absatz 1 entsprechend an.")]);
        let v = finde("dora", &s, &t, &b, &m);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].von.pfad, "art/28");
        assert_eq!(v[0].nach.rw, "dora");
        assert_eq!(v[0].nach.pfad, "art/6/abs/1", "feinster vorhandener Treffer");
        assert!(v[0].beleg.contains("Artikel 6 Absatz 1"), "der Beleg traegt die Stelle");
        assert_eq!(v[0].typ, "verweist");
    }

    #[test]
    fn nicht_vorhandener_absatz_faellt_auf_den_artikel_zurueck() {
        let (b, m) = welt();
        let s = struktur("dora", vec![knoten("a28", "art/28", vec![])]);
        // Absatz 9 gibt es nicht - dann der Artikel, aber nie ein Verweis ins Leere.
        let t = text(&[("a28", "Siehe Artikel 6 Absatz 9 sowie Artikel 99 Absatz 1.")]);
        let v = finde("dora", &s, &t, &b, &m);
        assert_eq!(v.len(), 1, "Artikel 99 existiert nicht und wird weggelassen");
        assert_eq!(v[0].nach.pfad, "art/6");
    }

    #[test]
    fn fremder_rechtsakt_wird_zugeordnet() {
        let (b, m) = welt();
        let s = struktur("dora", vec![knoten("a28", "art/28", vec![])]);
        let t = text(&[(
            "a28",
            "unberuehrt bleibt Artikel 32 der Verordnung (EU) 2016/679 des Europaeischen Parlaments",
        )]);
        let v = finde("dora", &s, &t, &b, &m);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].nach.rw, "dsgvo", "nicht das eigene Regelwerk");
        assert_eq!(v[0].nach.pfad, "art/32");
    }

    #[test]
    fn rechtsakt_weit_hinter_der_stelle_zaehlt_nicht() {
        let (b, m) = welt();
        let s = struktur("dora", vec![knoten("a28", "art/28", vec![])]);
        // Der Rechtsakt steht in einem anderen Satzteil - dann bleibt es beim eigenen Werk.
        let t = text(&[(
            "a28",
            "Artikel 6 gilt entsprechend; davon unabhaengig bleiben die Befugnisse der Behoerden \
             nach den Vorschriften der Verordnung (EU) 2016/679 bestehen.",
        )]);
        let v = finde("dora", &s, &t, &b, &m);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].nach.rw, "dora");
    }

    #[test]
    fn paragrafen_nur_im_deutschen_gesetz() {
        let (b, m) = welt();
        let s = struktur("vag", vec![knoten("p23", "par/23", vec![])]);
        let t = text(&[("p23", "Im Rahmen des § 26 Absatz 1 ist dies zu beruecksichtigen.")]);
        let v = finde("vag", &s, &t, &b, &m);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].nach.pfad, "par/26/abs/1");

        // Dasselbe Muster in einem EU-Rechtsakt: dort gibt es keine Paragrafen.
        let s2 = struktur("dora", vec![knoten("a28", "art/28", vec![])]);
        let t2 = text(&[("a28", "§ 26 Absatz 1 waere hier nicht zuzuordnen.")]);
        assert!(finde("dora", &s2, &t2, &b, &m).is_empty());
    }

    #[test]
    fn kein_verweis_auf_sich_selbst() {
        let (b, m) = welt();
        let s = struktur("dora", vec![knoten("a6", "art/6", vec![])]);
        let t = text(&[("a6", "Dieser Artikel 6 regelt den Rahmen.")]);
        assert!(finde("dora", &s, &t, &b, &m).is_empty());
    }

    #[test]
    fn beleg_schneidet_an_zeichengrenzen() {
        // Umlaute und Anfuehrungszeichen sind mehrere Bytes lang; ein roher
        // Byte-Schnitt wuerde hier in Panik enden (genau das ist mal passiert).
        let lang = format!("{} Artikel 6 Absatz 1 {}", "Grundsätze „üblich“ ".repeat(6), "Maßnahmen äöüß ".repeat(6));
        let (b, m) = welt();
        let s = struktur("dora", vec![knoten("a28", "art/28", vec![])]);
        let t = text(&[("a28", &lang)]);
        let v = finde("dora", &s, &t, &b, &m);
        assert_eq!(v.len(), 1);
        assert!(v[0].beleg.starts_with('…') && v[0].beleg.ends_with('…'));
        assert!(v[0].beleg.contains("Artikel 6 Absatz 1"));
    }

    #[test]
    fn grenze_schneidet_nie_mitten_im_zeichen() {
        let s = "aä€ß";
        for i in 0..=s.len() + 3 {
            let g = grenze(s, i);
            assert!(s.is_char_boundary(g), "{i} -> {g}");
        }
    }
}
