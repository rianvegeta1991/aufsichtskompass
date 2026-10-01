//! Eigene Zusammenfassungen fuer Werke ohne zulaessigen Volltext.
//!
//! Der Kniff: Statt eines zweiten Anzeigewegs werden die Zusammenfassungen in
//! dieselbe Form gebracht wie die Originaltexte - Gliederungsbaum plus Text je
//! Fundstelle. Damit funktionieren Viewer, Deep-Links, Suche, Themenzuordnung und
//! Mappings ohne eine einzige Sonderbehandlung. Was sich unterscheidet, steht im
//! Katalog (`modus: zusammenfassung`) und im Quellenhinweis der Fassung; die App
//! zeigt daraufhin den vorgeschriebenen Hinweis, dass hier **nicht** der
//! Originaltext steht.
//!
//! Jeder Eintrag traegt Kennung, Titel, Zweck, Kernanforderungen, typische
//! Nachweise und Bezuege - so verlangt es der Auftrag fuer die Tiefe "mittel".

use crate::modell::{Art, Block, Fundstelle, Knoten, Punkt, Quelle, Struktur, Textdatei};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct Datei {
    pub regelwerk: String,
    /// Fassung des Werks, auf die sich die Zusammenfassung bezieht.
    pub bezugsfassung: String,
    pub stand: String,
    #[serde(default)]
    pub herkunft: Option<String>,
    #[serde(default)]
    pub geprueft: bool,
    pub quelle: Quellenangabe,
    #[serde(default)]
    pub gruppen: Vec<Gruppe>,
    pub eintraege: Vec<Eintrag>,
}

#[derive(Debug, Deserialize)]
pub struct Quellenangabe {
    pub name: String,
    pub url: String,
}

/// Gliederungsebene, unter der Eintraege haengen (z. B. "A.5 Organisatorische Controls").
#[derive(Debug, Deserialize)]
pub struct Gruppe {
    pub kennung: String,
    pub titel: String,
    /// Redaktionelle Notiz zur Gruppe; die App zeigt sie nicht, sie erklaert
    /// in der Datei, wofuer die Gruppe steht.
    #[serde(default)]
    #[allow(dead_code)]
    pub beschreibung: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Eintrag {
    /// Kennung des Werks, z. B. "A.5.19", "4" oder "APO12".
    pub kennung: String,
    pub titel: String,
    /// Zu welcher Gruppe der Eintrag gehoert (Kennung aus `gruppen`).
    #[serde(default)]
    pub gruppe: Option<String>,
    /// Pfadstueck fuer den Deep-Link, z. B. "a/5.19" oder "kl/4".
    pub pfad: String,
    pub zweck: String,
    #[serde(default)]
    pub anforderungen: Vec<String>,
    #[serde(default)]
    pub nachweise: Vec<String>,
    /// Bezuege zu anderen Regelwerken, im Fliesstext genannt.
    #[serde(default)]
    pub bezuege: Vec<String>,
}

pub struct Gebaut {
    pub struktur: Struktur,
    pub texte: Textdatei,
    pub quelle: Quelle,
    pub stand: String,
}

/// Formt eine Zusammenfassungsdatei in Struktur und Text um.
pub fn bauen(d: &Datei) -> Result<Gebaut> {
    if d.eintraege.is_empty() {
        bail!("[{}] Zusammenfassung ohne Eintraege", d.regelwerk);
    }

    let herkunft = d.herkunft.clone().unwrap_or_else(|| "manuell".into());
    let pruefstand = if d.geprueft {
        "fachlich geprüft"
    } else {
        "ungeprüft"
    };
    let quelle = Quelle {
        name: format!(
            "Eigene Zusammenfassung ({herkunft}, {pruefstand}) – Bezugsfassung: {}",
            d.bezugsfassung
        ),
        url: d.quelle.url.clone(),
        celex: None,
        hinweis: format!(
            "Eigene Zusammenfassung – ersetzt nicht das Original. Maßgeblich ist allein {} ({}).",
            d.quelle.name, d.bezugsfassung
        ),
    };

    // Gruppenknoten anlegen, Eintraege einhaengen.
    let mut gruppen: Vec<Knoten> = d
        .gruppen
        .iter()
        .map(|g| Knoten {
            id: format!("g_{}", schluessel(&g.kennung)),
            art: Art::Abschnitt,
            nummer: Some(g.kennung.clone()),
            bez: Some(g.kennung.clone()),
            titel: eintitel(&g.titel),
            pfad: Some(format!("gr/{}", schluessel(&g.kennung))),
            text: false,
            kinder: Vec::new(),
        })
        .collect();
    let mut ohne_gruppe: Vec<Knoten> = Vec::new();
    let mut texte: Textdatei = BTreeMap::new();

    for e in &d.eintraege {
        let id = format!("z_{}", schluessel(&e.kennung));
        let bloecke = bloecke_fuer(e);
        let text = bloecke
            .iter()
            .map(|b| b.nur_text())
            .collect::<Vec<_>>()
            .join("\n");
        texte.insert(id.clone(), Fundstelle { h: crate::hash_oeffentlich(&text), b: bloecke });

        let knoten = Knoten {
            id,
            art: Art::Control,
            nummer: Some(e.kennung.clone()),
            bez: Some(e.kennung.clone()),
            titel: eintitel(&e.titel),
            pfad: Some(e.pfad.clone()),
            text: true,
            kinder: Vec::new(),
        };
        match e.gruppe.as_ref().and_then(|g| {
            gruppen
                .iter_mut()
                .find(|k| k.nummer.as_deref() == Some(g.as_str()))
        }) {
            Some(g) => g.kinder.push(knoten),
            None => ohne_gruppe.push(knoten),
        }
    }

    // Gruppen ohne Inhalt weglassen - sie wuerden im Baum nur leer herumstehen.
    gruppen.retain(|g| !g.kinder.is_empty());
    let mut knoten = gruppen;
    knoten.extend(ohne_gruppe);

    let struktur = Struktur {
        regelwerk: d.regelwerk.clone(),
        fassung: d.stand.clone(),
        abgerufen: crate::jetzt_oeffentlich(),
        quelle: quelle.clone(),
        sprachen: vec!["de".into()],
        knoten,
    };
    Ok(Gebaut { struktur, texte, quelle, stand: d.stand.clone() })
}

/// Aus einem Eintrag die Textbloecke bauen: Zweck, Kernanforderungen,
/// typische Nachweise, Bezuege.
fn bloecke_fuer(e: &Eintrag) -> Vec<Block> {
    let mut aus = vec![Block::P { t: e.zweck.clone() }];
    if !e.anforderungen.is_empty() {
        aus.push(Block::P { t: "Kernanforderungen:".into() });
        aus.push(Block::Liste {
            p: e
                .anforderungen
                .iter()
                .enumerate()
                .map(|(i, t)| Punkt { m: format!("{}.", i + 1), t: t.clone(), u: Vec::new() })
                .collect(),
        });
    }
    if !e.nachweise.is_empty() {
        aus.push(Block::P {
            t: format!("Typische Nachweise: {}.", e.nachweise.join(", ")),
        });
    }
    if !e.bezuege.is_empty() {
        aus.push(Block::P {
            t: format!("Bezüge: {}", e.bezuege.join(" ")),
        });
    }
    aus
}

fn eintitel(t: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("de".to_string(), t.to_string());
    m
}

/// Kennung in eine Form bringen, die sich als Datei- und Knotenkennung eignet.
fn schluessel(k: &str) -> String {
    k.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect()
}

pub fn lies(pfad: &std::path::Path) -> Result<Datei> {
    let roh = std::fs::read_to_string(pfad).with_context(|| format!("{} lesen", pfad.display()))?;
    serde_json::from_str(&roh).with_context(|| format!("{} auswerten", pfad.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beispiel() -> Datei {
        Datei {
            regelwerk: "iso27001".into(),
            bezugsfassung: "ISO/IEC 27001:2022".into(),
            stand: "2026-10-01".into(),
            herkunft: Some("KI-erstellt".into()),
            geprueft: false,
            quelle: Quellenangabe { name: "ISO/IEC 27001".into(), url: "https://www.iso.org/standard/27001".into() },
            gruppen: vec![Gruppe { kennung: "A.5".into(), titel: "Organisatorische Controls".into(), beschreibung: None }],
            eintraege: vec![Eintrag {
                kennung: "A.5.19".into(),
                titel: "Informationssicherheit in Lieferantenbeziehungen".into(),
                gruppe: Some("A.5".into()),
                pfad: "a/5.19".into(),
                zweck: "Zweck".into(),
                anforderungen: vec!["Erstens".into(), "Zweitens".into()],
                nachweise: vec!["Vertrag".into()],
                bezuege: vec!["Vergleichbar mit Artikel 30 DORA.".into()],
            }],
        }
    }

    #[test]
    fn hinweis_und_pruefstand_stehen_in_der_quelle() {
        let g = bauen(&beispiel()).unwrap();
        assert!(g.quelle.name.contains("KI-erstellt"));
        assert!(g.quelle.name.contains("ungeprüft"));
        assert!(g.quelle.name.contains("ISO/IEC 27001:2022"));
        assert!(g.quelle.hinweis.starts_with("Eigene Zusammenfassung – ersetzt nicht das Original"));
    }

    #[test]
    fn eintrag_wird_zur_fundstelle() {
        let g = bauen(&beispiel()).unwrap();
        // Gruppe mit einem Control darunter
        assert_eq!(g.struktur.knoten.len(), 1);
        let gruppe = &g.struktur.knoten[0];
        assert_eq!(gruppe.kinder.len(), 1);
        let control = &gruppe.kinder[0];
        assert_eq!(control.pfad.as_deref(), Some("a/5.19"));
        assert_eq!(control.bez.as_deref(), Some("A.5.19"));
        let t = g.texte.get(&control.id).unwrap();
        assert!(t.b.len() >= 3, "Zweck, Anforderungen und Nachweise");
    }
}
