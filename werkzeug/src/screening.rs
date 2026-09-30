//! Tägliches Markt- und Compliance-Screening.
//!
//! Vollständig mit kostenlosen Mitteln: Feeds der Aufsichts- und Fachstellen,
//! Seitenüberwachung für Quellen ohne Feed, dazu der SPARQL-Dienst des Amts für
//! Veröffentlichungen für neue Level-2-Rechtsakte. Kein LLM, kein Bezahldienst.
//!
//! Ablauf je Lauf:
//!   1. Abruf (höflich: eigener User-Agent, ETag/Last-Modified, Fehler je Quelle
//!      werden protokolliert und brechen den Lauf nicht ab),
//!   2. Normalisierung und Deduplizierung (kanonische URL, Inhalts-Hash, Titelähnlichkeit),
//!   3. Relevanzbewertung über die pflegbare Taxonomie - mit Begründung, welche
//!      Begriffe gegriffen haben,
//!   4. Kategorisierung nach Regeln,
//!   5. extraktive Zusammenfassung (Sätze nach Begriffsüberdeckung gewichtet),
//!   6. Zuordnung zu Themen und Regelwerken.

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

// ------------------------------------------------------------------ Konfiguration

#[derive(Debug, Deserialize)]
pub struct Feedquelle {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub herausgeber: String,
    #[serde(default)]
    pub nutzung: Option<String>,
    #[serde(default)]
    pub aktiv: Option<bool>,
    /// Eigene Schwelle fuer diese Quelle. Sinnvoll bei Feeds, die taeglich
    /// dutzende Meldungen liefern, von denen die wenigsten hierher gehoeren.
    #[serde(default)]
    pub mindestpunkte: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct Seitenquelle {
    pub id: String,
    pub name: String,
    pub url: String,
    /// CSS-Auswahl des Bereichs, der überwacht wird.
    pub selektor: String,
    #[serde(default)]
    pub herausgeber: String,
    #[serde(default)]
    pub aktiv: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Taxonomie {
    pub begriffe: Vec<Begriff>,
    #[serde(default)]
    pub ausschluss: Vec<String>,
    #[serde(default = "standardschwelle")]
    pub schwelle: i32,
}

fn standardschwelle() -> i32 {
    8
}

#[derive(Debug, Deserialize)]
pub struct Begriff {
    pub wort: String,
    pub gewicht: i32,
    #[serde(default)]
    pub synonyme: Vec<String>,
    #[serde(default)]
    pub themen: Vec<String>,
    #[serde(default)]
    pub regelwerke: Vec<String>,
}

// ------------------------------------------------------------------- Ergebnisse

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meldung {
    pub id: String,
    pub titel: String,
    pub url: String,
    pub quelle: String,
    pub quelle_name: String,
    #[serde(default)]
    pub herausgeber: String,
    /// Veröffentlichungsdatum der Quelle, sonst der Zeitpunkt des Fundes.
    pub datum: String,
    pub gefunden: String,
    pub kategorie: String,
    pub punkte: i32,
    /// Welche Begriffe gegriffen haben - die Bewertung soll nachvollziehbar sein.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub begruendung: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub zusammenfassung: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub themen: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regelwerke: Vec<String>,
    /// Vorschlag, ein bislang unbekanntes Regelwerk aufzunehmen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vorschlag: Option<Vorschlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vorschlag {
    pub celex: String,
    pub titel: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct Quellenstand {
    pub quelle: String,
    pub name: String,
    pub status: String,
    pub gefunden: usize,
    pub uebernommen: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fehler: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Lauf {
    pub start: String,
    pub dauer_s: f64,
    pub ausloeser: String,
    pub quellen: Vec<Quellenstand>,
    pub neu: usize,
    pub geprueft: usize,
}

/// Merkt sich ETag und Hash je Quelle, damit unveränderte Quellen nichts kosten.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Stand {
    #[serde(default)]
    pub quellen: BTreeMap<String, Quellgedaechtnis>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Quellgedaechtnis {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stand: Option<String>,
    /// Hash des überwachten Seitenausschnitts (nur Seitenüberwachung).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(default)]
    pub zuletzt: String,
}

// ----------------------------------------------------------------------- Abruf

pub fn klient() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent("Aufsichtskompass/0.1 (private Lernanwendung; taeglicher Lauf)")
        .timeout(std::time::Duration::from_secs(60))
        .build()?)
}

pub struct Antwort {
    pub koerper: String,
    pub etag: Option<String>,
    pub stand: Option<String>,
    pub unveraendert: bool,
}

pub fn hole(
    klient: &reqwest::blocking::Client,
    url: &str,
    gedaechtnis: Option<&Quellgedaechtnis>,
) -> Result<Antwort> {
    let mut anfrage = klient.get(url);
    if let Some(g) = gedaechtnis {
        if let Some(e) = &g.etag {
            anfrage = anfrage.header("If-None-Match", e);
        }
        if let Some(s) = &g.stand {
            anfrage = anfrage.header("If-Modified-Since", s);
        }
    }
    let antwort = anfrage.send().with_context(|| format!("Abruf {url}"))?;
    if antwort.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(Antwort { koerper: String::new(), etag: None, stand: None, unveraendert: true });
    }
    if !antwort.status().is_success() {
        anyhow::bail!("HTTP {}", antwort.status());
    }
    let kopf = |n: &str| {
        antwort.headers().get(n).and_then(|v| v.to_str().ok()).map(|s| s.to_string())
    };
    let etag = kopf("etag");
    let stand = kopf("last-modified");
    Ok(Antwort { koerper: antwort.text()?, etag, stand, unveraendert: false })
}

// ---------------------------------------------------------------- Feed-Parser

pub struct Roheintrag {
    pub titel: String,
    pub url: String,
    pub datum: Option<String>,
    pub text: String,
}

/// Liest RSS 2.0 und Atom. Feeds sind XML, aber nicht immer sauberes:
/// benannte HTML-Entitäten kommen vor und sind in XML nicht definiert.
pub fn feed_lesen(roh: &str) -> Result<Vec<Roheintrag>> {
    let sauber = entitaeten_entschaerfen(roh);
    let dok = roxmltree::Document::parse_with_options(
        &sauber,
        roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() },
    )
    .context("Feed ist kein lesbares XML")?;

    let mut aus = Vec::new();
    for knoten in dok.descendants() {
        let ist_item = knoten.has_tag_name("item");
        let ist_entry = knoten.has_tag_name("entry");
        if !ist_item && !ist_entry {
            continue;
        }
        let kind = |name: &str| {
            knoten
                .children()
                .find(|c| c.has_tag_name(name))
                .map(|c| c.text().unwrap_or("").trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let titel = kind("title").unwrap_or_default();
        // Atom: <link href="..."/>, RSS: <link>...</link>
        let url = kind("link")
            .or_else(|| {
                knoten
                    .children()
                    .find(|c| c.has_tag_name("link"))
                    .and_then(|c| c.attribute("href").map(|s| s.to_string()))
            })
            .or_else(|| kind("guid"))
            .unwrap_or_default();
        let datum = kind("pubDate").or_else(|| kind("updated")).or_else(|| kind("published"));
        let text = kind("description")
            .or_else(|| kind("summary"))
            .or_else(|| kind("content"))
            .unwrap_or_default();
        if titel.is_empty() || url.is_empty() {
            continue;
        }
        aus.push(Roheintrag { titel: text_saeubern(&titel), url, datum, text: text_saeubern(&text) });
    }
    Ok(aus)
}

/// Benannte Entitäten außerhalb der fünf XML-Standards in Zeichen umsetzen -
/// sonst scheitert der Parser an `&nbsp;` und Verwandten.
fn entitaeten_entschaerfen(s: &str) -> String {
    let muster = Regex::new(r"&([a-zA-Z][a-zA-Z0-9]{1,10});").unwrap();
    muster
        .replace_all(s, |c: &regex::Captures| match &c[1] {
            "amp" | "lt" | "gt" | "quot" | "apos" => c[0].to_string(),
            "nbsp" => " ".to_string(),
            "auml" => "ä".into(),
            "ouml" => "ö".into(),
            "uuml" => "ü".into(),
            "Auml" => "Ä".into(),
            "Ouml" => "Ö".into(),
            "Uuml" => "Ü".into(),
            "szlig" => "ß".into(),
            "euro" => "€".into(),
            "ndash" | "mdash" => "–".into(),
            "bdquo" => "„".into(),
            "ldquo" | "rdquo" | "quo" => "\u{201c}".into(),
            _ => " ".to_string(),
        })
        .into_owned()
}

/// HTML aus Feed-Texten entfernen und Leerraum glätten.
pub fn text_saeubern(s: &str) -> String {
    let ohne_tags = Regex::new(r"<[^>]*>").unwrap().replace_all(s, " ");
    let ohne_entitaeten = entitaeten_entschaerfen(&ohne_tags)
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'");
    let mut aus = String::with_capacity(ohne_entitaeten.len());
    let mut leer = false;
    for c in ohne_entitaeten.chars() {
        if c.is_whitespace() || c == '\u{a0}' {
            leer = true;
            continue;
        }
        if leer && !aus.is_empty() {
            aus.push(' ');
        }
        leer = false;
        aus.push(c);
    }
    aus.trim().to_string()
}

// ----------------------------------------------------------- Deduplizierung

/// URL auf das Wesentliche zurückführen: ohne Tracking-Parameter, ohne Anker,
/// ohne abschließenden Schrägstrich.
pub fn url_kanonisch(url: &str) -> String {
    let ohne_anker = url.split('#').next().unwrap_or(url);
    let (pfad, abfrage) = match ohne_anker.split_once('?') {
        Some((p, a)) => (p, Some(a)),
        None => (ohne_anker, None),
    };
    let behalten: Vec<&str> = abfrage
        .map(|a| {
            a.split('&')
                .filter(|p| {
                    let name = p.split('=').next().unwrap_or("").to_lowercase();
                    !name.starts_with("utm_")
                        && name != "nn"
                        && name != "fbclid"
                        && name != "gclid"
                        && !name.is_empty()
                })
                .collect()
        })
        .unwrap_or_default();
    let mut aus = pfad.trim_end_matches('/').to_string();
    if !behalten.is_empty() {
        aus.push('?');
        aus.push_str(&behalten.join("&"));
    }
    aus
}

pub fn kennung(url: &str) -> String {
    let mut h = Sha256::new();
    h.update(url_kanonisch(url).to_lowercase().as_bytes());
    hex::encode(h.finalize())[..16].to_string()
}

/// Titelähnlichkeit über gemeinsame Wörter (Jaccard). Fängt dieselbe Meldung
/// in zwei Feeds ab, wenn sich die Adressen unterscheiden.
pub fn aehnlich(a: &str, b: &str) -> bool {
    let menge = |s: &str| -> BTreeSet<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 3)
            .map(|w| w.to_string())
            .collect()
    };
    let (x, y) = (menge(a), menge(b));
    if x.len() < 3 || y.len() < 3 {
        return false;
    }
    let schnitt = x.intersection(&y).count() as f32;
    let vereinigung = x.union(&y).count() as f32;
    schnitt / vereinigung > 0.7
}

// ------------------------------------------------------------ Relevanzbewertung

pub struct Bewertung {
    pub punkte: i32,
    pub begruendung: Vec<String>,
    pub themen: BTreeSet<String>,
    pub regelwerke: BTreeSet<String>,
}

/// Kommt `wort` als eigenes Wort vor? Der Anfang muss auf einer Wortgrenze liegen,
/// das Ende nicht: so trifft "IKT-Risikomanagement" auch den "-rahmen", aber "ITS"
/// nicht mehr mitten in "bereits".
fn enthaelt_wort(heuhaufen: &str, wort: &str) -> bool {
    let w = wort.to_string();
    if w.is_empty() {
        return false;
    }
    let mut ab = 0;
    while let Some(i) = heuhaufen[ab..].find(&w) {
        let start = ab + i;
        let davor = heuhaufen[..start].chars().next_back();
        if davor.is_none_or(|c| !c.is_alphanumeric()) {
            return true;
        }
        ab = start + w.len().max(1);
        if ab >= heuhaufen.len() {
            break;
        }
    }
    false
}

/// Kurze Grossbuchstaben-Kuerzel ("ITS", "RTS", "IKS") werden gross gesucht.
/// Sonst trifft das englische Wort "its" die Abkuerzung ITS - in englischsprachigen
/// Feeds war das der haeufigste Fehlgriff.
fn ist_akronym(wort: &str) -> bool {
    wort.len() <= 5
        && wort.chars().any(|c| c.is_ascii_uppercase())
        && wort.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-' || c == '/')
}

/// Regelbasierte Bewertung: Treffer im Titel zählen doppelt, jeder Begriff nur einmal.
pub fn bewerten(titel: &str, text: &str, tax: &Taxonomie) -> Bewertung {
    let t_klein = titel.to_lowercase();
    let v_klein = format!("{} {}", t_klein, text.to_lowercase());
    let v_roh = format!("{titel} {text}");
    let mut b = Bewertung {
        punkte: 0,
        begruendung: Vec::new(),
        themen: BTreeSet::new(),
        regelwerke: BTreeSet::new(),
    };
    for aus in &tax.ausschluss {
        if enthaelt_wort(&v_klein, &aus.to_lowercase()) {
            b.punkte -= 12;
            b.begruendung.push(format!("Ausschluss: {aus}"));
        }
    }
    for begriff in &tax.begriffe {
        let mut treffer: Option<String> = None;
        let mut im_titel = false;
        for wort in std::iter::once(&begriff.wort).chain(begriff.synonyme.iter()) {
            // Akronyme in der Schreibweise der Quelle suchen, alles andere kleingeschrieben.
            let (heu_voll, heu_titel, nadel) = if ist_akronym(wort) {
                (v_roh.as_str(), titel, wort.clone())
            } else {
                (v_klein.as_str(), t_klein.as_str(), wort.to_lowercase())
            };
            if enthaelt_wort(heu_voll, &nadel) {
                treffer = Some(wort.clone());
                im_titel = enthaelt_wort(heu_titel, &nadel);
                break;
            }
        }
        let Some(wort) = treffer else { continue };
        let punkte = if im_titel { begriff.gewicht * 2 } else { begriff.gewicht };
        b.punkte += punkte;
        b.begruendung.push(format!("{wort} ({punkte:+})"));
        b.themen.extend(begriff.themen.iter().cloned());
        b.regelwerke.extend(begriff.regelwerke.iter().cloned());
    }
    b
}

// --------------------------------------------------------------- Kategorisierung

pub fn kategorie(titel: &str, text: &str) -> &'static str {
    let s = format!("{} {}", titel.to_lowercase(), text.to_lowercase());
    let hat = |w: &[&str]| w.iter().any(|x| s.contains(x));
    if hat(&["konsultation", "consultation", "anhörung", "call for advice", "call for papers"]) {
        "Konsultation"
    } else if hat(&["schwachstelle", "sicherheitswarnung", "vulnerabilit", "advisory", "angriff", "ransomware", "zeroday", "zero-day"]) {
        "Vorfall & Bedrohungslage"
    } else if hat(&["bußgeld", "bussgeld", "sanktion", "geldbuße", "maßnahme gegen", "abberufung", "fine"]) {
        "Sanktion"
    } else if hat(&["tritt in kraft", "verkündet", "neue verordnung", "neue richtlinie", "veröffentlicht im amtsblatt", "delegierte verordnung", "durchführungsverordnung"]) {
        "Neue Regulierung"
    } else if hat(&["ändert", "änderung", "geändert", "neufassung", "ersetzt", "aktualisiert", "amendment"]) {
        "Änderung"
    } else if hat(&["rundschreiben", "aufsichtsmitteilung", "merkblatt", "auslegung", "faq", "guideline", "leitlinie", "hinweis der"]) {
        "Aufsichtspraxis"
    } else {
        "Fachartikel"
    }
}

// ------------------------------------------------------------ Zusammenfassung

/// Extraktiv und ohne Sprachmodell: Sätze werden nach der Überdeckung mit den
/// Suchbegriffen gewichtet, die besten zwei in Originalreihenfolge ausgegeben.
pub fn zusammenfassen(text: &str, begriffe: &[String], max: usize) -> String {
    let saubere = text_saeubern(text);
    if saubere.len() <= max {
        return saubere;
    }
    let worte: Vec<String> = begriffe
        .iter()
        .map(|b| b.split(' ').next().unwrap_or(b).to_lowercase())
        .collect();
    let mut saetze: Vec<(usize, i32, &str)> = saubere
        .split_inclusive(['.', '!', '?'])
        .map(|s| s.trim())
        .filter(|s| s.len() > 25)
        .enumerate()
        .map(|(i, s)| {
            let k = s.to_lowercase();
            let punkte = worte.iter().filter(|w| k.contains(*w)).count() as i32;
            (i, punkte, s)
        })
        .collect();
    if saetze.is_empty() {
        return saubere.chars().take(max).collect::<String>() + "…";
    }
    saetze.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut gewaehlt: Vec<(usize, &str)> =
        saetze.iter().take(2).map(|(i, _, s)| (*i, *s)).collect();
    gewaehlt.sort_by_key(|(i, _)| *i);
    let mut aus = gewaehlt.iter().map(|(_, s)| *s).collect::<Vec<_>>().join(" ");
    if aus.chars().count() > max {
        aus = aus.chars().take(max).collect::<String>() + "…";
    }
    aus
}

// ------------------------------------------------------------------- Zeitpunkt

/// Datum einer Meldung in ISO-Form. Feeds schreiben RFC 2822 oder RFC 3339.
pub fn datum_iso(roh: Option<&String>) -> Option<String> {
    let r = roh?;
    if let Ok(d) = DateTime::parse_from_rfc2822(r) {
        return Some(d.with_timezone(&Utc).format("%Y-%m-%dT%H:%M:%SZ").to_string());
    }
    if let Ok(d) = DateTime::parse_from_rfc3339(r) {
        return Some(d.with_timezone(&Utc).format("%Y-%m-%dT%H:%M:%SZ").to_string());
    }
    None
}

/// Monatsdatei, in die eine Meldung gehört.
pub fn monat(datum: &str) -> String {
    DateTime::parse_from_rfc3339(datum)
        .map(|d| format!("{:04}-{:02}", d.year(), d.month()))
        .unwrap_or_else(|_| Utc::now().format("%Y-%m").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_werden_zusammengefuehrt() {
        assert_eq!(
            url_kanonisch("https://www.bafin.de/x.html?nn=150166&utm_source=rss#content"),
            "https://www.bafin.de/x.html"
        );
        assert_eq!(kennung("https://a.de/x/"), kennung("https://a.de/x#weg"));
    }

    #[test]
    fn titel_aehnlichkeit() {
        assert!(aehnlich(
            "BaFin veröffentlicht Rundschreiben zur Auslagerung",
            "BaFin veröffentlicht neues Rundschreiben zur Auslagerung"
        ));
        assert!(!aehnlich(
            "BaFin veröffentlicht Rundschreiben zur Auslagerung",
            "EIOPA konsultiert Leitlinien zur Nachhaltigkeit"
        ));
    }

    #[test]
    fn kategorien_nach_regeln() {
        assert_eq!(kategorie("Konsultation 01/2026", ""), "Konsultation");
        assert_eq!(kategorie("Kritische Schwachstelle in Citrix", ""), "Vorfall & Bedrohungslage");
        assert_eq!(kategorie("BaFin verhängt Bußgeld", ""), "Sanktion");
        assert_eq!(kategorie("Rundschreiben 13/2026 (VA)", ""), "Aufsichtspraxis");
        assert_eq!(kategorie("Jahresbericht des Verbandes", ""), "Fachartikel");
    }

    #[test]
    fn feeds_beider_bauarten() {
        let rss = r#"<?xml version="1.0"?><rss><channel><item>
            <title>Rundschreiben 13/2026</title><link>https://a.de/x</link>
            <description>&lt;p&gt;Text mit &auml;lteren Entit&auml;ten&lt;/p&gt;</description>
            <pubDate>Tue, 15 Oct 2024 11:53:52 GMT</pubDate></item></channel></rss>"#;
        let e = feed_lesen(rss).unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].titel, "Rundschreiben 13/2026");
        assert!(e[0].text.contains("älteren"), "Entitaeten: {}", e[0].text);

        let atom = r#"<?xml version="1.0"?><feed xmlns="http://www.w3.org/2005/Atom"><entry>
            <title>Neue Leitlinie</title><link href="https://b.eu/y"/>
            <summary>Zusammenfassung</summary><updated>2026-09-30T08:00:00Z</updated></entry></feed>"#;
        let e = feed_lesen(atom).unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].url, "https://b.eu/y");
    }

    #[test]
    fn akronyme_nur_gross() {
        let tax = Taxonomie {
            begriffe: vec![Begriff {
                wort: "ITS".into(),
                gewicht: 7,
                synonyme: vec![],
                themen: vec![],
                regelwerke: vec![],
            }],
            ausschluss: vec![],
            schwelle: 1,
        };
        // Englisches "its" darf nicht greifen, die Abkuerzung schon.
        assert_eq!(bewerten("ESMA sets its priorities", "text", &tax).punkte, 0);
        assert_eq!(bewerten("Neuer ITS zum Register", "text", &tax).punkte, 14);
    }

    #[test]
    fn wortgrenzen_statt_zeichenketten() {
        // "ITS" darf nicht in "bereits" treffen, "IKT-Risikomanagement" aber im Kompositum.
        assert!(!enthaelt_wort("das ist bereits erledigt", "its"));
        assert!(enthaelt_wort("die its-verordnung", "its"));
        assert!(enthaelt_wort("der ikt-risikomanagementrahmen", "ikt-risikomanagement"));
        assert!(!enthaelt_wort("mehrere schwachstellen in n8n", "its"));
        // Grossschreibung bleibt erhalten, wenn danach gesucht wird
        assert!(enthaelt_wort("Neuer ITS zum Register", "ITS"));
        assert!(!enthaelt_wort("ESMA sets its priorities", "ITS"));
    }

    #[test]
    fn bewertung_begruendet_sich() {
        let tax = Taxonomie {
            begriffe: vec![Begriff {
                wort: "DORA".into(),
                gewicht: 10,
                synonyme: vec!["2022/2554".into()],
                themen: vec!["ikt-risikomanagement".into()],
                regelwerke: vec!["dora".into()],
            }],
            ausschluss: vec!["Stellenangebot".into()],
            schwelle: 8,
        };
        let b = bewerten("DORA: neue Vorgaben", "Text", &tax);
        assert_eq!(b.punkte, 20, "Titeltreffer zaehlt doppelt");
        assert!(b.begruendung[0].starts_with("DORA"));
        assert!(b.regelwerke.contains("dora"));

        let b2 = bewerten("Stellenangebot", "Wir suchen", &tax);
        assert!(b2.punkte < 0);
    }
}
