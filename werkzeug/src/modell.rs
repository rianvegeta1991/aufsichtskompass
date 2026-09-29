//! Datenmodell der Dateien unter `daten/`.
//!
//! Grundsatz des Projekts: fachliche Inhalte liegen als Daten im Repo, nicht im Code.
//! Dieses Modul beschreibt nur die *Form* dieser Dateien - welche Regelwerke es gibt,
//! steht in `daten/regelwerke.json` und `daten/quellen.json`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------- Konnektoren

/// `daten/quellen.json` - je Eintrag ein Abrufweg. Wird redaktionell gepflegt;
/// ein neues Regelwerk braucht deshalb keinen Code-Patch, nur einen neuen Eintrag.
#[derive(Debug, Deserialize)]
pub struct Quellenliste {
    pub konnektoren: Vec<Konnektor>,
}

#[derive(Debug, Deserialize)]
pub struct Konnektor {
    /// Id des Regelwerks, zu dem der Abruf gehoert (Schluessel in `regelwerke.json`).
    pub regelwerk: String,
    /// Bauart des Abrufs. Zurzeit: `cellar` (EUR-Lex/CELLAR).
    pub typ: String,
    /// CELEX-Nummer, z. B. `32022R2554` fuer DORA.
    #[serde(default)]
    pub celex: Option<String>,
    /// Kennung bei gesetze-im-internet.de, z. B. `vag_2016`.
    #[serde(default)]
    pub kennung: Option<String>,
    /// Beschraenkung auf einzelne Paragrafen (leer = ganzes Gesetz). Fuer Gesetze,
    /// von denen nur die IT-relevanten Vorschriften aufgenommen werden (HGB, AO).
    #[serde(default)]
    pub paragraphen: Vec<String>,
    /// Sprachcodes des Abrufs, dreistellig nach CELLAR (`deu`, `eng`).
    #[serde(default)]
    pub sprachen: Vec<String>,
    /// Nutzungsbedingungen und Kosten - dokumentationspflichtig, wird nicht ausgewertet.
    #[serde(default)]
    pub nutzung: Option<String>,
    #[serde(default)]
    pub aktiv: Option<bool>,
}

// ------------------------------------------------------------------ Fassungen

/// `daten/rw/<id>/fassungen.json` - Versionsregister eines Regelwerks.
/// Jede Fassung bleibt unveraenderlich stehen; neue Fassungen kommen hinzu.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Fassungsregister {
    pub regelwerk: String,
    #[serde(default)]
    pub fassungen: Vec<Fassung>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fassung {
    /// Ordnername und Kennung der Fassung, z. B. `2024-10-15`.
    pub id: String,
    /// Stand der Fassung laut Quelle (bei CELLAR: `Last-Modified`).
    pub stand: String,
    /// Zeitpunkt des Abrufs (ISO 8601, UTC).
    pub abgerufen: String,
    pub quelle: Quelle,
    /// `ETag` der Quelle - spart beim naechsten Lauf den Volltextabruf.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// SHA-256 ueber die normalisierten Texte der Fassung.
    pub hash: String,
    /// Gueltig bis: gesetzt, sobald eine neuere Fassung uebernommen wurde.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gueltig_bis: Option<String>,
    /// Anzahl Fundstellen mit Text - zur Sichtkontrolle im Betrieb.
    #[serde(default)]
    pub fundstellen: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quelle {
    pub name: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub celex: Option<String>,
    /// Rechtlicher Hinweis, der im Viewer unter dem Text steht.
    pub hinweis: String,
}

// ------------------------------------------------------------------- Struktur

/// `daten/rw/<id>/<fassung>/struktur.json` - Gliederungsbaum einer Fassung.
#[derive(Debug, Serialize, Deserialize)]
pub struct Struktur {
    pub regelwerk: String,
    pub fassung: String,
    pub abgerufen: String,
    pub quelle: Quelle,
    pub sprachen: Vec<String>,
    pub knoten: Vec<Knoten>,
}

/// Ein Strukturelement. `id` ist ueber Fassungen hinweg stabil (Kennung der Quelle),
/// `pfad` ist der Anteil des Deep-Links hinter dem Regelwerk, z. B. `art/28/abs/4`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Knoten {
    pub id: String,
    pub art: Art,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nummer: Option<String>,
    /// Beschriftung genau so, wie die Quelle sie schreibt - z. B. "Erstes Buch"
    /// oder "Zweiter Abschnitt". Deutsche Gesetze nummerieren mit Wortformen;
    /// aus Art und Nummer liesse sich das nicht richtig zusammensetzen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bez: Option<String>,
    /// Ueberschrift je Sprache (`de`, `en`); leer, wo die Quelle keine fuehrt.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub titel: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pfad: Option<String>,
    /// true, wenn zu dieser Kennung Text in `text-<sprache>.json` liegt.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub text: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinder: Vec<Knoten>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Art {
    Teil,
    Kapitel,
    Abschnitt,
    Artikel,
    /// Paragraf deutscher Gesetze (§).
    Paragraf,
    Absatz,
    Erwaegungsgrund,
    Bezugsvermerk,
    Anhang,
    Praeambel,
}

impl Art {
    /// Bezeichnung fuer Ueberschriften und Zitate.
    pub fn wort(&self) -> (&'static str, &'static str) {
        match self {
            Art::Teil => ("Titel", "Title"),
            Art::Kapitel => ("Kapitel", "Chapter"),
            Art::Abschnitt => ("Abschnitt", "Section"),
            Art::Artikel => ("Artikel", "Article"),
            Art::Paragraf => ("§", "Section"),
            Art::Absatz => ("Absatz", "paragraph"),
            Art::Erwaegungsgrund => ("Erwägungsgrund", "Recital"),
            Art::Bezugsvermerk => ("Bezugsvermerk", "Citation"),
            Art::Anhang => ("Anhang", "Annex"),
            Art::Praeambel => ("Präambel", "Preamble"),
        }
    }
}

// ----------------------------------------------------------------------- Text

/// `daten/rw/<id>/<fassung>/text-<sprache>.json` - Text je Fundstelle.
pub type Textdatei = BTreeMap<String, Fundstelle>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Fundstelle {
    /// SHA-256 des normalisierten Textes - Grundlage der Change-Detection je Element.
    pub h: String,
    pub b: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "art")]
pub enum Block {
    /// Fortlaufender Absatztext.
    #[serde(rename = "p")]
    P { t: String },
    /// Aufzaehlung; `m` ist die Marke der Quelle (`a)`, `i)`, `1.`).
    #[serde(rename = "liste")]
    Liste { p: Vec<Punkt> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Punkt {
    pub m: String,
    pub t: String,
    /// Verschachtelte Unterpunkte (z. B. i), ii) unter a)).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub u: Vec<Punkt>,
}

impl Block {
    /// Reiner Text eines Blocks - fuer Hash und Suchindex.
    pub fn nur_text(&self) -> String {
        match self {
            Block::P { t } => t.clone(),
            Block::Liste { p } => p
                .iter()
                .map(punkt_text)
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

fn punkt_text(p: &Punkt) -> String {
    let mut s = format!("{} {}", p.m, p.t);
    for u in &p.u {
        s.push(' ');
        s.push_str(&punkt_text(u));
    }
    s.trim().to_string()
}

// ------------------------------------------------------------- Aenderungslog

/// `daten/aenderungen.json` - Protokoll der uebernommenen Fassungen.
/// Grundlage fuer Benachrichtigung, Diff-Ansicht und "zu pruefen"-Markierungen.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Aenderungslog {
    #[serde(default)]
    pub ereignisse: Vec<Aenderung>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Aenderung {
    pub zeitpunkt: String,
    pub regelwerk: String,
    pub fassung: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vorherige: Option<String>,
    pub art: String,
    /// Kennungen neuer, geaenderter und entfallener Fundstellen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub neu: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub geaendert: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entfallen: Vec<String>,
    pub quelle: String,
}
