//! Konnektor fuer EUR-Lex ueber **CELLAR**, das amtliche Content-Negotiation-Portal
//! des Amts fuer Veroeffentlichungen.
//!
//! Warum nicht die EUR-Lex-Weboberflaeche: `eur-lex.europa.eu/legal-content/...` liegt
//! hinter einer WAF, die automatisierte Abrufe mit HTTP 202 und leerem Koerper abweist
//! ("x-amzn-waf-action: challenge"). CELLAR liefert dieselben Fassungen sauber aus und
//! schickt `ETag` und `Last-Modified` mit - beides braucht die Change-Detection.
//!
//! Die ausgelieferte XHTML-Fassung ist ELI-strukturiert, deshalb haengt der Parser nicht
//! an der Optik, sondern an stabilen Kennungen:
//!   * `div.eli-subdivision#art_28`   - Artikel 28
//!   * `div.eli-title > p.oj-sti-art` - seine Ueberschrift
//!   * `div#028.001`                  - dessen Absatz 1 (Kennung bleibt ueber Fassungen gleich)
//!   * `p.oj-ti-section-1/2`          - "KAPITEL II" bzw. "Abschnitt I" und der Titel dazu
//!   * `div#rct_47` / `div#cit_3`     - Erwaegungsgrund 47, Bezugsvermerk 3
//! Listenpunkte stehen je Punkt in einer eigenen einzeiligen `table` (Marke | Text),
//! verschachtelte Punkte als Tabelle in der Textzelle.

use crate::modell::{Art, Block, Knoten, Punkt};
use anyhow::{Context, Result, bail};
use scraper::{ElementRef, Html, Node, Selector};
use std::collections::BTreeMap;

pub const BASIS: &str = "https://publications.europa.eu/resource/celex";

pub struct Abruf {
    pub url: String,
    pub etag: Option<String>,
    pub stand: Option<String>,
    pub koerper: String,
}

/// Holt eine Sprachfassung. `etag` aus dem letzten Lauf mitgeben - antwortet die Quelle
/// mit 304, gibt es `Ok(None)` und der Volltext wird gar nicht uebertragen.
pub fn hole(celex: &str, sprache: &str, etag: Option<&str>) -> Result<Option<Abruf>> {
    let url = format!("{BASIS}/{celex}");
    let klient = reqwest::blocking::Client::builder()
        .user_agent("Aufsichtskompass/0.1 (private Lernanwendung)")
        .timeout(std::time::Duration::from_secs(180))
        .build()?;
    let mut anfrage = klient
        .get(&url)
        .header("Accept", "application/xhtml+xml")
        .header("Accept-Language", sprache);
    if let Some(e) = etag {
        anfrage = anfrage.header("If-None-Match", e);
    }
    let antwort = anfrage
        .send()
        .with_context(|| format!("Abruf {url} ({sprache})"))?;
    if antwort.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(None);
    }
    if !antwort.status().is_success() {
        bail!("{url} ({sprache}) antwortete mit {}", antwort.status());
    }
    let etag = kopfwert(antwort.headers(), "etag");
    let stand = kopfwert(antwort.headers(), "last-modified");
    let koerper = antwort.text()?;
    if koerper.len() < 5000 {
        bail!(
            "{url} ({sprache}) lieferte nur {} Zeichen - vermutlich abgewiesen",
            koerper.len()
        );
    }
    Ok(Some(Abruf { url, etag, stand, koerper }))
}

fn kopfwert(kopf: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    kopf.get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

pub struct Geparst {
    pub knoten: Vec<Knoten>,
    pub texte: BTreeMap<String, Vec<Block>>,
}

/// Zerlegt eine Sprachfassung in Gliederungsbaum und Text je Fundstelle.
pub fn parse(html: &str) -> Result<Geparst> {
    let dok = Html::parse_document(html);
    let koerper = Selector::parse("body").unwrap();
    let body = dok
        .select(&koerper)
        .next()
        .context("XHTML ohne <body> - Quellstruktur hat sich geaendert")?;

    let mut lauf = Lauf::default();
    lauf.gehe(body);
    let mut knoten = lauf.fertig();
    if knoten.is_empty() {
        bail!("kein Strukturelement gefunden - Parser pruefen, Daten nicht ueberschreiben");
    }
    markiere_text(&mut knoten, &lauf.texte);
    Ok(Geparst { knoten, texte: lauf.texte })
}

fn markiere_text(kn: &mut [Knoten], texte: &BTreeMap<String, Vec<Block>>) {
    for k in kn {
        k.text = texte.contains_key(&k.id);
        markiere_text(&mut k.kinder, texte);
    }
}

// --------------------------------------------------------------------- Aufbau

#[derive(Default)]
struct Lauf {
    wurzel: Vec<Knoten>,
    bezug: Vec<Knoten>,
    erwaegung: Vec<Knoten>,
    /// Index des offenen Kapitels in `wurzel` und des offenen Abschnitts darin.
    kapitel: Option<usize>,
    abschnitt: Option<usize>,
    /// Wohin der naechste Abschnittstitel gehoert.
    titel_fuer: Option<(Option<usize>, Option<usize>)>,
    texte: BTreeMap<String, Vec<Block>>,
}

impl Lauf {
    fn fertig(&mut self) -> Vec<Knoten> {
        let mut alles = Vec::new();
        if !self.bezug.is_empty() {
            alles.push(Knoten {
                id: "bezug".into(),
                art: Art::Praeambel,
                nummer: None,
                titel: paar("Bezugsvermerke", "Citations"),
                pfad: Some("bezug".into()),
                text: false,
                kinder: std::mem::take(&mut self.bezug),
            });
        }
        if !self.erwaegung.is_empty() {
            alles.push(Knoten {
                id: "eg".into(),
                art: Art::Praeambel,
                nummer: None,
                titel: paar("Erwägungsgründe", "Recitals"),
                pfad: Some("eg".into()),
                text: false,
                kinder: std::mem::take(&mut self.erwaegung),
            });
        }
        alles.append(&mut self.wurzel);
        alles
    }

    fn gehe(&mut self, el: ElementRef) {
        for kind in el.children() {
            let Some(k) = ElementRef::wrap(kind) else {
                continue;
            };
            let klasse = k.value().attr("class").unwrap_or("");
            let id = k.value().attr("id").unwrap_or("");
            match k.value().name() {
                "p" if klasse.contains("oj-ti-section-1") => {
                    self.abschnitts_kopf(&reintext(k));
                    continue;
                }
                "p" if klasse.contains("oj-ti-section-2") => {
                    self.abschnitts_titel(&reintext(k));
                    continue;
                }
                "div" if klasse.contains("eli-subdivision") => {
                    if let Some(n) = id.strip_prefix("art_") {
                        self.artikel(k, n);
                        continue;
                    }
                    if let Some(n) = id.strip_prefix("rct_") {
                        let kn = self.einzeiler(k, n, Art::Erwaegungsgrund, "eg");
                        self.erwaegung.push(kn);
                        continue;
                    }
                    if let Some(n) = id.strip_prefix("cit_") {
                        let kn = self.einzeiler(k, n, Art::Bezugsvermerk, "bezug");
                        self.bezug.push(kn);
                        continue;
                    }
                }
                _ => {}
            }
            self.gehe(k);
        }
    }

    /// "KAPITEL II" / "Abschnitt I" / "CHAPTER II" / "SECTION I"
    fn abschnitts_kopf(&mut self, roh: &str) {
        let Some((art, nummer)) = zerlege_kopf(roh) else {
            return;
        };
        if art == Art::Abschnitt {
            let Some(kap) = self.kapitel else { return };
            let pfad = format!(
                "{}/abschnitt/{}",
                self.wurzel[kap].pfad.clone().unwrap_or_default(),
                nummer
            );
            let kap_id = self.wurzel[kap].id.clone();
            self.wurzel[kap].kinder.push(Knoten {
                id: format!("{kap_id}_abschnitt_{nummer}"),
                art,
                nummer: Some(nummer),
                titel: BTreeMap::new(),
                pfad: Some(pfad),
                text: false,
                kinder: Vec::new(),
            });
            self.abschnitt = Some(self.wurzel[kap].kinder.len() - 1);
            self.titel_fuer = Some((self.kapitel, self.abschnitt));
        } else {
            self.wurzel.push(Knoten {
                id: format!("kap_{nummer}"),
                art,
                nummer: Some(nummer.clone()),
                titel: BTreeMap::new(),
                pfad: Some(format!("kap/{nummer}")),
                text: false,
                kinder: Vec::new(),
            });
            self.kapitel = Some(self.wurzel.len() - 1);
            self.abschnitt = None;
            self.titel_fuer = Some((self.kapitel, None));
        }
    }

    fn abschnitts_titel(&mut self, titel: &str) {
        let Some((kap, abs)) = self.titel_fuer.take() else {
            return;
        };
        let Some(kap) = kap else { return };
        let ziel = match abs {
            Some(a) => &mut self.wurzel[kap].kinder[a],
            None => &mut self.wurzel[kap],
        };
        ziel.titel.insert("de".into(), titel.to_string());
    }

    /// Erwaegungsgrund oder Bezugsvermerk: ein Block Text, Nummer steht in der Kennung.
    fn einzeiler(&mut self, el: ElementRef, nummer: &str, art: Art, pfad: &str) -> Knoten {
        let id = match art {
            Art::Erwaegungsgrund => format!("rct_{nummer}"),
            _ => format!("cit_{nummer}"),
        };
        let mut bloecke = bloecke_von(el);
        // Erwaegungsgruende beginnen mit "(47) " - die Nummer steht schon in der Kennung.
        if let Some(Block::P { t }) = bloecke.first_mut() {
            *t = marke_weg(t);
        }
        if !bloecke.is_empty() {
            self.texte.insert(id.clone(), bloecke);
        }
        Knoten {
            id,
            art,
            nummer: Some(nummer.to_string()),
            titel: BTreeMap::new(),
            pfad: Some(format!("{pfad}/{nummer}")),
            text: false,
            kinder: Vec::new(),
        }
    }

    fn artikel(&mut self, el: ElementRef, nummer_roh: &str) {
        let nummer = nummer_roh.trim().to_string();
        let id = format!("art_{nummer}");
        let ueberschrift = Selector::parse("p.oj-sti-art").unwrap();
        let mut titel = BTreeMap::new();
        if let Some(t) = el.select(&ueberschrift).next() {
            titel.insert("de".into(), reintext(t));
        }

        let mut kinder = Vec::new();
        // Nummerierte Absaetze liegen als <div id="028.001"> vor. Artikel ohne
        // Absatznummerierung haben ihren Text unmittelbar im Artikel-div.
        for kind in el.children() {
            let Some(k) = ElementRef::wrap(kind) else {
                continue;
            };
            if k.value().name() != "div" {
                continue;
            }
            let kid = k.value().attr("id").unwrap_or("");
            if !ist_absatz_kennung(kid) {
                continue;
            }
            let mut bloecke = bloecke_von(k);
            let mut abs_nr = None;
            if let Some(Block::P { t }) = bloecke.first_mut() {
                if let Some(n) = marke_lesen(t) {
                    abs_nr = Some(n);
                    *t = marke_weg(t);
                }
            }
            let nr = abs_nr.unwrap_or_else(|| {
                // Rueckfall: laufende Nummer aus der Kennung (028.003 -> 3)
                kid.split('.')
                    .nth(1)
                    .map(|s| s.trim_start_matches('0').to_string())
                    .unwrap_or_default()
            });
            if !bloecke.is_empty() {
                self.texte.insert(kid.to_string(), bloecke);
            }
            kinder.push(Knoten {
                id: kid.to_string(),
                art: Art::Absatz,
                nummer: Some(nr.clone()),
                titel: BTreeMap::new(),
                pfad: Some(format!("art/{nummer}/abs/{nr}")),
                text: false,
                kinder: Vec::new(),
            });
        }

        if kinder.is_empty() {
            let bloecke = bloecke_von(el);
            if !bloecke.is_empty() {
                self.texte.insert(id.clone(), bloecke);
            }
        }

        let knoten = Knoten {
            id,
            art: Art::Artikel,
            nummer: Some(nummer.clone()),
            titel,
            pfad: Some(format!("art/{nummer}")),
            text: false,
            kinder,
        };
        match (self.kapitel, self.abschnitt) {
            (Some(kap), Some(abs)) => self.wurzel[kap].kinder[abs].kinder.push(knoten),
            (Some(kap), None) => self.wurzel[kap].kinder.push(knoten),
            _ => self.wurzel.push(knoten),
        }
    }
}

// ------------------------------------------------------------------ Textteile

/// Bloecke eines Containers in Dokumentreihenfolge. Aufeinanderfolgende
/// Einzeiler-Tabellen werden zu **einer** Liste zusammengezogen.
fn bloecke_von(el: ElementRef) -> Vec<Block> {
    let mut aus: Vec<Block> = Vec::new();
    let mut liste: Vec<Punkt> = Vec::new();
    for kind in el.children() {
        let Some(k) = ElementRef::wrap(kind) else {
            continue;
        };
        let klasse = k.value().attr("class").unwrap_or("");
        match k.value().name() {
            "p" if klasse.contains("oj-ti-art") || klasse.contains("oj-sti-art") => {}
            "p" => {
                let t = reintext(k);
                if !t.is_empty() {
                    if !liste.is_empty() {
                        aus.push(Block::Liste {
                            p: std::mem::take(&mut liste),
                        });
                    }
                    aus.push(Block::P { t });
                }
            }
            "table" => {
                if let Some(p) = punkt_von_tabelle(k) {
                    liste.push(p);
                }
            }
            _ => {}
        }
    }
    if !liste.is_empty() {
        aus.push(Block::Liste { p: liste });
    }
    aus
}

/// Ein Listenpunkt: einzeilige Tabelle mit Marke in der ersten und Text in der zweiten Zelle.
fn punkt_von_tabelle(t: ElementRef) -> Option<Punkt> {
    let zeile = Selector::parse("tr").unwrap();
    let zelle = Selector::parse("td").unwrap();
    let tr = t.select(&zeile).next()?;
    let zellen: Vec<ElementRef> = tr.select(&zelle).collect();
    if zellen.is_empty() {
        return None;
    }
    let (marke, inhalt) = if zellen.len() == 1 {
        (String::new(), zellen[0])
    } else {
        (reintext(zellen[0]), zellen[1])
    };
    let mut text = Vec::new();
    let mut unter = Vec::new();
    for kind in inhalt.children() {
        let Some(k) = ElementRef::wrap(kind) else {
            continue;
        };
        match k.value().name() {
            "p" => {
                let s = reintext(k);
                if !s.is_empty() {
                    text.push(s);
                }
            }
            "table" => {
                if let Some(u) = punkt_von_tabelle(k) {
                    unter.push(u);
                }
            }
            _ => {}
        }
    }
    let text = text.join(" ");
    if text.is_empty() && unter.is_empty() {
        return None;
    }
    Some(Punkt { m: marke, t: text, u: unter })
}

/// Reiner Text eines Elements: Fussnotenzeichen und eingebettete Listen bleiben aussen vor.
fn reintext(el: ElementRef) -> String {
    let mut roh = String::new();
    sammle(el, &mut roh);
    normalisiere(&roh)
}

fn sammle(el: ElementRef, aus: &mut String) {
    for kind in el.children() {
        match kind.value() {
            Node::Text(t) => aus.push_str(t),
            Node::Element(e) => {
                let klasse = e.attr("class").unwrap_or("");
                // Hochgestellte Fussnotenzeichen "(1)" wuerden sonst mit Absatzmarken
                // verwechselt; Tabellen sind eigene Bloecke.
                if klasse.contains("oj-note") || e.name() == "table" {
                    continue;
                }
                if let Some(k) = ElementRef::wrap(kind) {
                    sammle(k, aus);
                }
            }
            _ => {}
        }
    }
}

pub fn normalisiere(s: &str) -> String {
    let mut aus = String::with_capacity(s.len());
    let mut leer = false;
    for c in s.chars() {
        // Geschuetzte und schmale Leerzeichen der Quelle gleichziehen.
        if c.is_whitespace() || c == '\u{a0}' || c == '\u{2009}' || c == '\u{202f}' {
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

fn ist_absatz_kennung(id: &str) -> bool {
    let Some((a, b)) = id.split_once('.') else {
        return false;
    };
    !a.is_empty()
        && !b.is_empty()
        && a.chars().all(|c| c.is_ascii_digit())
        && b.chars().all(|c| c.is_ascii_digit())
}

/// Liest die fuehrende Absatzmarke. Die deutschen Fassungen schreiben "(1)",
/// die englischen "1." - beide Formen kommen vor und gehoeren nicht in den Text.
fn marke_lesen(t: &str) -> Option<String> {
    if let Some(rest) = t.strip_prefix('(') {
        let (innen, _) = rest.split_once(')')?;
        if !innen.is_empty() && innen.len() <= 4 && innen.chars().all(|c| c.is_ascii_digit()) {
            return Some(innen.to_string());
        }
        return None;
    }
    // "1. Financial entities shall ..." - nur wenn danach ein Leerzeichen folgt,
    // damit Zahlen wie "1.000" oder Datumsangaben unangetastet bleiben.
    let (innen, rest) = t.split_once('.')?;
    if innen.is_empty() || innen.len() > 3 || !innen.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !rest.starts_with(' ') {
        return None;
    }
    Some(innen.to_string())
}

fn marke_weg(t: &str) -> String {
    if marke_lesen(t).is_none() {
        return t.to_string();
    }
    let trenner = if t.starts_with('(') { ')' } else { '.' };
    match t.split_once(trenner) {
        Some((_, rest)) => rest.trim_start().to_string(),
        None => t.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absatzmarken_beider_sprachen() {
        assert_eq!(marke_lesen("(1) Finanzunternehmen managen ..."), Some("1".into()));
        assert_eq!(marke_lesen("1. Financial entities shall ..."), Some("1".into()));
        assert_eq!(marke_lesen("12. Where a financial entity ..."), Some("12".into()));
        assert_eq!(marke_weg("(4) Vor Abschluss"), "Vor Abschluss");
        assert_eq!(marke_weg("4. Before entering"), "Before entering");
        // Kein Marker: Zahlen im Text bleiben stehen.
        assert_eq!(marke_lesen("1.000 Euro sind faellig"), None);
        assert_eq!(marke_lesen("Artikel 6 Absatz 1 gilt"), None);
        assert_eq!(marke_weg("1.000 Euro"), "1.000 Euro");
    }
}

/// "KAPITEL II" -> (Kapitel, "II"); "Abschnitt I" -> (Abschnitt, "I").
/// Englische Fassung: "CHAPTER II", "SECTION I", "TITLE".
fn zerlege_kopf(roh: &str) -> Option<(Art, String)> {
    let roh = normalisiere(roh);
    let (wort, nummer) = roh.split_once(' ').unwrap_or((roh.as_str(), ""));
    let art = match wort.to_lowercase().as_str() {
        "kapitel" | "chapter" => Art::Kapitel,
        "abschnitt" | "section" => Art::Abschnitt,
        "titel" | "title" => Art::Teil,
        "anhang" | "annex" => Art::Anhang,
        _ => return None,
    };
    let nummer = nummer.trim().trim_end_matches(['.', ':']).to_string();
    if nummer.is_empty() {
        None
    } else {
        Some((art, nummer))
    }
}

fn paar(de: &str, en: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("de".into(), de.into());
    m.insert("en".into(), en.into());
    m
}
