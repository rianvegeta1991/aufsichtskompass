//! Konnektor fuer **Veroeffentlichungen der BaFin** (Phase 2c).
//!
//! Warum der Volltext hier zulaessig ist: Die Nutzungsbedingungen der BaFin
//! (bafin.de, "Haftungsausschluss & Copyright", abgerufen am 01.10.2026) sagen,
//! dass die abrufbaren Inhalte und Dokumente "grundsaetzlich gespeichert,
//! weitergegeben und vervielfaeltigt werden" duerfen - unter deutlicher Angabe der
//! Quelle und, soweit amtliche Werke im Sinne des § 5 Absatz 2 UrhG betroffen sind,
//! **unveraendert**. Deshalb traegt jede so uebernommene Fassung den Quellenhinweis
//! "© Bundesanstalt fuer Finanzdienstleistungsaufsicht / www.bafin.de", und der
//! Wortlaut wird nicht angetastet: dieser Parser gliedert nur, er formuliert nicht.
//! Ein Dokument, das abweichende Bedingungen nennt, gehoert nicht hierher (die
//! Bedingungen lassen Abweichungen im Einzelfall ausdruecklich zu).
//!
//! Zwei Bauarten, beide als HTML auf bafin.de:
//!
//! * **`rundschreiben`** - der Fliesstext steht in `div.l-article__content`:
//!   `h2`/`h3`/`h4` tragen die nummerierte Gliederung ("9.", "9.1", "9.1.1"),
//!   jeder `p` beginnt mit seiner **Randziffer** ("31 Die Unternehmen ..."). Genau
//!   so wird zitiert ("MaGo Rz. 31"), deshalb ist die Randziffer der Deep-Link:
//!   `rz/31`. Absaetze ohne Randziffer (Aufzaehlungen "a) ...") und `ul`-Listen
//!   gehoeren zur offenen Randziffer und werden an sie angehaengt.
//! * **`faq`** - Fragen und Antworten in Aufklapp-Feldern. Haengt nicht an den
//!   CSS-Klassen, sondern an den schema.org-Auszeichnungen der Seite:
//!   `[itemtype*="schema.org/Question"]` mit `[itemprop=name]` (Frage) und
//!   `[itemprop=text]` (Antwort). Die `h2` darueber sind die Themengruppen.

use crate::modell::{Art, Block, Knoten, Punkt};
use anyhow::{Context, Result, bail};
use scraper::{ElementRef, Html, Selector};
use std::collections::BTreeMap;

const KENNUNG: &str = "Aufsichtskompass/0.1 (private Lernanwendung)";

/// Quellenhinweis, der mit jeder uebernommenen Fassung mitgeht.
pub const HINWEIS: &str = "© Bundesanstalt für Finanzdienstleistungsaufsicht / www.bafin.de – \
     Wiedergabe unverändert und mit Quellenangabe, wie es die Nutzungsbedingungen der BaFin \
     vorsehen. Rechtsverbindlich ist allein die von der BaFin veröffentlichte Fassung.";

pub struct Abruf {
    /// Abgerufene Adresse - steht im Protokoll des Laufs und in Fehlermeldungen.
    #[allow(dead_code)]
    pub url: String,
    pub etag: Option<String>,
    pub stand: Option<String>,
    pub html: String,
}

pub fn hole(leine: &crate::netz::Leine, url: &str, etag: Option<&str>) -> Result<Option<Abruf>> {
    leine.erlaubt(url)?;
    let klient = leine.klient(KENNUNG, 120)?;
    let mut anfrage = klient.get(url);
    if let Some(e) = etag {
        anfrage = anfrage.header("If-None-Match", e);
    }
    let antwort = anfrage.send().with_context(|| format!("Abruf {url}"))?;
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
    let html = antwort.text()?;
    if html.len() < 5000 {
        bail!("{url} lieferte nur {} Zeichen - vermutlich abgewiesen", html.len());
    }
    Ok(Some(Abruf { url: url.to_string(), etag, stand, html }))
}

pub struct Geparst {
    pub knoten: Vec<Knoten>,
    pub texte: BTreeMap<String, Vec<Block>>,
    /// Titel der Veroeffentlichung, wie die Seite ihn fuehrt.
    pub titel: Option<String>,
    /// Datum, das die Seite nennt (erstes Vorkommen, Form TT.MM.JJJJ).
    pub stand: Option<String>,
}

pub fn parse(html: &str, bauart: &str) -> Result<Geparst> {
    match bauart {
        "rundschreiben" => rundschreiben(html),
        "faq" => faq(html),
        sonst => bail!("Bauart '{sonst}' kennt der BaFin-Konnektor nicht"),
    }
}

fn inhalt<'a>(dok: &'a Html) -> Result<ElementRef<'a>> {
    let wahl = Selector::parse("div.l-article__content").unwrap();
    dok.select(&wahl)
        .next()
        .context("kein 'div.l-article__content' gefunden - Seitenaufbau der BaFin hat sich geaendert")
}

// ------------------------------------------------------------- Rundschreiben

fn rundschreiben(html: &str) -> Result<Geparst> {
    let dok = Html::parse_document(html);
    let bereich = inhalt(&dok)?;

    let mut wurzel: Vec<Knoten> = Vec::new();
    let mut texte: BTreeMap<String, Vec<Block>> = BTreeMap::new();
    // Offene Gliederungsebenen als Kette von Indizes in den Baum.
    let mut ebenen: Vec<usize> = Vec::new();
    let mut offene_rz: Option<String> = None;

    for kind in bereich.children().filter_map(ElementRef::wrap) {
        let name = kind.value().name();
        let roh = reintext(kind);
        if roh.is_empty() {
            continue;
        }
        match name {
            "h2" | "h3" | "h4" => {
                offene_rz = None;
                let tiefe = match name {
                    "h2" => 1,
                    "h3" => 2,
                    _ => 3,
                };
                let (nummer, titel) = kopf_zerlegen(&roh);
                let art = match tiefe {
                    1 => Art::Kapitel,
                    _ => Art::Abschnitt,
                };
                let knoten = Knoten {
                    id: format!("nr_{}", schluessel(&nummer)),
                    art,
                    nummer: Some(nummer.clone()),
                    bez: Some(nummer.clone()),
                    titel: BTreeMap::from([("de".to_string(), titel)]),
                    pfad: Some(format!("nr/{nummer}")),
                    text: false,
                    kinder: Vec::new(),
                };
                einhaengen(&mut wurzel, &mut ebenen, knoten, tiefe);
            }
            "p" => {
                match randziffer(&roh) {
                    Some((nr, rest)) => {
                        let id = format!("rz_{nr}");
                        texte.insert(id.clone(), vec![Block::P { t: rest }]);
                        let knoten = Knoten {
                            id: id.clone(),
                            art: Art::Absatz,
                            nummer: Some(nr.clone()),
                            bez: Some(format!("Rz. {nr}")),
                            titel: BTreeMap::new(),
                            pfad: Some(format!("rz/{nr}")),
                            text: true,
                            kinder: Vec::new(),
                        };
                        anhaengen(&mut wurzel, &ebenen, knoten);
                        offene_rz = Some(id);
                    }
                    // Kein Marker: gehoert zur offenen Randziffer (z. B. "a) ...").
                    None => {
                        if let Some(id) = &offene_rz {
                            let punkt = punkt_zerlegen(&roh);
                            anfuegen(texte.get_mut(id), punkt);
                        }
                    }
                }
            }
            "ul" | "ol" => {
                if let Some(id) = &offene_rz {
                    let wahl = Selector::parse("li").unwrap();
                    for li in kind.select(&wahl) {
                        let t = reintext(li);
                        if !t.is_empty() {
                            anfuegen(texte.get_mut(id), punkt_zerlegen(&t));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if texte.len() < 5 {
        bail!(
            "nur {} Randziffern gefunden - Seitenaufbau pruefen, Daten bleiben unberuehrt",
            texte.len()
        );
    }
    Ok(Geparst {
        knoten: wurzel,
        texte,
        titel: titel_von(&dok),
        stand: datum_von(&dok),
    })
}

/// "9.1.2 Festlegung von Aufgaben" -> ("9.1.2", "Festlegung von Aufgaben")
fn kopf_zerlegen(roh: &str) -> (String, String) {
    let mut teile = roh.splitn(2, char::is_whitespace);
    let erstes = teile.next().unwrap_or("");
    let rest = teile.next().unwrap_or("").trim().to_string();
    let nummer = erstes.trim_end_matches('.').to_string();
    let ist_nummer = !nummer.is_empty()
        && nummer.chars().all(|c| c.is_ascii_digit() || c == '.')
        && nummer.chars().any(|c| c.is_ascii_digit());
    if ist_nummer && !rest.is_empty() {
        (nummer, rest)
    } else {
        // Ueberschrift ohne Nummer (z. B. "Anhang") - dann ist der Titel die Kennung.
        (roh.trim().to_string(), roh.trim().to_string())
    }
}

/// "31 Die Unternehmen ..." -> ("31", "Die Unternehmen ...")
fn randziffer(roh: &str) -> Option<(String, String)> {
    let mut zeichen = roh.char_indices();
    let mut ende = 0;
    for (i, c) in zeichen.by_ref() {
        if c.is_ascii_digit() {
            ende = i + c.len_utf8();
        } else {
            break;
        }
    }
    if ende == 0 || ende > 4 {
        return None;
    }
    let nr = roh[..ende].to_string();
    let rest = roh[ende..].trim_start();
    // Hinter der Randziffer muss Text folgen, und sie darf nicht Teil einer Zahl sein
    // ("1.000 Euro" beginnt mit einer Ziffer, ist aber keine Randziffer).
    if rest.is_empty() || rest.starts_with(['.', ',', '%', ')']) {
        return None;
    }
    Some((nr, rest.to_string()))
}

/// "a) regelmaessig zu bewerten, ..." -> Punkt mit Marke "a)"
fn punkt_zerlegen(roh: &str) -> Punkt {
    let marke = roh
        .split_whitespace()
        .next()
        .filter(|m| m.len() <= 4 && m.ends_with(')'))
        .unwrap_or("")
        .to_string();
    let t = roh[marke.len()..].trim().to_string();
    if marke.is_empty() {
        Punkt { m: String::new(), t: roh.to_string(), u: Vec::new() }
    } else {
        Punkt { m: marke, t, u: Vec::new() }
    }
}

/// Punkt an die Liste der offenen Randziffer anhaengen (und die Liste anlegen,
/// wenn es noch keine gibt).
fn anfuegen(bloecke: Option<&mut Vec<Block>>, punkt: Punkt) {
    let Some(b) = bloecke else { return };
    if let Some(Block::Liste { p }) = b.last_mut() {
        p.push(punkt);
    } else {
        b.push(Block::Liste { p: vec![punkt] });
    }
}

// ------------------------------------------------------------------- FAQ

fn faq(html: &str) -> Result<Geparst> {
    let dok = Html::parse_document(html);
    let bereich = inhalt(&dok)?;

    let mut wurzel: Vec<Knoten> = Vec::new();
    let mut texte: BTreeMap<String, Vec<Block>> = BTreeMap::new();
    let mut nummer = 0usize;
    let mut gruppe = 0usize;
    let mut offene_frage: Option<String> = None;

    for kind in bereich.children().filter_map(ElementRef::wrap) {
        let klasse = kind.value().attr("class").unwrap_or("");
        let roh = reintext(kind);
        match kind.value().name() {
            "h2" => {
                if roh.is_empty() {
                    continue;
                }
                offene_frage = None;
                gruppe += 1;
                wurzel.push(Knoten {
                    id: format!("gr_{gruppe}"),
                    art: Art::Abschnitt,
                    nummer: Some(gruppe.to_string()),
                    bez: None,
                    titel: BTreeMap::from([("de".to_string(), roh)]),
                    pfad: Some(format!("gr/{gruppe}")),
                    text: false,
                    kinder: Vec::new(),
                });
            }
            // Die Frage ist ein Absatz mit der Aufklapp-Klasse der Seite; alles
            // danach bis zur naechsten Frage oder Gruppe ist die Antwort.
            "p" if klasse.contains("c-richtext-accordion__opener") => {
                if roh.is_empty() {
                    continue;
                }
                nummer += 1;
                let id = format!("frage_{nummer}");
                texte.insert(id.clone(), Vec::new());
                let knoten = Knoten {
                    id: id.clone(),
                    art: Art::Control,
                    nummer: Some(nummer.to_string()),
                    bez: Some(format!("Frage {nummer}")),
                    titel: BTreeMap::from([("de".to_string(), roh)]),
                    pfad: Some(format!("frage/{nummer}")),
                    text: true,
                    kinder: Vec::new(),
                };
                match wurzel.last_mut() {
                    Some(g) if g.art == Art::Abschnitt => g.kinder.push(knoten),
                    _ => wurzel.push(knoten),
                }
                offene_frage = Some(id);
            }
            "p" => {
                if roh.is_empty() {
                    continue;
                }
                if let Some(b) = offene_frage.as_ref().and_then(|id| texte.get_mut(id)) {
                    b.push(Block::P { t: roh });
                }
            }
            "ul" | "ol" => {
                if let Some(id) = &offene_frage {
                    let wahl = Selector::parse("li").unwrap();
                    let punkte: Vec<Punkt> = kind
                        .select(&wahl)
                        .map(reintext)
                        .filter(|t| !t.is_empty())
                        .map(|t| punkt_zerlegen(&t))
                        .collect();
                    if let Some(b) = texte.get_mut(id).filter(|_| !punkte.is_empty()) {
                        b.push(Block::Liste { p: punkte });
                    }
                }
            }
            _ => {}
        }
    }

    // Fragen ohne Antwort und Gruppen ohne Fragen weglassen.
    texte.retain(|_, b| !b.is_empty());
    let vorhanden: Vec<String> = texte.keys().cloned().collect();
    for g in wurzel.iter_mut() {
        g.kinder.retain(|k| vorhanden.contains(&k.id));
    }
    wurzel.retain(|k| k.art != Art::Abschnitt || !k.kinder.is_empty());
    if texte.len() < 5 {
        bail!(
            "nur {} Fragen gefunden - Seitenaufbau pruefen, Daten bleiben unberuehrt",
            texte.len()
        );
    }
    Ok(Geparst {
        knoten: wurzel,
        texte,
        titel: titel_von(&dok),
        stand: datum_von(&dok),
    })
}

// --------------------------------------------------------------- Hilfsmittel

/// Haengt einen Gliederungsknoten auf der gewuenschten Tiefe ein.
fn einhaengen(wurzel: &mut Vec<Knoten>, ebenen: &mut Vec<usize>, knoten: Knoten, tiefe: usize) {
    ebenen.truncate(tiefe - 1);
    let weg: Vec<usize> = ebenen.clone();
    let mut stelle = &mut *wurzel;
    for i in &weg {
        stelle = &mut stelle[*i].kinder;
    }
    stelle.push(knoten);
    ebenen.push(stelle.len() - 1);
}

/// Haengt eine Randziffer unter die zuletzt geoeffnete Gliederungsebene.
fn anhaengen(wurzel: &mut Vec<Knoten>, ebenen: &[usize], knoten: Knoten) {
    let mut stelle = &mut *wurzel;
    for i in ebenen {
        stelle = &mut stelle[*i].kinder;
    }
    stelle.push(knoten);
}

/// Text eines Elements, **ohne** Trennzeichen zwischen den Textstuecken.
///
/// Wichtig: Die Seiten der BaFin setzen `abbr` und Links mitten in den Satz
/// ("Versicherungsaufsichtsgesetz (<abbr>VAG</abbr>)"). Wer die Textstuecke mit
/// einem Leerzeichen verbindet, erhaelt "( VAG )" - und veraendert damit den
/// Wortlaut, den die Nutzungsbedingungen unveraendert verlangen. Die Leerzeichen
/// des Quelltexts stehen schon in den Textstuecken; hier wird nur noch
/// zusammengefasst, was mehrfach vorkommt.
fn reintext(el: ElementRef) -> String {
    let roh: String = el.text().collect();
    let mut aus = String::with_capacity(roh.len());
    let mut leer = false;
    for c in roh.chars() {
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

fn titel_von(dok: &Html) -> Option<String> {
    let wahl = Selector::parse("h1").unwrap();
    dok.select(&wahl).map(reintext).find(|t| !t.is_empty())
}

/// Erstes Datum der Form TT.MM.JJJJ auf der Seite, als ISO-Zeitpunkt.
fn datum_von(dok: &Html) -> Option<String> {
    let wahl = Selector::parse("main, body").unwrap();
    let text = dok.select(&wahl).next().map(reintext)?;
    let roh = regex::Regex::new(r"(\d{2})\.(\d{2})\.(\d{4})").unwrap();
    let m = roh.captures(&text)?;
    Some(format!("{}-{}-{}T00:00:00Z", &m[3], &m[2], &m[1]))
}

fn schluessel(k: &str) -> String {
    k.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNDSCHREIBEN: &str = r#"<html><body><main>
      <h1>Rundschreiben 09/2025 (VA) - Mindestanforderungen</h1>
      <p>14.07.2025</p>
      <div class="l-article__content">
        <h2>1. Ziel des Rundschreibens</h2>
        <p>1 Dieses Rundschreiben gibt Hinweise zur Auslegung.</p>
        <h2>9. Allgemeine Anforderungen</h2>
        <h3>9.1 Aufbau- und Ablauforganisation</h3>
        <h4>9.1.1 Allgemeines</h4>
        <p>31 Die Unternehmen entscheiden unter Beruecksichtigung ihres Risikoprofils.</p>
        <h4>9.1.2 Festlegung von Aufgaben</h4>
        <p>32 Eine angemessene Aufbauorganisation erfordert klare Abgrenzung. Dabei gilt:</p>
        <p>a) Aufgaben sind zu beschreiben,</p>
        <p>b) Berichtslinien sind festzulegen.</p>
        <ul><li>c) Vertretungen sind zu regeln.</li></ul>
        <p>33 Daneben sind Vertretungsregelungen klar festzulegen.</p>
        <p>1.000 Euro sind hier keine Randziffer.</p>
        <h4>9.1.3 Angemessene Trennung</h4>
        <p>34 Die Organisationsstruktur muss eine Trennung der Zustaendigkeiten vorsehen.</p>
        <p>35 Die Gesamtverantwortung bleibt bei der Geschaeftsleitung.</p>
      </div></main></body></html>"#;

    #[test]
    fn gliederung_und_randziffern() {
        let g = parse(RUNDSCHREIBEN, "rundschreiben").unwrap();
        assert!(g.titel.as_deref().unwrap_or("").starts_with("Rundschreiben 09/2025"));
        assert_eq!(g.stand.as_deref(), Some("2025-07-14T00:00:00Z"));

        // Zwei Kapitel, das zweite mit Abschnitt und Unterabschnitten.
        let pfade: Vec<&str> = g.knoten.iter().map(|k| k.pfad.as_deref().unwrap_or("")).collect();
        assert_eq!(pfade, ["nr/1", "nr/9"]);
        assert_eq!(g.knoten[0].kinder[0].pfad.as_deref(), Some("rz/1"));
        let abschnitt = &g.knoten[1].kinder[0];
        assert_eq!(abschnitt.pfad.as_deref(), Some("nr/9.1"));
        assert_eq!(abschnitt.titel.get("de").map(String::as_str), Some("Aufbau- und Ablauforganisation"));
        let unter = &abschnitt.kinder[0];
        assert_eq!(unter.pfad.as_deref(), Some("nr/9.1.1"));
        assert_eq!(unter.kinder[0].pfad.as_deref(), Some("rz/31"));
        assert_eq!(unter.kinder[0].bez.as_deref(), Some("Rz. 31"));
    }

    #[test]
    fn aufzaehlung_gehoert_zur_offenen_randziffer() {
        let g = parse(RUNDSCHREIBEN, "rundschreiben").unwrap();
        let rz32 = g.texte.get("rz_32").expect("Rz. 32");
        assert!(matches!(rz32[0], Block::P { .. }));
        let Block::Liste { p } = &rz32[1] else { panic!("Liste erwartet") };
        assert_eq!(p.len(), 3, "zwei Absaetze und ein Listenpunkt");
        assert_eq!(p[0].m, "a)");
        assert_eq!(p[2].m, "c)");
        // Die naechste Randziffer beginnt eine neue Fundstelle ...
        let rz33 = g.texte.get("rz_33").unwrap();
        assert!(rz33[0].nur_text().starts_with("Daneben sind Vertretungsregelungen"));
        // ... und "1.000 Euro" wird nicht als Randziffer 1000 gelesen, sondern
        // als Fortsetzung der offenen Randziffer gefuehrt.
        assert!(!g.texte.contains_key("rz_1000"));
        assert!(
            rz33.iter().any(|b| b.nur_text().contains("1.000 Euro")),
            "haengt an Rz. 33: {rz33:?}"
        );
    }

    /// So liefert bafin.de die Seite aus: die Frage ist ein Absatz mit der
    /// Aufklapp-Klasse, die Antwort sind die Absaetze danach. Die
    /// schema.org-Auszeichnung, die man im Browser sieht, setzt erst das Skript
    /// der Seite - danach darf sich der Parser also nicht richten.
    const FAQ: &str = r#"<html><body><main><h1>FAQs zu DORA</h1><p>12.09.2026</p>
      <div class="l-article__content">
        <h2>Anwendungsbereich</h2>
        <p class="js-richtext-accordion-start c-richtext-accordion__opener">Für welche Unternehmen gilt <abbr title="Digital Operational Resilience Act">DORA</abbr>?</p>
        <p><a href="https://eur-lex.europa.eu/x">Artikel 2</a> Absatz 1 DORA regelt, wer die Vorgaben erfüllen muss.</p>
        <p>Das Begleitgesetz hat den Anwendungsbereich erweitert.</p>
        <p class="s-richtext-accordion-headline c-richtext-accordion__opener">Gilt DORA auch für Kleinstunternehmen?</p>
        <p>Für Kleinstunternehmen gilt der vereinfachte Rahmen nach Artikel 16 DORA.</p>
        <ul><li>a) erster Punkt,</li><li>b) zweiter Punkt.</li></ul>
        <h2>Meldewesen</h2>
        <p class="c-richtext-accordion__opener">Wann ist zu melden?</p>
        <p>Die Fristen stehen in Artikel 19 DORA.</p>
        <p class="c-richtext-accordion__opener">Wer meldet bei Auslagerung?</p>
        <p>Das Finanzunternehmen bleibt verantwortlich.</p>
        <p class="c-richtext-accordion__opener">Wohin geht die Meldung?</p>
        <p>Über das MVP-Portal der Bafin.</p>
        <p class="c-richtext-accordion__opener">Frage ohne Antwort?</p>
        <h2>Leere Gruppe</h2>
      </div></main></body></html>"#;

    #[test]
    fn fragen_haengen_in_ihrer_gruppe() {
        let g = parse(FAQ, "faq").unwrap();
        assert_eq!(g.titel.as_deref(), Some("FAQs zu DORA"));
        assert_eq!(g.stand.as_deref(), Some("2026-09-12T00:00:00Z"));

        // Zwei Gruppen mit Inhalt; die leere Gruppe und die Frage ohne Antwort fallen weg.
        assert_eq!(g.knoten.len(), 2);
        assert_eq!(g.knoten[0].titel.get("de").map(String::as_str), Some("Anwendungsbereich"));
        assert_eq!(g.knoten[0].pfad.as_deref(), Some("gr/1"));
        assert_eq!(g.knoten[0].kinder.len(), 2);
        assert_eq!(g.knoten[1].kinder.len(), 3, "die Frage ohne Antwort zaehlt nicht");
        assert_eq!(g.texte.len(), 5);
        assert!(!g.texte.contains_key("frage_6"), "Frage ohne Antwort wird nicht gefuehrt");

        // Deep-Link und Beschriftung je Frage.
        let f1 = &g.knoten[0].kinder[0];
        assert_eq!(f1.pfad.as_deref(), Some("frage/1"));
        assert_eq!(f1.bez.as_deref(), Some("Frage 1"));
        // Inline-Auszeichnungen duerfen den Wortlaut nicht veraendern: kein " DORA ?"
        assert_eq!(
            f1.titel.get("de").map(String::as_str),
            Some("Für welche Unternehmen gilt DORA?")
        );
    }

    #[test]
    fn antwort_nimmt_alle_absaetze_und_listen_mit() {
        let g = parse(FAQ, "faq").unwrap();
        let a1 = &g.texte["frage_1"];
        assert_eq!(a1.len(), 2, "zwei Absaetze Antwort");
        assert_eq!(a1[0].nur_text(), "Artikel 2 Absatz 1 DORA regelt, wer die Vorgaben erfüllen muss.");
        let a2 = &g.texte["frage_2"];
        assert!(matches!(a2[1], Block::Liste { .. }), "die Liste gehoert zur Antwort");
        let Block::Liste { p } = &a2[1] else { unreachable!() };
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].m, "a)");
        // Die Antwort der naechsten Gruppe bleibt getrennt.
        assert_eq!(g.texte["frage_3"][0].nur_text(), "Die Fristen stehen in Artikel 19 DORA.");
    }

    #[test]
    fn fremder_seitenaufbau_bricht_ab() {
        assert!(parse("<html><body><p>Zugriff verweigert</p></body></html>", "rundschreiben").is_err());
        assert!(parse(RUNDSCHREIBEN, "etwas-anderes").is_err());
    }

    #[test]
    fn randziffern_erkennen_keine_betraege() {
        assert_eq!(randziffer("31 Die Unternehmen"), Some(("31".into(), "Die Unternehmen".into())));
        assert_eq!(randziffer("256 Der Personenkreis"), Some(("256".into(), "Der Personenkreis".into())));
        assert_eq!(randziffer("1.000 Euro sind faellig"), None);
        assert_eq!(randziffer("9.1 Aufbau"), None);
        assert_eq!(randziffer("a) Aufgaben"), None);
        assert_eq!(randziffer("12345 zu viele Stellen"), None);
        assert_eq!(randziffer("Die Unternehmen"), None);
    }
}
