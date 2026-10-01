//! Baut den Suchindex, den die App im Browser auswertet (BM25).
//!
//! Wichtig: **Die Wortnormalisierung muss auf beiden Seiten identisch sein.**
//! Dieselben Regeln stehen in `suche.js` als `stamm()`; aendert sich eine Regel hier,
//! muss sie dort mitgeaendert werden - sonst findet die Suche indexierte Worte nicht
//! mehr. Die Testfaelle unten und die Liste in `suche.js` halten das zusammen.
//!
//! Absichtlich kein Snowball-Stemmer: der liesse sich im Browser nicht ohne
//! zusaetzliche Abhaengigkeit nachbilden, und eine Abweichung zwischen Index- und
//! Abfrageseite waere schlimmer als eine etwas groebere Stammbildung.

use crate::modell::{Knoten, Struktur, Textdatei};
use serde::Serialize;
use std::collections::BTreeMap;

/// Worte, die in fast jedem Satz stehen und den Index nur aufblaehen.
const STOPP: &[&str] = &[
    // deutsch
    "der", "die", "das", "des", "dem", "den", "ein", "eine", "einer", "eines", "einem", "einen",
    "und", "oder", "aber", "auch", "als", "am", "an", "auf", "aus", "bei", "bis", "durch", "für",
    "fur", "gegen", "im", "in", "ist", "sind", "mit", "nach", "nicht", "von", "vor", "zu", "zum",
    "zur", "über", "uber", "unter", "sowie", "dass", "dieser", "diese", "dieses", "werden", "wird",
    "kann", "können", "konnen", "muss", "müssen", "mussen", "soll", "sollen", "haben", "hat",
    "ihre", "ihrer", "ihren", "ihr", "sich", "sie", "es", "er", "wenn", "wie", "so", "einschließlich",
    // englisch
    "the", "and", "or", "of", "to", "in", "for", "on", "with", "by", "as", "at", "from", "that",
    "this", "these", "those", "be", "is", "are", "was", "were", "shall", "may", "which", "such",
    "any", "all", "their", "its", "not", "other", "where", "when",
];

/// Ein Wort auf seinen Suchstamm bringen. Rueckgabe `None` = nicht indexieren.
pub fn stamm(wort: &str) -> Option<String> {
    let klein: String = wort
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'ä' => 'a',
            'ö' => 'o',
            'ü' => 'u',
            'á' | 'à' | 'â' => 'a',
            'é' | 'è' | 'ê' => 'e',
            'í' | 'ì' => 'i',
            'ó' | 'ò' | 'ô' => 'o',
            'ú' | 'ù' | 'û' => 'u',
            'ç' => 'c',
            _ => c,
        })
        .collect();
    let klein = klein.replace('ß', "ss");
    if klein.len() < 2 || STOPP.contains(&klein.as_str()) {
        return None;
    }
    let s = kuerze(&klein);
    if s.len() < 2 || STOPP.contains(&s.as_str()) {
        return None;
    }
    Some(s)
}

/// Sehr schlanke deutsche/englische Stammbildung - bewusst wenige, gut vorhersagbare Regeln.
fn kuerze(w: &str) -> String {
    // Zahlen bleiben unangetastet ("2022", "2554", "27001").
    if w.chars().all(|c| c.is_ascii_digit()) {
        return w.to_string();
    }
    let mut s = w.to_string();
    // Substantivierung: Meldung/Meldungen -> meld
    if s.len() > 7 && s.ends_with("ungen") {
        s.truncate(s.len() - 5);
    } else if s.len() > 5 && s.ends_with("ung") {
        s.truncate(s.len() - 3);
    } else if s.len() > 5 && s.ends_with("ies") {
        // englischer Plural: policies -> policy
        s.truncate(s.len() - 3);
        s.push('y');
        return s;
    }
    // Flexionsendungen, laengste zuerst; Stamm bleibt mindestens vier Zeichen lang.
    // Zwei Durchgaenge, damit Plural *und* Beugung fallen: "Registers" -> "regist".
    for _ in 0..2 {
        for suf in ["ern", "est", "end", "ing", "em", "er", "en", "es", "et", "ed", "e", "s", "n"] {
            if s.len() >= suf.len() + 4 && s.ends_with(suf) {
                s.truncate(s.len() - suf.len());
                break;
            }
        }
    }
    // Doppelkonsonant am Ende glaetten (Risikoo -> Risiko ist nicht gemeint,
    // wohl aber "kommt"/"komm" -> gleicher Stamm).
    let b: Vec<char> = s.chars().collect();
    if b.len() > 4 && b[b.len() - 1] == b[b.len() - 2] && "nmtlrsfp".contains(b[b.len() - 1]) {
        s.truncate(s.len() - b[b.len() - 1].len_utf8());
    }
    s
}

/// Zerlegt Text in Suchstaemme.
pub fn worte(text: &str) -> Vec<String> {
    let mut aus = Vec::new();
    for roh in text.split(|c: char| !c.is_alphanumeric()) {
        if roh.is_empty() {
            continue;
        }
        if let Some(s) = stamm(roh) {
            aus.push(s);
        }
    }
    aus
}

// ------------------------------------------------------------------ Indexdatei

#[derive(Serialize)]
pub struct Index {
    pub regelwerk: String,
    pub sprache: String,
    pub fassung: String,
    /// Mittlere Dokumentlaenge - BM25 braucht sie zur Laengennormierung.
    pub avgdl: f64,
    pub dokumente: Vec<Dokument>,
    /// Stamm -> Liste aus [Dokumentnummer, Haeufigkeit].
    pub worte: BTreeMap<String, Vec<[u32; 2]>>,
}

#[derive(Serialize)]
pub struct Dokument {
    /// Kennung der Fundstelle (z. B. `028.001`).
    pub id: String,
    /// Deep-Link-Pfad (z. B. `art/28/abs/1`).
    pub pfad: String,
    /// Menschliche Bezeichnung (z. B. "Artikel 28 Absatz 1 - Allgemeine Prinzipien").
    pub b: String,
    /// Anfang des Textes fuer die Trefferliste.
    pub t: String,
    /// Laenge in Suchstaemmen.
    pub l: u32,
}

/// Baut den Index einer Sprachfassung.
pub fn baue(struktur: &Struktur, text: &Textdatei, sprache: &str) -> Index {
    let mut pfade: BTreeMap<String, (String, String)> = BTreeMap::new();
    sammle_bezeichnungen(&struktur.knoten, sprache, &[], &mut pfade);

    let mut dokumente = Vec::new();
    let mut postings: BTreeMap<String, Vec<[u32; 2]>> = BTreeMap::new();
    let mut summe = 0usize;

    for (id, fundstelle) in text {
        let (pfad, bez) = pfade
            .get(id)
            .cloned()
            .unwrap_or_else(|| (id.clone(), id.clone()));
        let voll = fundstelle
            .b
            .iter()
            .map(|b| b.nur_text())
            .collect::<Vec<_>>()
            .join(" ");
        // Die Bezeichnung mitindexieren: wer "Artikel 28" sucht, soll ihn finden.
        let mut zaehler: BTreeMap<String, u32> = BTreeMap::new();
        let mut laenge = 0u32;
        for w in worte_mit_gewicht(&bez, 2).into_iter().chain(worte(&voll)) {
            *zaehler.entry(w).or_insert(0) += 1;
            laenge += 1;
        }
        if laenge == 0 {
            continue;
        }
        let nr = dokumente.len() as u32;
        for (w, n) in zaehler {
            postings.entry(w).or_default().push([nr, n]);
        }
        summe += laenge as usize;
        let mut teaser: String = voll.chars().take(220).collect();
        if voll.chars().count() > 220 {
            teaser.push('…');
        }
        dokumente.push(Dokument {
            id: id.clone(),
            pfad,
            b: bez,
            t: teaser,
            l: laenge,
        });
    }

    let avgdl = if dokumente.is_empty() {
        0.0
    } else {
        (summe as f64 / dokumente.len() as f64 * 100.0).round() / 100.0
    };
    Index {
        regelwerk: struktur.regelwerk.clone(),
        sprache: sprache.to_string(),
        fassung: struktur.fassung.clone(),
        avgdl,
        dokumente,
        worte: postings,
    }
}

fn worte_mit_gewicht(text: &str, mal: usize) -> Vec<String> {
    let einmal = worte(text);
    let mut aus = Vec::with_capacity(einmal.len() * mal);
    for _ in 0..mal {
        aus.extend(einmal.iter().cloned());
    }
    aus
}

/// Baut je Fundstelle Pfad und lesbare Bezeichnung aus dem Gliederungsbaum.
fn sammle_bezeichnungen(
    knoten: &[Knoten],
    sprache: &str,
    eltern: &[String],
    aus: &mut BTreeMap<String, (String, String)>,
) {
    for k in knoten {
        let (de, en) = k.art.wort();
        let wort = if sprache == "en" { en } else { de };
        // `bez` ist die Beschriftung der Quelle ("§ 23", "Erstes Buch") und geht vor,
        // weil sich aus Art und Nummer sonst "Abschnitt Zweiter" ergeben wuerde.
        let kopf = k.bez.clone().unwrap_or_else(|| match &k.nummer {
            Some(n) => format!("{wort} {n}"),
            None => wort.to_string(),
        });
        let eigen = match k.titel.get(sprache) {
            Some(t) => format!("{kopf} – {t}"),
            None => kopf,
        };
        let mut kette: Vec<String> = eltern.to_vec();
        // Absaetze erben die Artikelbezeichnung, Artikel stehen fuer sich.
        let bez = match k.art {
            crate::modell::Art::Absatz => format!(
                "{} {}",
                eltern.last().cloned().unwrap_or_default(),
                eigen
            )
            .trim()
            .to_string(),
            _ => eigen.clone(),
        };
        if let Some(pfad) = &k.pfad {
            aus.insert(k.id.clone(), (pfad.clone(), bez.clone()));
        }
        if matches!(
            k.art,
            crate::modell::Art::Artikel
                | crate::modell::Art::Paragraf
                | crate::modell::Art::Control
                | crate::modell::Art::Erwaegungsgrund
        ) {
            kette = vec![eigen.clone()];
        }
        sammle_bezeichnungen(&k.kinder, sprache, &kette, aus);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Diese Paare stehen genauso in `suche.js` - beide Seiten muessen sie gleich loesen.
    #[test]
    fn staemme_wie_im_browser() {
        let proben = [
            ("Informationsregister", "informationsregist"),
            ("Informationsregisters", "informationsregist"),
            ("Meldungen", "meld"),
            ("Meldung", "meld"),
            ("Auslagerung", "auslag"),
            ("Auslagerungen", "auslag"),
            ("IKT", "ikt"),
            ("Drittparteienrisiko", "drittparteienrisiko"),
            ("2022", "2022"),
            ("Resilienz", "resilienz"),
            ("Finanzunternehmen", "finanzunternehm"),
            ("requirements", "requirement"),
            ("policies", "policy"),
        ];
        for (rein, raus) in proben {
            assert_eq!(stamm(rein).as_deref(), Some(raus), "Wort: {rein}");
        }
        assert_eq!(stamm("und"), None);
        assert_eq!(stamm("the"), None);
    }

    #[test]
    fn worte_zerlegt_bindestriche() {
        assert_eq!(
            worte("IKT-Drittparteienrisiko der Finanzunternehmen"),
            vec!["ikt", "drittparteienrisiko", "finanzunternehm"]
        );
    }

    // ------------------------------------------------------------- Indexaufbau

    use crate::modell::{Art, Block, Fundstelle, Knoten, Punkt, Quelle};

    fn probe_index() -> Index {
        let absatz = Knoten {
            id: "028.001".into(),
            art: Art::Absatz,
            nummer: Some("1".into()),
            bez: None,
            titel: BTreeMap::new(),
            pfad: Some("art/28/abs/1".into()),
            text: true,
            kinder: Vec::new(),
        };
        let artikel = Knoten {
            id: "art_28".into(),
            art: Art::Artikel,
            nummer: Some("28".into()),
            bez: None,
            titel: BTreeMap::from([("de".to_string(), "Allgemeine Grundsätze".to_string())]),
            pfad: Some("art/28".into()),
            text: false,
            kinder: vec![absatz],
        };
        let struktur = Struktur {
            regelwerk: "dora".into(),
            fassung: "2024-10-15".into(),
            abgerufen: "2026-10-01T00:00:00Z".into(),
            quelle: Quelle {
                name: "Probe".into(),
                url: "https://beispiel.test".into(),
                celex: None,
                hinweis: String::new(),
            },
            sprachen: vec!["de".into()],
            knoten: vec![artikel],
        };
        let mut text: Textdatei = BTreeMap::new();
        text.insert(
            "028.001".into(),
            Fundstelle {
                h: "x".into(),
                b: vec![
                    Block::P { t: "Die Finanzunternehmen führen ein Informationsregister.".into() },
                    Block::Liste {
                        p: vec![Punkt {
                            m: "a)".into(),
                            t: "Auslagerungen kritischer Funktionen".into(),
                            u: vec![],
                        }],
                    },
                ],
            },
        );
        baue(&struktur, &text, "de")
    }

    #[test]
    fn index_kennt_pfad_bezeichnung_und_laenge() {
        let i = probe_index();
        assert_eq!(i.regelwerk, "dora");
        assert_eq!(i.dokumente.len(), 1);
        let d = &i.dokumente[0];
        assert_eq!(d.id, "028.001");
        assert_eq!(d.pfad, "art/28/abs/1");
        // Die Bezeichnung führt den Artikel mit - wer "Artikel 28" sucht, findet den Absatz.
        assert!(d.b.contains("28"), "Bezeichnung: {}", d.b);
        assert!(d.t.starts_with("Die Finanzunternehmen"));
        assert_eq!(d.l as usize, i.avgdl as usize, "ein Dokument: avgdl ist seine Länge");
    }

    #[test]
    fn listenpunkte_landen_im_index() {
        let i = probe_index();
        // Auch der Text in der Aufzählung ist durchsuchbar.
        assert!(i.worte.contains_key("auslag"), "Stämme: {:?}", i.worte.keys());
        assert!(i.worte.contains_key("informationsregist"));
        // Stoppwörter nicht.
        assert!(!i.worte.contains_key("die"));
        // Die Bezeichnung zählt doppelt (worte_mit_gewicht), der Absatztext einfach.
        assert_eq!(i.worte["informationsregist"][0], [0, 1]);
    }

    #[test]
    fn leere_fundstellen_kommen_nicht_in_den_index() {
        let mut leer: Textdatei = BTreeMap::new();
        leer.insert("x".into(), Fundstelle { h: "h".into(), b: vec![] });
        let struktur = Struktur {
            regelwerk: "leer".into(),
            fassung: "2026-01-01".into(),
            abgerufen: "2026-01-01T00:00:00Z".into(),
            quelle: Quelle {
                name: String::new(),
                url: String::new(),
                celex: None,
                hinweis: String::new(),
            },
            sprachen: vec!["de".into()],
            knoten: vec![],
        };
        let i = baue(&struktur, &leer, "de");
        assert!(i.dokumente.is_empty());
        assert_eq!(i.avgdl, 0.0, "keine Division durch Null");
    }
}
