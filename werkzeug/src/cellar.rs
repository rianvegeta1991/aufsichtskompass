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
//!
//! * `div.eli-subdivision#art_28` - Artikel 28
//! * `div.eli-title > p.oj-sti-art` - seine Ueberschrift
//! * `div#028.001` - dessen Absatz 1 (Kennung bleibt ueber Fassungen gleich)
//! * `p.oj-ti-section-1/2` - "KAPITEL II" bzw. "Abschnitt I" und der Titel dazu
//! * `div#rct_47` / `div#cit_3` - Erwaegungsgrund 47, Bezugsvermerk 3
//!
//! Listenpunkte stehen je Punkt in einer eigenen einzeiligen `table` (Marke | Text),
//! verschachtelte Punkte als Tabelle in der Textzelle.

use crate::modell::{Art, Block, Knoten, Punkt};
use anyhow::{Context, Result, bail};
use scraper::{ElementRef, Html, Node, Selector};
use std::collections::BTreeMap;

pub const BASIS: &str = "https://publications.europa.eu/resource/celex";
const KENNUNG: &str = "Aufsichtskompass/0.1 (private Lernanwendung)";

pub struct Abruf {
    /// Abgerufene Adresse - steht im Protokoll des Laufs und in Fehlermeldungen.
    #[allow(dead_code)]
    pub url: String,
    pub etag: Option<String>,
    pub stand: Option<String>,
    pub koerper: String,
}

/// Holt eine Sprachfassung. `etag` aus dem letzten Lauf mitgeben - antwortet die Quelle
/// mit 304, gibt es `Ok(None)` und der Volltext wird gar nicht uebertragen.
pub fn hole(
    leine: &crate::netz::Leine,
    celex: &str,
    sprache: &str,
    etag: Option<&str>,
) -> Result<Option<Abruf>> {
    let url = format!("{BASIS}/{celex}");
    let klient = leine.klient(KENNUNG, 180)?;
    let mut kopf = vec![("Accept", "application/xhtml+xml"), ("Accept-Language", sprache)];
    if let Some(e) = etag {
        kopf.push(("If-None-Match", e));
    }
    let antwort = leine
        .senden(&klient, &url, &kopf)
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
    /// Anhaenge stehen im Dokument nach dem verfuegenden Teil und kommen auch so in den Baum.
    anhaenge: Vec<Knoten>,
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
                bez: None,
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
                bez: None,
                titel: paar("Erwägungsgründe", "Recitals"),
                pfad: Some("eg".into()),
                text: false,
                kinder: std::mem::take(&mut self.erwaegung),
            });
        }
        alles.append(&mut self.wurzel);
        alles.append(&mut self.anhaenge);
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
                // Anhang: `div.eli-container#anx_III`. Nur oberste Anhaenge (ohne Punkt in
                // der Kennung); Unterteile liegen innerhalb und gehoeren zum Anhang.
                "div" if id.starts_with("anx_") && !id[4..].contains('.') => {
                    self.anhang(k, &id[4..]);
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
                bez: None,
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
                bez: None,
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
            bez: None,
            titel: BTreeMap::new(),
            pfad: Some(format!("{pfad}/{nummer}")),
            text: false,
            kinder: Vec::new(),
        }
    }

    /// Ein Anhang als eine Fundstelle: Kopfzeile ("ANHANG III") und Titel stehen als
    /// `p.oj-doc-ti`, danach Absaetze und Tabellen - bei Anhaengen auch echte,
    /// mehrzeilige Tabellen (Meldeformulare, Vorlagen), die nicht verloren gehen duerfen.
    fn anhang(&mut self, el: ElementRef, kennung: &str) {
        let mut zeilen: Vec<String> = Vec::new();
        for kind in el.children().filter_map(ElementRef::wrap) {
            let klasse = kind.value().attr("class").unwrap_or("");
            if kind.value().name() == "p" && klasse.contains("oj-doc-ti") {
                let t = reintext(kind);
                if !t.is_empty() {
                    zeilen.push(t);
                }
            }
        }
        // Erste Zeile: "ANHANG III" bzw. "ANNEX III" - die Nummer steht dahinter.
        let kopf = zeilen.first().cloned().unwrap_or_default();
        let nummer = kopf
            .split_once(' ')
            .map(|(_, n)| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| kennung.to_string());
        let titel = zeilen.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");

        let id = format!("anx_{kennung}");
        let bloecke = anhang_bloecke(el);
        if !bloecke.is_empty() {
            self.texte.insert(id.clone(), bloecke);
        }
        let mut t = BTreeMap::new();
        if !titel.is_empty() {
            t.insert("de".to_string(), titel);
        }
        self.anhaenge.push(Knoten {
            id,
            art: Art::Anhang,
            nummer: Some(nummer.clone()),
            bez: None,
            titel: t,
            pfad: Some(format!("anh/{nummer}")),
            text: false,
            kinder: Vec::new(),
        });
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
            if let Some(Block::P { t }) = bloecke.first_mut()
                && let Some(n) = marke_lesen(t) {
                    abs_nr = Some(n);
                    *t = marke_weg(t);
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
                bez: None,
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
            bez: None,
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

/// Bloecke eines Anhangs. Wie `bloecke_von`, aber ohne die Titelzeilen
/// (`oj-doc-ti`, die stehen im Knoten), mit vollstaendigen mehrzeiligen Tabellen und
/// rekursiv: Anhaenge liegen oft in zusaetzlichen `div`-Huellen oder tragen ihre
/// Nummerierung in `div.oj-enumeration-spacing` (Marke und Text als zwei `p`).
fn anhang_bloecke(el: ElementRef) -> Vec<Block> {
    let mut aus: Vec<Block> = Vec::new();
    let mut liste: Vec<Punkt> = Vec::new();
    anhang_sammeln(el, &mut aus, &mut liste);
    abschliessen(&mut aus, &mut liste);
    aus
}

fn abschliessen(aus: &mut Vec<Block>, liste: &mut Vec<Punkt>) {
    if !liste.is_empty() {
        aus.push(Block::Liste { p: std::mem::take(liste) });
    }
}

fn anhang_sammeln(el: ElementRef, aus: &mut Vec<Block>, liste: &mut Vec<Punkt>) {
    for kind in el.children().filter_map(ElementRef::wrap) {
        let klasse = kind.value().attr("class").unwrap_or("");
        match kind.value().name() {
            "p" if klasse.contains("oj-doc-ti") => {}
            "p" => {
                let t = reintext(kind);
                if !t.is_empty() {
                    abschliessen(aus, liste);
                    aus.push(Block::P { t });
                }
            }
            // Zeile aus Marke und Text: <div class="oj-enumeration-spacing"><p>1. </p><p>Text</p></div>
            "div" if klasse.contains("oj-enumeration-spacing") => {
                let teile: Vec<String> = kind
                    .children()
                    .filter_map(ElementRef::wrap)
                    .map(reintext)
                    .filter(|t| !t.is_empty())
                    .collect();
                match teile.as_slice() {
                    [marke, rest @ ..] if !rest.is_empty() && ist_marke(marke) => {
                        liste.push(Punkt { m: marke.clone(), t: rest.join(" "), u: Vec::new() });
                    }
                    [] => {}
                    alle => {
                        abschliessen(aus, liste);
                        aus.push(Block::P { t: alle.join(" ") });
                    }
                }
            }
            "div" => anhang_sammeln(kind, aus, liste),
            "table" => {
                let zeilen = eigene_zeilen(kind);
                if zeilen.len() > 1 {
                    // Echte Tabelle: jede Zeile ein Punkt, die Zellen mit " | " getrennt.
                    for tr in zeilen {
                        let zellen: Vec<String> = tr
                            .children()
                            .filter_map(ElementRef::wrap)
                            .filter(|z| z.value().name() == "td" || z.value().name() == "th")
                            .map(reintext)
                            .filter(|t| !t.is_empty())
                            .collect();
                        if !zellen.is_empty() {
                            liste.push(Punkt { m: String::new(), t: zellen.join(" | "), u: Vec::new() });
                        }
                    }
                } else if let Some(p) = punkt_von_tabelle(kind) {
                    liste.push(p);
                }
            }
            _ => {}
        }
    }
}

/// Kurze Aufzaehlungsmarke wie "1.", "a)", "(2)", "iii)".
fn ist_marke(t: &str) -> bool {
    t.chars().count() <= 6
        && (t.ends_with('.') || t.ends_with(')'))
        && t.chars().any(|c| c.is_alphanumeric())
}

/// Die eigenen Zeilen einer Tabelle (ohne die von verschachtelten Tabellen).
fn eigene_zeilen(t: ElementRef) -> Vec<ElementRef> {
    let mut aus = Vec::new();
    for kind in t.children().filter_map(ElementRef::wrap) {
        match kind.value().name() {
            "tr" => aus.push(kind),
            "tbody" | "thead" | "tfoot" => aus.extend(
                kind.children()
                    .filter_map(ElementRef::wrap)
                    .filter(|k| k.value().name() == "tr"),
            ),
            _ => {}
        }
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

    /// Ausschnitt in der Form, in der CELLAR ausliefert: Bezugsvermerk,
    /// Erwaegungsgrund, Kapitel mit Abschnitt, Artikel mit nummerierten
    /// Absaetzen und eine Aufzaehlung in der Tabellenform der Quelle.
    const AUSSCHNITT: &str = r#"<html><body>
      <div class="eli-subdivision" id="cit_1"><p class="oj-normal">gestützt auf den Vertrag …</p></div>
      <div class="eli-subdivision" id="rct_47"><p class="oj-normal">(47) Die Finanzunternehmen sollten …</p></div>
      <p class="oj-ti-section-1">KAPITEL II</p>
      <p class="oj-ti-section-2">IKT-Risikomanagement</p>
      <p class="oj-ti-section-1">Abschnitt I</p>
      <p class="oj-ti-section-2">Allgemeine Bestimmungen</p>
      <div class="eli-subdivision" id="art_6">
        <div class="eli-title"><p class="oj-ti-art">Artikel 6</p><p class="oj-sti-art">IKT-Risikomanagementrahmen</p></div>
        <div id="006.001"><p class="oj-normal">(1) Die Finanzunternehmen verfügen über einen Rahmen.</p></div>
        <div id="006.002">
          <p class="oj-normal">(2) Der Rahmen umfasst</p>
          <table><tr><td><p class="oj-normal">a)</p></td><td><p class="oj-normal">Strategien,</p></td></tr></table>
          <table><tr><td><p class="oj-normal">b)</p></td><td><p class="oj-normal">Verfahren.</p></td></tr></table>
        </div>
      </div>
      <div class="eli-subdivision" id="art_7">
        <div class="eli-title"><p class="oj-ti-art">Artikel 7</p><p class="oj-sti-art">IKT-Systeme</p></div>
        <p class="oj-normal">Die Finanzunternehmen verwenden angemessene Systeme.</p>
      </div>
    </body></html>"#;

    #[test]
    fn gliederung_aus_dem_ausschnitt() {
        let g = parse(AUSSCHNITT).unwrap();
        // Bezugsvermerke und Erwaegungsgruende stehen vorn, dann das Kapitel.
        let pfade: Vec<&str> =
            g.knoten.iter().map(|k| k.pfad.as_deref().unwrap_or("")).collect();
        assert_eq!(pfade, ["bezug", "eg", "kap/II"]);
        assert_eq!(g.knoten[0].kinder[0].pfad.as_deref(), Some("bezug/1"));
        assert_eq!(g.knoten[1].kinder[0].pfad.as_deref(), Some("eg/47"));

        let kapitel = &g.knoten[2];
        assert_eq!(kapitel.titel.get("de").map(String::as_str), Some("IKT-Risikomanagement"));
        // Abschnitt und Artikel haengen im Kapitel.
        let abschnitt = kapitel.kinder.iter().find(|k| k.art == Art::Abschnitt).unwrap();
        assert_eq!(abschnitt.pfad.as_deref(), Some("kap/II/abschnitt/I"));
        assert_eq!(abschnitt.titel.get("de").map(String::as_str), Some("Allgemeine Bestimmungen"));
    }

    #[test]
    fn artikel_absaetze_und_aufzaehlung() {
        let g = parse(AUSSCHNITT).unwrap();
        // Artikel 6 mit zwei Absaetzen - die Absatzmarke wandert in die Nummer.
        let a6 = g.texte.get("006.001").expect("Absatz 1 hat Text");
        assert_eq!(a6[0].nur_text(), "Die Finanzunternehmen verfügen über einen Rahmen.");
        let a6_2 = g.texte.get("006.002").unwrap();
        assert!(matches!(a6_2[1], Block::Liste { .. }), "die Tabellenform wird Liste");
        if let Block::Liste { p } = &a6_2[1] {
            assert_eq!(p.len(), 2);
            assert_eq!(p[0].m, "a)");
            assert_eq!(p[1].t, "Verfahren.");
        }

        // Artikel 7 hat keine nummerierten Absaetze - der Text haengt am Artikel.
        assert!(g.texte.contains_key("art_7"));
        assert!(!g.texte.contains_key("art_6"), "Artikel 6 traegt seinen Text in den Absaetzen");
    }

    /// Anhang wie bei CELLAR: Kopfzeile und Titel als `p.oj-doc-ti`, danach Text,
    /// eine verschachtelte Aufzaehlung und eine echte mehrzeilige Tabelle.
    const MIT_ANHANG: &str = r#"<html><body>
      <div class="eli-subdivision" id="art_1">
        <div class="eli-title"><p class="oj-ti-art">Artikel 1</p><p class="oj-sti-art">Gegenstand</p></div>
        <p class="oj-normal">Diese Verordnung regelt Folgendes.</p>
      </div>
      <div id="L_x.012701.fmx"><div class="eli-container" id="anx_III">
        <p class="oj-doc-ti">ANHANG III</p>
        <p class="oj-doc-ti">Hochrisiko-Systeme gemäß Artikel 6 Absatz 2</p>
        <p class="oj-normal">Als Hochrisiko-Systeme gelten die folgenden Bereiche:</p>
        <table><tbody><tr><td><p>1.</p></td><td><p>Biometrie:</p>
          <table><tbody><tr><td><p>a)</p></td><td><p>Fernidentifizierung.</p></td></tr></tbody></table>
        </td></tr></tbody></table>
        <table><tbody>
          <tr><td><p>Feld</p></td><td><p>Beschreibung</p></td></tr>
          <tr><td><p>LEI</p></td><td><p>Kennung des Unternehmens</p></td></tr>
          <tr><td><p>Rang</p></td><td><p>Rang des Dienstleisters</p></td></tr>
        </tbody></table>
      </div></div>
      <div class="eli-container" id="anx_IV"><p class="oj-doc-ti">ANHANG IV</p>
        <p class="oj-doc-ti">Technische Dokumentation</p><p class="oj-normal">Inhalt.</p></div>
    </body></html>"#;

    #[test]
    fn anhang_wird_fundstelle_mit_nummer_titel_und_deep_link() {
        let g = parse(MIT_ANHANG).unwrap();
        let pfade: Vec<&str> = g.knoten.iter().map(|k| k.pfad.as_deref().unwrap_or("")).collect();
        assert_eq!(pfade, ["art/1", "anh/III", "anh/IV"], "Anhaenge kommen nach den Artikeln");
        let a3 = &g.knoten[1];
        assert_eq!(a3.art, Art::Anhang);
        assert_eq!(a3.nummer.as_deref(), Some("III"));
        assert_eq!(
            a3.titel.get("de").map(String::as_str),
            Some("Hochrisiko-Systeme gemäß Artikel 6 Absatz 2")
        );
        assert!(a3.text, "der Anhang traegt Text");
        // Die Titelzeilen stehen im Knoten, nicht zusaetzlich im Text.
        let b = &g.texte["anx_III"];
        assert!(b[0].nur_text().starts_with("Als Hochrisiko-Systeme gelten"));
        assert!(!b.iter().any(|x| x.nur_text().contains("ANHANG III")));
    }

    #[test]
    fn mehrzeilige_tabellen_im_anhang_verlieren_keine_zeile() {
        let g = parse(MIT_ANHANG).unwrap();
        let b = &g.texte["anx_III"];
        let alle: Vec<String> = b
            .iter()
            .flat_map(|x| match x {
                Block::Liste { p } => p.iter().map(|q| format!("{} {}", q.m, q.t)).collect::<Vec<_>>(),
                Block::P { t } => vec![t.clone()],
            })
            .collect();
        // Die verschachtelte Aufzaehlung bleibt als Liste erhalten ...
        assert!(alle.iter().any(|z| z.contains("Biometrie")), "{alle:?}");
        // ... und alle drei Zeilen der echten Tabelle sind da, mit ihren Zellen.
        assert!(alle.iter().any(|z| z.trim() == "Feld | Beschreibung"), "{alle:?}");
        assert!(alle.iter().any(|z| z.trim() == "LEI | Kennung des Unternehmens"), "{alle:?}");
        assert!(alle.iter().any(|z| z.trim() == "Rang | Rang des Dienstleisters"), "{alle:?}");
    }

    /// Anhang, dessen Nummerierung in `div.oj-enumeration-spacing` steht (AI Act, Anhang VI),
    /// dazu eine zusaetzliche `div`-Huelle um einen Unterabschnitt.
    const ANHANG_OHNE_TABELLEN: &str = r#"<html><body>
      <div class="eli-subdivision" id="art_1"><div class="eli-title"><p class="oj-sti-art">Gegenstand</p></div>
        <p class="oj-normal">Text.</p></div>
      <div class="eli-container" id="anx_VI">
        <p class="oj-doc-ti">ANHANG VI</p>
        <p class="oj-doc-ti">Konformitätsbewertung auf der Grundlage einer internen Kontrolle</p>
        <div class="oj-enumeration-spacing"><p style="display: inline;">1. </p>
          <p style="display: inline;"><span>Das Verfahren ist das Verfahren gemäß den Nummern 2 bis 4.</span></p></div>
        <div class="oj-enumeration-spacing"><p style="display: inline;">2. </p>
          <p style="display: inline;"><span>Der Anbieter überprüft das Qualitätsmanagementsystem.</span></p></div>
        <p class="oj-ti-grseq-1">Teil B</p>
        <div class="unterabschnitt"><p class="oj-normal">Verschachtelter Absatz.</p></div>
      </div>
    </body></html>"#;

    #[test]
    fn anhang_mit_nummerierungszeilen_und_huellen_behaelt_seinen_text() {
        let g = parse(ANHANG_OHNE_TABELLEN).unwrap();
        let a = g.knoten.iter().find(|k| k.art == Art::Anhang).expect("Anhang VI");
        assert_eq!(a.pfad.as_deref(), Some("anh/VI"));
        assert!(a.text, "der Anhang traegt Text");
        let b = &g.texte["anx_VI"];
        let Block::Liste { p } = &b[0] else { panic!("zuerst die Nummerierung als Liste: {b:?}") };
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].m, "1.");
        assert!(p[0].t.starts_with("Das Verfahren ist"));
        assert_eq!(p[1].m, "2.");
        // Zwischenueberschrift und verschachtelter Absatz bleiben erhalten.
        let rest: Vec<String> = b[1..].iter().map(|x| x.nur_text()).collect();
        assert_eq!(rest, ["Teil B", "Verschachtelter Absatz."]);
    }

    #[test]
    fn aufzaehlungsmarken_werden_von_text_unterschieden() {
        assert!(ist_marke("1."));
        assert!(ist_marke("a)"));
        assert!(ist_marke("(2)"));
        assert!(ist_marke("iii)"));
        assert!(!ist_marke("Die Verfahren"));
        assert!(!ist_marke("."));
        assert!(!ist_marke("Einleitung."), "zu lang fuer eine Marke");
    }

    #[test]
    fn ohne_struktur_wird_abgebrochen_statt_geleert() {
        // Aendert die Quelle ihren Aufbau, darf der Parser nicht "nichts" liefern -
        // sonst wuerde ein Lauf den Bestand mit einer leeren Fassung ueberschreiben.
        assert!(parse("<html><body><p>Zugriff verweigert</p></body></html>").is_err());
        assert!(parse("kein HTML").is_err());
    }

    #[test]
    fn kopfzeilen_beider_sprachen() {
        assert_eq!(zerlege_kopf("KAPITEL II"), Some((Art::Kapitel, "II".into())));
        assert_eq!(zerlege_kopf("CHAPTER III"), Some((Art::Kapitel, "III".into())));
        assert_eq!(zerlege_kopf("Abschnitt I"), Some((Art::Abschnitt, "I".into())));
        assert_eq!(zerlege_kopf("SECTION II"), Some((Art::Abschnitt, "II".into())));
        assert_eq!(zerlege_kopf("ANHANG"), None, "ohne Nummer kein Knoten");
        assert_eq!(zerlege_kopf("Irgendwas X"), None);
    }
}
