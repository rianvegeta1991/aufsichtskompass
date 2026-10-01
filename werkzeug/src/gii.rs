//! Konnektor fuer **gesetze-im-internet.de** (Bundesministerium der Justiz / juris).
//!
//! Je Gesetz liegt dort eine ZIP-Datei mit einer XML-Fassung bereit:
//! `https://www.gesetze-im-internet.de/<kennung>/xml.zip`. Der Server schickt `ETag`
//! und `Last-Modified` - wie bei CELLAR ist damit die Change-Detection moeglich.
//!
//! Aufbau der Quelle (gii-norm.dtd):
//!   * `<norm>` ist alles - das Gesetz selbst, jede Gliederungseinheit und jeder Paragraf.
//!   * Gliederung: `<metadaten><gliederungseinheit><gliederungskennzahl>020010030` -
//!     ein Schluessel aus **Dreiergruppen**, die Laenge gibt die Ebene an
//!     (3 = Teil, 6 = Kapitel, 9 = Abschnitt).
//!   * Paragraf: `<metadaten><enbez>§ 23</enbez><titel>...` und der Text in
//!     `<textdaten><text><Content><P>(1) ...</P>`.
//!   * Aufzaehlungen stehen **im** Absatz: `<DL><DT>1.</DT><DD><LA>Text</LA></DD></DL>`,
//!     verschachtelt bis mehrere Ebenen tief. Nach der Liste kann noch Text folgen.
//!
//! Absatzmarken sind hier `(1)`, aber auch `(1a)`, `(2b)` - anders als im EU-Recht.

use crate::modell::{Art, Block, Knoten, Punkt};
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::io::Read;

pub const BASIS: &str = "https://www.gesetze-im-internet.de";
const KENNUNG: &str = "Aufsichtskompass/0.1 (private Lernanwendung)";

pub struct Abruf {
    /// Abgerufene Adresse - steht im Protokoll des Laufs und in Fehlermeldungen.
    #[allow(dead_code)]
    pub url: String,
    pub etag: Option<String>,
    pub stand: Option<String>,
    pub xml: String,
}

/// Holt die XML-Fassung eines Gesetzes und packt sie aus.
pub fn hole(
    leine: &crate::netz::Leine,
    kennung: &str,
    etag: Option<&str>,
) -> Result<Option<Abruf>> {
    let url = format!("{BASIS}/{kennung}/xml.zip");
    leine.erlaubt(&url)?;
    let klient = leine.klient(KENNUNG, 180)?;
    let mut anfrage = klient.get(&url);
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
    let etag = kopfwert(antwort.headers(), "etag");
    let stand = kopfwert(antwort.headers(), "last-modified");
    let daten = antwort.bytes()?;

    let mut archiv = zip::ZipArchive::new(std::io::Cursor::new(daten))
        .with_context(|| format!("{url} ist kein lesbares ZIP"))?;
    let name = (0..archiv.len())
        .filter_map(|i| archiv.by_index(i).ok().map(|d| d.name().to_string()))
        .find(|n| n.to_lowercase().ends_with(".xml"))
        .with_context(|| format!("{url} enthaelt keine XML-Datei"))?;
    let mut xml = String::new();
    archiv.by_name(&name)?.read_to_string(&mut xml)?;
    if xml.len() < 2000 {
        bail!("{url}: XML mit nur {} Zeichen - Quelle pruefen", xml.len());
    }
    Ok(Some(Abruf { url, etag, stand, xml }))
}

fn kopfwert(kopf: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    kopf.get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

pub struct Geparst {
    pub knoten: Vec<Knoten>,
    pub texte: BTreeMap<String, Vec<Block>>,
    /// Amtliche Kurzbezeichnung aus den Metadaten (z. B. "VAG 2016").
    pub jurabk: Option<String>,
}

/// Zerlegt die XML-Fassung. `nur` beschraenkt auf bestimmte Paragrafen
/// (z. B. ["238", "239", "257"] fuer die IT-relevanten Teile des HGB).
pub fn parse(xml: &str, nur: &[String]) -> Result<Geparst> {
    let dok = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() },
    )
    .context("XML von gesetze-im-internet.de nicht lesbar - Quellstruktur pruefen")?;

    let mut lauf = Lauf::default();
    for norm in dok.root_element().children().filter(|n| n.has_tag_name("norm")) {
        lauf.norm(norm, nur);
    }
    if lauf.paragrafen == 0 {
        bail!("kein einziger Paragraf geparst - Daten bleiben unberuehrt");
    }
    let mut knoten = lauf.fertig();
    if !nur.is_empty() {
        // Bei einem Auszug bleibt sonst das ganze Gliederungsgeruest ohne Inhalt
        // stehen - beim HGB waeren das 92 leere Aeste.
        aufraeumen(&mut knoten);
    }
    markiere_text(&mut knoten, &lauf.texte);
    Ok(Geparst { knoten, texte: lauf.texte, jurabk: lauf.jurabk })
}

/// Entfernt Gliederungsknoten, unter denen kein Paragraf mehr haengt.
fn aufraeumen(kn: &mut Vec<Knoten>) {
    kn.retain_mut(|k| {
        aufraeumen(&mut k.kinder);
        matches!(k.art, Art::Paragraf | Art::Anhang | Art::Absatz) || !k.kinder.is_empty()
    });
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
    /// Offene Gliederungsebenen als Pfad von Indizes in den Baum.
    ebenen: Vec<(usize, usize)>, // (Laenge der Gliederungskennzahl, Index auf dieser Ebene)
    texte: BTreeMap<String, Vec<Block>>,
    jurabk: Option<String>,
    paragrafen: usize,
}

impl Lauf {
    fn fertig(&mut self) -> Vec<Knoten> {
        std::mem::take(&mut self.wurzel)
    }

    fn norm(&mut self, norm: roxmltree::Node, nur: &[String]) {
        let Some(md) = kind(norm, "metadaten") else { return };
        if self.jurabk.is_none() {
            self.jurabk = text_von(kind(md, "jurabk"));
        }

        if let Some(ge) = kind(md, "gliederungseinheit") {
            self.gliederung(ge);
            return;
        }

        let Some(enbez) = text_von(kind(md, "enbez")) else { return };
        let (art, nummer) = match kennzeichnung(&enbez) {
            Some(x) => x,
            None => return, // "Inhaltsübersicht" und Ähnliches
        };
        if !nur.is_empty() {
            // Auszug: nur die benannten Paragrafen, keine Anlagen.
            if art != Art::Paragraf || !nur.iter().any(|n| n == &nummer) {
                return;
            }
        }
        self.paragrafen += 1;

        let id = match art {
            Art::Anhang => format!("anl_{nummer}"),
            _ => format!("par_{nummer}"),
        };
        let pfad = match art {
            Art::Anhang => format!("anl/{nummer}"),
            _ => format!("par/{nummer}"),
        };
        let mut titel = BTreeMap::new();
        if let Some(t) = text_von(kind(md, "titel")) {
            titel.insert("de".to_string(), t);
        }

        // Text: <textdaten><text><Content><P>…
        let mut kinder = Vec::new();
        if let Some(content) = kind(md.parent().unwrap(), "textdaten")
            .and_then(|td| kind(td, "text"))
            .and_then(|t| kind(t, "Content"))
        {
            let absaetze = absaetze_von(content);
            if absaetze.len() == 1 && absaetze[0].0.is_none() {
                // Kein "(1)" - der Text haengt unmittelbar am Paragrafen.
                if !absaetze[0].1.is_empty() {
                    self.texte.insert(id.clone(), absaetze[0].1.clone());
                }
            } else {
                for (marke, bloecke) in absaetze {
                    if bloecke.is_empty() {
                        continue;
                    }
                    let nr = marke.unwrap_or_else(|| (kinder.len() + 1).to_string());
                    let kid = format!("{id}.{nr}");
                    self.texte.insert(kid.clone(), bloecke);
                    kinder.push(Knoten {
                        id: kid,
                        art: Art::Absatz,
                        nummer: Some(nr.clone()),
                        bez: None,
                        titel: BTreeMap::new(),
                        pfad: Some(format!("{pfad}/abs/{nr}")),
                        text: false,
                        kinder: Vec::new(),
                    });
                }
            }
        }

        let knoten = Knoten {
            id,
            art,
            nummer: Some(nummer.clone()),
            bez: Some(match art {
                Art::Anhang => format!("Anlage {nummer}"),
                _ => format!("§ {nummer}"),
            }),
            titel,
            pfad: Some(pfad),
            text: false,
            kinder,
        };
        self.einhaengen(knoten);
    }

    /// Neue Gliederungsebene oeffnen. Die Laenge der Kennzahl gibt die Tiefe vor.
    fn gliederung(&mut self, ge: roxmltree::Node) {
        let kennzahl = text_von(kind(ge, "gliederungskennzahl")).unwrap_or_default();
        let bez = text_von(kind(ge, "gliederungsbez")).unwrap_or_default();
        let titel = text_von(kind(ge, "gliederungstitel"));
        if kennzahl.is_empty() || bez.is_empty() {
            return;
        }
        let tiefe = kennzahl.len();
        let (art, nummer) = einheit(&bez);

        // Alle Ebenen schliessen, die tiefer oder gleich tief liegen.
        while self.ebenen.last().is_some_and(|(t, _)| *t >= tiefe) {
            self.ebenen.pop();
        }

        let mut titelkarte = BTreeMap::new();
        if let Some(t) = titel {
            titelkarte.insert("de".to_string(), t);
        }
        // Pfad aus der Gliederungskennzahl: sie ist eindeutig und bleibt stabil,
        // waehrend "Erstes Buch" sich weder gut in eine URL noch in eine Nummer fuegt.
        let knoten = Knoten {
            id: format!("g_{kennzahl}"),
            art,
            nummer: if nummer.is_empty() { None } else { Some(nummer) },
            bez: Some(bez.clone()),
            titel: titelkarte,
            pfad: Some(format!("gl/{kennzahl}")),
            text: false,
            kinder: Vec::new(),
        };
        let index = self.anhaengen(knoten);
        self.ebenen.push((tiefe, index));
    }

    /// Haengt einen Knoten an die aktuell offene Ebene und gibt seinen Index zurueck.
    fn anhaengen(&mut self, knoten: Knoten) -> usize {
        let mut liste = &mut self.wurzel;
        for (_, i) in &self.ebenen {
            liste = &mut liste[*i].kinder;
        }
        liste.push(knoten);
        liste.len() - 1
    }

    fn einhaengen(&mut self, knoten: Knoten) {
        self.anhaengen(knoten);
    }
}

// ------------------------------------------------------------------ Textteile

/// Zerlegt den Inhalt eines Paragrafen in Absaetze. Ein Absatz beginnt mit "(1)";
/// Text ohne Marke gehoert zum laufenden Absatz.
fn absaetze_von(content: roxmltree::Node) -> Vec<(Option<String>, Vec<Block>)> {
    let mut aus: Vec<(Option<String>, Vec<Block>)> = Vec::new();
    for p in content.children().filter(|n| n.has_tag_name("P")) {
        let bloecke = bloecke_von_p(p);
        if bloecke.is_empty() {
            continue;
        }
        // Fuehrende Marke nur im ersten Textblock suchen.
        let marke = match bloecke.first() {
            Some(Block::P { t }) => marke_lesen(t),
            _ => None,
        };
        match marke {
            Some(m) => {
                let mut bloecke = bloecke;
                if let Some(Block::P { t }) = bloecke.first_mut() {
                    *t = marke_weg(t);
                }
                bloecke.retain(|b| !matches!(b, Block::P { t } if t.is_empty()));
                aus.push((Some(m), bloecke));
            }
            None => match aus.last_mut() {
                Some((_, vorhandene)) => vorhandene.extend(bloecke),
                None => aus.push((None, bloecke)),
            },
        }
    }
    aus
}

/// Ein `<P>`: Text, dazwischen Aufzaehlungen, danach ggf. weiterer Text.
fn bloecke_von_p(p: roxmltree::Node) -> Vec<Block> {
    let mut aus = Vec::new();
    let mut puffer = String::new();
    for kind in p.children() {
        if kind.is_text() {
            puffer.push_str(kind.text().unwrap_or(""));
        } else if kind.has_tag_name("DL") {
            let t = normalisiere(&puffer);
            if !t.is_empty() {
                aus.push(Block::P { t });
            }
            puffer.clear();
            let punkte = punkte_von_dl(kind);
            if !punkte.is_empty() {
                aus.push(Block::Liste { p: punkte });
            }
        } else if kind.has_tag_name("BR") {
            puffer.push(' ');
        } else if kind.is_element() {
            sammle(kind, &mut puffer);
        }
    }
    let t = normalisiere(&puffer);
    if !t.is_empty() {
        aus.push(Block::P { t });
    }
    aus
}

fn punkte_von_dl(dl: roxmltree::Node) -> Vec<Punkt> {
    let mut aus = Vec::new();
    let mut marke = String::new();
    for kind in dl.children() {
        if kind.has_tag_name("DT") {
            let mut s = String::new();
            sammle(kind, &mut s);
            marke = normalisiere(&s);
        } else if kind.has_tag_name("DD") {
            let mut text = String::new();
            let mut unter = Vec::new();
            for inner in kind.children() {
                if inner.has_tag_name("DL") {
                    unter.extend(punkte_von_dl(inner));
                } else if inner.has_tag_name("LA") {
                    for la in inner.children() {
                        if la.has_tag_name("DL") {
                            unter.extend(punkte_von_dl(la));
                        } else if la.is_text() {
                            text.push_str(la.text().unwrap_or(""));
                        } else if la.is_element() {
                            sammle(la, &mut text);
                        }
                    }
                } else if inner.is_text() {
                    text.push_str(inner.text().unwrap_or(""));
                } else if inner.is_element() {
                    sammle(inner, &mut text);
                }
            }
            let text = normalisiere(&text);
            if !text.is_empty() || !unter.is_empty() {
                aus.push(Punkt { m: std::mem::take(&mut marke), t: text, u: unter });
            }
        }
    }
    aus
}

/// Reiner Text eines Elements; Fussnoten und eingebettete Listen bleiben aussen vor.
fn sammle(el: roxmltree::Node, aus: &mut String) {
    for kind in el.children() {
        if kind.is_text() {
            aus.push_str(kind.text().unwrap_or(""));
        } else if kind.has_tag_name("DL") || kind.has_tag_name("fussnoten") {
            continue;
        } else if kind.has_tag_name("BR") {
            aus.push(' ');
        } else if kind.is_element() {
            sammle(kind, aus);
        }
    }
}

pub fn normalisiere(s: &str) -> String {
    let mut aus = String::with_capacity(s.len());
    let mut leer = false;
    for c in s.chars() {
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

fn kind<'a>(n: roxmltree::Node<'a, 'a>, name: &str) -> Option<roxmltree::Node<'a, 'a>> {
    n.children().find(|c| c.has_tag_name(name))
}

fn text_von(n: Option<roxmltree::Node>) -> Option<String> {
    let n = n?;
    let mut s = String::new();
    sammle(n, &mut s);
    let s = normalisiere(&s);
    if s.is_empty() { None } else { Some(s) }
}

/// "§ 23" -> (Paragraf, "23"); "Anlage 1" -> (Anhang, "1").
fn kennzeichnung(enbez: &str) -> Option<(Art, String)> {
    let e = normalisiere(enbez);
    if let Some(rest) = e.strip_prefix('§') {
        let nummer = rest.trim().trim_end_matches(['.', ':']).to_string();
        if nummer.is_empty() {
            return None;
        }
        return Some((Art::Paragraf, nummer));
    }
    if let Some(rest) = e.strip_prefix("Anlage") {
        let nummer = rest.trim().to_string();
        if !nummer.is_empty() {
            return Some((Art::Anhang, nummer));
        }
    }
    None
}

/// Gliederungsbezeichnung zerlegen. Deutsche Gesetze schreiben sowohl "Teil 1" als
/// auch "Erstes Buch" oder "Zweiter Abschnitt" - das Typwort kann also vorn oder
/// hinten stehen. Gesucht wird deshalb das Typwort, der Rest ist die Nummer.
fn einheit(bez: &str) -> (Art, String) {
    let b = normalisiere(bez);
    let mut art = Art::Abschnitt;
    let mut rest: Vec<&str> = Vec::new();
    let mut gefunden = false;
    for wort in b.split_whitespace() {
        let sauber = wort.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        let treffer = match sauber.as_str() {
            "buch" | "teil" => Some(Art::Teil),
            "kapitel" => Some(Art::Kapitel),
            "abschnitt" | "unterabschnitt" | "titel" | "untertitel" => Some(Art::Abschnitt),
            _ => None,
        };
        match treffer {
            Some(a) if !gefunden => {
                art = a;
                gefunden = true;
            }
            _ => rest.push(wort),
        }
    }
    (art, rest.join(" "))
}

/// Absatzmarke "(1)", "(1a)", "(12b)".
fn marke_lesen(t: &str) -> Option<String> {
    let rest = t.strip_prefix('(')?;
    let (innen, _) = rest.split_once(')')?;
    if innen.is_empty() || innen.len() > 4 {
        return None;
    }
    let mut ziffern = 0;
    for (i, c) in innen.chars().enumerate() {
        if c.is_ascii_digit() {
            if i > ziffern {
                return None; // Buchstabe vor Ziffer
            }
            ziffern += 1;
        } else if !c.is_ascii_lowercase() {
            return None;
        }
    }
    if ziffern == 0 {
        return None;
    }
    Some(innen.to_string())
}

fn marke_weg(t: &str) -> String {
    if marke_lesen(t).is_some()
        && let Some((_, rest)) = t.split_once(')') {
            return rest.trim_start().to_string();
        }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absatzmarken_mit_buchstaben() {
        assert_eq!(marke_lesen("(1) Versicherungsunternehmen muessen"), Some("1".into()));
        assert_eq!(marke_lesen("(1a) Die Unternehmen"), Some("1a".into()));
        assert_eq!(marke_lesen("(12b) Sonderfall"), Some("12b".into()));
        assert_eq!(marke_weg("(1a) Die Unternehmen"), "Die Unternehmen");
        // Keine Marke: Klammern im laufenden Text
        assert_eq!(marke_lesen("(Produktfreigabeverfahren) Das Verfahren"), None);
        assert_eq!(marke_lesen("(a) Buchstabe zuerst"), None);
    }

    /// Aufbau wie in der XML-Fassung von gesetze-im-internet.de: erst die Norm
    /// mit der Gliederungseinheit, dann die Paragrafen darunter.
    const GESETZ: &str = r#"<?xml version="1.0" encoding="utf-8"?>
    <dokumente>
      <norm><metadaten><jurabk>VAG 2016</jurabk><enbez>Inhaltsübersicht</enbez></metadaten></norm>
      <norm><metadaten><jurabk>VAG 2016</jurabk>
        <gliederungseinheit><gliederungskennzahl>020</gliederungskennzahl>
          <gliederungsbez>Erstes Kapitel</gliederungsbez>
          <gliederungstitel>Vorschriften für die Erstversicherung</gliederungstitel></gliederungseinheit>
      </metadaten></norm>
      <norm><metadaten><jurabk>VAG 2016</jurabk>
        <gliederungseinheit><gliederungskennzahl>020030</gliederungskennzahl>
          <gliederungsbez>Zweiter Abschnitt</gliederungsbez>
          <gliederungstitel>Geschäftsorganisation</gliederungstitel></gliederungseinheit>
      </metadaten></norm>
      <norm>
        <metadaten><jurabk>VAG 2016</jurabk><enbez>§ 23</enbez>
          <titel>Allgemeine Anforderungen an die Geschäftsorganisation</titel></metadaten>
        <textdaten><text><Content>
          <P>(1) Versicherungsunternehmen müssen über eine Geschäftsorganisation verfügen.</P>
          <P>(2) Die Geschäftsorganisation umfasst
            <DL><DT>1.</DT><DD><LA>ein Risikomanagement,</LA></DD>
                <DT>2.</DT><DD><LA>ein internes Kontrollsystem.</LA></DD></DL>
          </P>
        </Content></text></textdaten>
      </norm>
      <norm>
        <metadaten><jurabk>VAG 2016</jurabk><enbez>§ 26</enbez><titel>Risikomanagement</titel></metadaten>
        <textdaten><text><Content><P>Versicherungsunternehmen müssen ein wirksames Risikomanagement haben.</P></Content></text></textdaten>
      </norm>
    </dokumente>"#;

    #[test]
    fn ganzes_gesetz_mit_gliederung() {
        let g = parse(GESETZ, &[]).unwrap();
        assert_eq!(g.jurabk.as_deref(), Some("VAG 2016"));
        // Kapitel -> Abschnitt -> Paragrafen
        assert_eq!(g.knoten.len(), 1);
        let kapitel = &g.knoten[0];
        assert_eq!(kapitel.art, Art::Kapitel);
        assert_eq!(kapitel.bez.as_deref(), Some("Erstes Kapitel"), "Wortform der Quelle bleibt");
        let abschnitt = &kapitel.kinder[0];
        assert_eq!(abschnitt.art, Art::Abschnitt);
        let paragrafen: Vec<&str> =
            abschnitt.kinder.iter().map(|k| k.pfad.as_deref().unwrap_or("")).collect();
        assert_eq!(paragrafen, ["par/23", "par/26"]);

        // § 23 hat zwei Absaetze, § 26 traegt seinen Text unmittelbar.
        let p23 = &abschnitt.kinder[0];
        assert_eq!(p23.bez.as_deref(), Some("§ 23"));
        assert_eq!(p23.kinder.len(), 2);
        assert_eq!(p23.kinder[1].pfad.as_deref(), Some("par/23/abs/2"));
        assert!(g.texte.contains_key("par_26"));
        assert!(!g.texte.contains_key("par_23"));

        // Die Aufzaehlung im Absatz wird zur Liste, der Text davor bleibt stehen.
        let abs2 = g.texte.get("par_23.2").unwrap();
        assert!(matches!(abs2[0], Block::P { .. }));
        let liste = abs2.iter().find_map(|b| match b {
            Block::Liste { p } => Some(p),
            _ => None,
        }).expect("Aufzaehlung erkannt");
        assert_eq!(liste.len(), 2);
        assert_eq!(liste[0].m, "1.");
        assert_eq!(liste[1].t, "ein internes Kontrollsystem.");
    }

    #[test]
    fn auszug_nimmt_nur_die_benannten_paragrafen() {
        let g = parse(GESETZ, &["26".to_string()]).unwrap();
        let mut pfade = Vec::new();
        fn sammle(kn: &[Knoten], aus: &mut Vec<String>) {
            for k in kn {
                if let Some(p) = &k.pfad {
                    aus.push(p.clone());
                }
                sammle(&k.kinder, aus);
            }
        }
        sammle(&g.knoten, &mut pfade);
        assert!(pfade.contains(&"par/26".to_string()));
        assert!(!pfade.contains(&"par/23".to_string()));
        // Leere Gliederungsaeste werden weggeraeumt - der Ast ueber § 26 bleibt stehen.
        // Gliederungsknoten tragen ihre Kennzahl im Pfad ("gl/020030").
        assert_eq!(pfade, ["gl/020", "gl/020030", "par/26"], "nur der tragende Ast bleibt");
    }

    #[test]
    fn ohne_paragrafen_wird_abgebrochen() {
        // Liefert die Quelle nichts Brauchbares, darf der Bestand nicht ueberschrieben werden.
        let leer = r#"<?xml version="1.0"?><dokumente><norm><metadaten><jurabk>X</jurabk>
          <enbez>Inhaltsübersicht</enbez></metadaten></norm></dokumente>"#;
        assert!(parse(leer, &[]).is_err());
        assert!(parse("kein XML", &[]).is_err());
        // Auszug, dessen Paragraf im Gesetz fehlt: ebenfalls Abbruch statt leerer Fassung.
        assert!(parse(GESETZ, &["999".to_string()]).is_err());
    }

    #[test]
    fn kennzeichnungen() {
        assert_eq!(kennzeichnung("§ 23"), Some((Art::Paragraf, "23".into())));
        assert_eq!(kennzeichnung("§ 7a"), Some((Art::Paragraf, "7a".into())));
        assert_eq!(kennzeichnung("Anlage 1"), Some((Art::Anhang, "1".into())));
        assert_eq!(kennzeichnung("Inhaltsübersicht"), None);
    }

    #[test]
    fn gliederungseinheiten() {
        assert_eq!(einheit("Teil 1"), (Art::Teil, "1".into()));
        assert_eq!(einheit("Kapitel 2"), (Art::Kapitel, "2".into()));
        assert_eq!(einheit("Abschnitt 3"), (Art::Abschnitt, "3".into()));
        assert_eq!(einheit("Unterabschnitt 1"), (Art::Abschnitt, "1".into()));
        // Wortnummerierung des HGB: das Typwort steht hinten.
        assert_eq!(einheit("Erstes Buch"), (Art::Teil, "Erstes".into()));
        assert_eq!(einheit("Zweiter Abschnitt"), (Art::Abschnitt, "Zweiter".into()));
        assert_eq!(einheit("Dritter Titel"), (Art::Abschnitt, "Dritter".into()));
    }
}
