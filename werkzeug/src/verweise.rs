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
use anyhow::Result;
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
