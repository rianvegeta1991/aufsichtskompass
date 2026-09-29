//! `kompass` - Werkzeug hinter dem Aufsichtskompass.
//!
//! Die App selbst ist eine statische Seite; alles, was abgerufen, verglichen und
//! indexiert werden muss, macht dieses Programm und legt das Ergebnis als JSON
//! unter `daten/` ab. Damit braucht ein neues oder geaendertes Regelwerk keinen
//! Code-Patch, sondern nur einen Eintrag in `daten/quellen.json` und einen Lauf.
//!
//! Befehle:
//!   kompass abruf [--rw dora] [--erzwingen]   Quellen abrufen, Aenderungen uebernehmen
//!   kompass index                             Suchindizes neu bauen
//!   kompass pruefen                           Datenbestand auf Konsistenz pruefen

mod cellar;
mod gii;
mod modell;
mod suche;
mod verweise;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use modell::*;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn main() {
    if let Err(e) = lauf() {
        eprintln!("FEHLER: {e:#}");
        std::process::exit(1);
    }
}

fn lauf() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let befehl = args.first().map(|s| s.as_str()).unwrap_or("hilfe");
    let wert = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let schalter = |name: &str| args.iter().any(|a| a == name);
    let daten = daten_ordner(wert("--daten"))?;

    match befehl {
        "abruf" => abruf(&daten, wert("--rw").as_deref(), schalter("--erzwingen")),
        "index" => {
            ableiten(&daten)?;
            Ok(())
        }
        "verweise" => {
            let n = verweise_bauen(&daten)?;
            println!("{n} belegte Verweise gefunden.");
            Ok(())
        }
        "pruefen" => pruefen(&daten),
        _ => {
            println!(
                "kompass <befehl>\n\
                 \n\
                   abruf [--rw <id>] [--erzwingen]  Quellen abrufen, neue Fassungen uebernehmen\n\
                   index                            Suchindizes und Verweise neu bauen\n\
                   verweise                         nur die Verweise aus den Texten sammeln\n\
                   pruefen                          Datenbestand auf Konsistenz pruefen\n\
                 \n\
                 Gemeinsame Schalter: --daten <pfad> (Standard: ../daten bzw. daten)"
            );
            Ok(())
        }
    }
}

fn daten_ordner(angabe: Option<String>) -> Result<PathBuf> {
    if let Some(p) = angabe {
        return Ok(PathBuf::from(p));
    }
    for k in ["daten", "../daten"] {
        let p = PathBuf::from(k);
        if p.is_dir() {
            return Ok(p);
        }
    }
    bail!("Datenordner nicht gefunden - mit --daten <pfad> angeben")
}

// ----------------------------------------------------------------------- Abruf

fn abruf(daten: &Path, nur: Option<&str>, erzwingen: bool) -> Result<()> {
    let liste: Quellenliste = lies_json(&daten.join("quellen.json"))?;
    let mut behandelt = 0;
    for k in &liste.konnektoren {
        if k.aktiv == Some(false) {
            continue;
        }
        if let Some(n) = nur {
            if k.regelwerk != n {
                continue;
            }
        }
        behandelt += 1;
        match k.typ.as_str() {
            "cellar" => abruf_cellar(daten, k, erzwingen)?,
            "gii" => abruf_gii(daten, k, erzwingen)?,
            sonst => println!("[{}] Konnektortyp '{sonst}' noch nicht umgesetzt - uebersprungen", k.regelwerk),
        }
    }
    if behandelt == 0 {
        println!("Kein passender Konnektor in quellen.json gefunden.");
    }
    Ok(())
}

/// Register eines Regelwerks laden und den ETag des letzten Laufs bestimmen.
fn register_laden(
    daten: &Path,
    rw: &str,
    erzwingen: bool,
) -> Result<(PathBuf, Fassungsregister, Option<Fassung>, Option<String>)> {
    let pfad = daten.join("rw").join(rw).join("fassungen.json");
    let register: Fassungsregister = if pfad.exists() {
        lies_json(&pfad)?
    } else {
        Fassungsregister { regelwerk: rw.to_string(), fassungen: Vec::new() }
    };
    let letzte = register.fassungen.last().cloned();
    let etag = if erzwingen { None } else { letzte.as_ref().and_then(|f| f.etag.clone()) };
    Ok((pfad, register, letzte, etag))
}

fn abruf_cellar(daten: &Path, k: &Konnektor, erzwingen: bool) -> Result<()> {
    let rw = &k.regelwerk;
    let celex = k
        .celex
        .as_deref()
        .with_context(|| format!("[{rw}] Konnektor 'cellar' ohne celex"))?;
    let sprachen: Vec<String> = if k.sprachen.is_empty() {
        vec!["deu".into()]
    } else {
        k.sprachen.clone()
    };
    let leit = sprachen[0].clone();
    let (_, _, _, etag) = register_laden(daten, rw, erzwingen)?;

    println!("[{rw}] CELEX {celex}, Sprachen {sprachen:?} - Abruf laeuft ...");
    let Some(haupt) = cellar::hole(celex, &leit, etag.as_deref())? else {
        println!("[{rw}] unveraendert (ETag der Quelle stimmt) - nichts zu tun.");
        return Ok(());
    };

    let geparst = cellar::parse(&haupt.koerper)?;
    let mut knoten = geparst.knoten;
    let mut sprachtexte = vec![(kurz(&leit), texte_aus(&geparst.texte))];

    // Weitere Sprachfassungen: gleiche Kennungen, nur andere Texte und Titel.
    for s in sprachen.iter().skip(1) {
        match cellar::hole(celex, s, None)? {
            Some(a) => {
                let g = cellar::parse(&a.koerper)?;
                let ks = kurz(s);
                titel_uebernehmen(&mut knoten, &titelkarte(&g.knoten), &ks);
                let t = texte_aus(&g.texte);
                println!("[{rw}] Sprachfassung {s}: {} Fundstellen.", t.len());
                sprachtexte.push((ks, t));
            }
            None => println!("[{rw}] Sprachfassung {s} nicht geliefert - uebersprungen."),
        }
    }

    uebernehmen(
        daten,
        Uebernahme {
            rw: rw.clone(),
            quelle: Quelle {
                name: "EUR-Lex / CELLAR (Amt für Veröffentlichungen der EU)".into(),
                url: format!("{}/{celex}", cellar::BASIS),
                celex: Some(celex.to_string()),
                hinweis: "Rechtsverbindlich ist nur die amtlich veröffentlichte Fassung.".into(),
            },
            etag: haupt.etag,
            stand: haupt.stand,
            knoten,
            sprachtexte,
        },
        erzwingen,
    )
}

fn abruf_gii(daten: &Path, k: &Konnektor, erzwingen: bool) -> Result<()> {
    let rw = &k.regelwerk;
    let kennung = k
        .kennung
        .as_deref()
        .with_context(|| format!("[{rw}] Konnektor 'gii' ohne kennung"))?;
    let (_, _, _, etag) = register_laden(daten, rw, erzwingen)?;

    let auswahl = if k.paragraphen.is_empty() {
        String::from("ganzes Gesetz")
    } else {
        format!("nur §§ {}", k.paragraphen.join(", "))
    };
    println!("[{rw}] gesetze-im-internet.de/{kennung} ({auswahl}) - Abruf laeuft ...");
    let Some(haupt) = gii::hole(kennung, etag.as_deref())? else {
        println!("[{rw}] unveraendert (ETag der Quelle stimmt) - nichts zu tun.");
        return Ok(());
    };

    let geparst = gii::parse(&haupt.xml, &k.paragraphen)?;
    if !k.paragraphen.is_empty() {
        let fehlend: Vec<&String> = k
            .paragraphen
            .iter()
            .filter(|p| !geparst.texte.keys().any(|id| id == &format!("par_{p}") || id.starts_with(&format!("par_{p}."))))
            .collect();
        if !fehlend.is_empty() {
            println!("[{rw}] Achtung: diese Paragrafen kamen nicht vor: {fehlend:?}");
        }
    }

    let hinweis = if k.paragraphen.is_empty() {
        "Amtliches Werk (§ 5 UrhG). Rechtsverbindlich ist nur die amtlich veröffentlichte Fassung.".to_string()
    } else {
        format!(
            "Auszug: nur die IT-relevanten Vorschriften (§§ {}). Amtliches Werk (§ 5 UrhG). \
             Rechtsverbindlich ist nur die amtlich veröffentlichte Fassung.",
            k.paragraphen.join(", ")
        )
    };

    uebernehmen(
        daten,
        Uebernahme {
            rw: rw.clone(),
            quelle: Quelle {
                name: format!(
                    "gesetze-im-internet.de ({})",
                    geparst.jurabk.clone().unwrap_or_else(|| kennung.to_string())
                ),
                url: format!("{}/{kennung}/", gii::BASIS),
                celex: None,
                hinweis,
            },
            etag: haupt.etag,
            stand: haupt.stand,
            knoten: geparst.knoten,
            sprachtexte: vec![("de".to_string(), texte_aus(&geparst.texte))],
        },
        erzwingen,
    )
}

/// Ergebnis eines Abrufs, bevor es in den Bestand wandert.
struct Uebernahme {
    rw: String,
    quelle: Quelle,
    etag: Option<String>,
    /// `Last-Modified` der Quelle.
    stand: Option<String>,
    knoten: Vec<Knoten>,
    /// Erste Sprache ist die Leitsprache; ihr Text entscheidet ueber neue Fassungen.
    sprachtexte: Vec<(String, Textdatei)>,
}

/// Vergleicht mit dem Bestand und legt bei Bedarf eine neue Fassung an.
/// Hier laufen CELLAR und gesetze-im-internet.de zusammen - die Versionslogik
/// ist fuer beide dieselbe und soll es bleiben.
fn uebernehmen(daten: &Path, u: Uebernahme, erzwingen: bool) -> Result<()> {
    let rw = &u.rw;
    let (reg_pfad, mut register, letzte, _) = register_laden(daten, rw, erzwingen)?;
    let (leitsprache, texte) = u
        .sprachtexte
        .first()
        .context("Uebernahme ohne Text")?
        .clone();

    if texte.len() < 3 {
        bail!(
            "[{rw}] nur {} Fundstellen geparst - Quellstruktur pruefen, Daten bleiben unberuehrt",
            texte.len()
        );
    }
    let gesamt = gesamthash(&texte);

    // `--erzwingen` heisst: die vorhandene Fassung mit dem aktuellen Parser neu erzeugen.
    // Das ist der Weg, eine Parserkorrektur auf den Bestand anzuwenden, ohne eine Fassung
    // zu erfinden, die es bei der Quelle nie gab.
    let neuschreiben = erzwingen && letzte.is_some();

    if let Some(l) = &letzte {
        if l.hash == gesamt && !neuschreiben {
            println!(
                "[{rw}] Text unveraendert (Fassung {}, {} Fundstellen). ETag wird nachgefuehrt.",
                l.id,
                texte.len()
            );
            let n = register.fassungen.len() - 1;
            register.fassungen[n].etag = u.etag.clone();
            register.fassungen[n].abgerufen = jetzt();
            schreib_json(&reg_pfad, &register, true)?;
            return Ok(());
        }
        if neuschreiben && l.hash != gesamt {
            println!(
                "[{rw}] Achtung: der Text der Fassung {} weicht vom gespeicherten Hash ab. \
                 Mit --erzwingen wird er an Ort und Stelle ersetzt, keine neue Fassung angelegt.",
                l.id
            );
        }
    }

    let stand = if neuschreiben {
        letzte.as_ref().unwrap().stand.clone()
    } else {
        u.stand.clone().unwrap_or_else(jetzt)
    };
    let mut fassung_id = if neuschreiben {
        letzte.as_ref().unwrap().id.clone()
    } else {
        datum_aus(&stand)
    };
    while !neuschreiben && register.fassungen.iter().any(|f| f.id == fassung_id) {
        fassung_id.push('b');
    }
    let ordner = daten.join("rw").join(rw).join(&fassung_id);
    std::fs::create_dir_all(&ordner)?;

    let struktur = Struktur {
        regelwerk: rw.clone(),
        fassung: fassung_id.clone(),
        abgerufen: jetzt(),
        quelle: u.quelle.clone(),
        sprachen: u.sprachtexte.iter().map(|(s, _)| s.clone()).collect(),
        knoten: u.knoten,
    };
    schreib_json(&ordner.join("struktur.json"), &struktur, true)?;
    for (s, t) in &u.sprachtexte {
        schreib_json(&ordner.join(format!("text-{s}.json")), t, true)?;
    }

    if neuschreiben {
        // Nur die Dateien wurden erneuert - kein regulatorisches Ereignis.
        let n = register.fassungen.len() - 1;
        register.fassungen[n].etag = u.etag.clone();
        register.fassungen[n].abgerufen = jetzt();
        register.fassungen[n].hash = gesamt;
        register.fassungen[n].fundstellen = texte.len();
        schreib_json(&reg_pfad, &register, true)?;
        println!("[{rw}] Fassung {fassung_id} neu erzeugt: {} Fundstellen.", texte.len());
        ableiten(daten)?;
        return Ok(());
    }

    // Aenderungsereignis gegen die Vorfassung.
    let mut ereignis = Aenderung {
        zeitpunkt: jetzt(),
        regelwerk: rw.clone(),
        fassung: fassung_id.clone(),
        vorherige: letzte.as_ref().map(|f| f.id.clone()),
        art: if letzte.is_some() { "neue Fassung".into() } else { "Erstaufnahme".into() },
        neu: Vec::new(),
        geaendert: Vec::new(),
        entfallen: Vec::new(),
        quelle: u.quelle.url.clone(),
    };
    if let Some(l) = &letzte {
        let alt_pfad = daten
            .join("rw")
            .join(rw)
            .join(&l.id)
            .join(format!("text-{leitsprache}.json"));
        if alt_pfad.exists() {
            let alt: Textdatei = lies_json(&alt_pfad)?;
            let alte: BTreeSet<&String> = alt.keys().collect();
            let neue: BTreeSet<&String> = texte.keys().collect();
            ereignis.neu = neue.difference(&alte).map(|s| s.to_string()).collect();
            ereignis.entfallen = alte.difference(&neue).map(|s| s.to_string()).collect();
            ereignis.geaendert = texte
                .iter()
                .filter(|(id, f)| alt.get(*id).is_some_and(|a| a.h != f.h))
                .map(|(id, _)| id.clone())
                .collect();
        }
        let n = register.fassungen.len() - 1;
        register.fassungen[n].gueltig_bis = Some(stand.clone());
    }

    register.fassungen.push(Fassung {
        id: fassung_id.clone(),
        stand,
        abgerufen: jetzt(),
        quelle: u.quelle,
        etag: u.etag,
        hash: gesamt,
        gueltig_bis: None,
        fundstellen: texte.len(),
    });
    schreib_json(&reg_pfad, &register, true)?;

    let log_pfad = daten.join("aenderungen.json");
    let mut log: Aenderungslog = if log_pfad.exists() {
        lies_json(&log_pfad)?
    } else {
        Aenderungslog::default()
    };
    println!(
        "[{rw}] Fassung {fassung_id} uebernommen: {} Fundstellen, {} neu, {} geaendert, {} entfallen.",
        texte.len(),
        ereignis.neu.len(),
        ereignis.geaendert.len(),
        ereignis.entfallen.len()
    );
    log.ereignisse.push(ereignis);
    schreib_json(&log_pfad, &log, true)?;

    ableiten(daten)?;
    Ok(())
}

/// Bloecke eines Parsers in Fundstellen mit Hash umwandeln.
fn texte_aus(roh: &BTreeMap<String, Vec<Block>>) -> Textdatei {
    let mut aus: Textdatei = BTreeMap::new();
    for (id, bloecke) in roh {
        aus.insert(id.clone(), fundstelle(bloecke.clone()));
    }
    aus
}

// ------------------------------------------------------------------ Ableitungen

/// Alles, was sich aus dem Bestand ergibt: Suchindizes und die belegten Verweise.
fn ableiten(daten: &Path) -> Result<()> {
    let n = alle_indizes(daten)?;
    let v = verweise_bauen(daten)?;
    println!("  {n} Indexdateien, {v} belegte Verweise.");
    Ok(())
}

/// Sammelt die Verweise aus allen aktuellen Fassungen.
fn verweise_bauen(daten: &Path) -> Result<usize> {
    let katalog: serde_json::Value = lies_json(&daten.join("regelwerke.json"))?;
    let mut strukturen: BTreeMap<String, Struktur> = BTreeMap::new();
    let mut texte: BTreeMap<String, Textdatei> = BTreeMap::new();

    let rw_ordner = daten.join("rw");
    if !rw_ordner.is_dir() {
        return Ok(0);
    }
    let mut ordner: Vec<PathBuf> = std::fs::read_dir(&rw_ordner)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    ordner.sort();
    for p in ordner {
        let reg_pfad = p.join("fassungen.json");
        if !reg_pfad.exists() {
            continue;
        }
        let register: Fassungsregister = lies_json(&reg_pfad)?;
        let Some(aktuell) = register.fassungen.last() else { continue };
        let ord = p.join(&aktuell.id);
        let struktur: Struktur = lies_json(&ord.join("struktur.json"))?;
        let leitsprache = struktur.sprachen.first().cloned().unwrap_or_else(|| "de".into());
        let t: Textdatei = lies_json(&ord.join(format!("text-{leitsprache}.json")))?;
        texte.insert(struktur.regelwerk.clone(), t);
        strukturen.insert(struktur.regelwerk.clone(), struktur);
    }

    let bestand = verweise::bestand(&katalog, &strukturen);
    let muster = verweise::Muster::neu();
    let mut alle = Vec::new();
    for (rw, struktur) in &strukturen {
        let Some(t) = texte.get(rw) else { continue };
        alle.extend(verweise::finde(rw, struktur, t, &bestand, &muster));
    }

    // Zaehlung je Regelwerkspaar - die Mapping-Matrix liest sie direkt.
    let mut paare: BTreeMap<String, usize> = BTreeMap::new();
    for v in &alle {
        *paare.entry(format!("{}|{}", v.von.rw, v.nach.rw)).or_insert(0) += 1;
    }

    schreib_json(
        &daten.join("verweise.json"),
        &serde_json::json!({
            "gebaut": jetzt(),
            "hinweis": "Automatisch aus dem Wortlaut der Texte gewonnen; jede Beziehung traegt ihren Beleg. \
                        Fachliche Beziehungen stehen redaktionell in beziehungen.json.",
            "paare": paare,
            "verweise": alle,
        }),
        false,
    )?;
    Ok(alle.len())
}

// ---------------------------------------------------------------------- Index

/// Baut die Suchindizes aller Regelwerke, die eine aktuelle Fassung haben, und
/// schreibt dabei `bestand.json` - das Verzeichnis, an dem die App ablesen kann,
/// welche Regelwerke Volltext haben. Ohne diese Datei muesste sie fuer jedes
/// Regelwerk auf Verdacht eine Fassungsdatei anfragen (lauter 404er).
fn alle_indizes(daten: &Path) -> Result<usize> {
    let rw_ordner = daten.join("rw");
    let mut liste = Vec::new();
    let mut bestand = serde_json::Map::new();
    let mut gebaut = 0;
    if !rw_ordner.is_dir() {
        return Ok(0);
    }
    let mut ordner: Vec<PathBuf> = std::fs::read_dir(&rw_ordner)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    ordner.sort();
    for p in ordner {
        let reg_pfad = p.join("fassungen.json");
        if !reg_pfad.exists() {
            continue;
        }
        let register: Fassungsregister = lies_json(&reg_pfad)?;
        let Some(aktuell) = register.fassungen.last() else {
            continue;
        };
        let ord = p.join(&aktuell.id);
        let struktur: Struktur = lies_json(&ord.join("struktur.json"))?;
        bestand.insert(
            struktur.regelwerk.clone(),
            serde_json::json!({
                "fassung": aktuell,
                "sprachen": struktur.sprachen,
                "fassungen": register.fassungen.len(),
            }),
        );
        for sprache in &struktur.sprachen {
            let tp = ord.join(format!("text-{sprache}.json"));
            if !tp.exists() {
                continue;
            }
            let text: Textdatei = lies_json(&tp)?;
            let index = suche::baue(&struktur, &text, sprache);
            let name = format!("index-{sprache}.json");
            schreib_json(&ord.join(&name), &index, false)?;
            liste.push(serde_json::json!({
                "regelwerk": struktur.regelwerk,
                "sprache": sprache,
                "fassung": aktuell.id,
                "datei": format!("rw/{}/{}/{}", struktur.regelwerk, aktuell.id, name),
                "dokumente": index.dokumente.len(),
            }));
            gebaut += 1;
        }
    }
    schreib_json(
        &daten.join("suchindex.json"),
        &serde_json::json!({ "gebaut": jetzt(), "indizes": liste }),
        true,
    )?;
    schreib_json(
        &daten.join("bestand.json"),
        &serde_json::json!({ "gebaut": jetzt(), "bestand": bestand }),
        true,
    )?;
    Ok(gebaut)
}

// --------------------------------------------------------------------- Pruefen

fn pruefen(daten: &Path) -> Result<()> {
    let mut fehler = Vec::new();
    let regelwerke: serde_json::Value = lies_json(&daten.join("regelwerke.json"))?;
    let eintraege = regelwerke["regelwerke"]
        .as_array()
        .context("regelwerke.json: Feld 'regelwerke' fehlt oder ist keine Liste")?;
    println!("{} Regelwerke im Katalog.", eintraege.len());

    let mut ids = BTreeSet::new();
    for r in eintraege {
        let id = r["id"].as_str().unwrap_or("");
        if id.is_empty() {
            fehler.push("Katalogeintrag ohne id".to_string());
            continue;
        }
        if !ids.insert(id.to_string()) {
            fehler.push(format!("doppelte id: {id}"));
        }
        for feld in ["kurzname", "langtitel", "typ", "herausgeber", "verbindlichkeit", "status", "modus", "tiefe"] {
            if r[feld].is_null() {
                fehler.push(format!("{id}: Feld '{feld}' fehlt"));
            }
        }
        // Volltext-Regelwerke muessen eine Fassung haben.
        if r["modus"].as_str() == Some("original") {
            let reg = daten.join("rw").join(id).join("fassungen.json");
            if !reg.exists() {
                println!("  Hinweis: {id} ist auf Originaltext gestellt, hat aber noch keine Fassung.");
            }
        }
    }

    let quellen: Quellenliste = lies_json(&daten.join("quellen.json"))?;
    for k in &quellen.konnektoren {
        if !ids.contains(&k.regelwerk) {
            fehler.push(format!("quellen.json: Konnektor zeigt auf unbekanntes Regelwerk '{}'", k.regelwerk));
        }
        if k.nutzung.is_none() {
            fehler.push(format!("quellen.json: '{}' ohne dokumentierte Nutzungsbedingung", k.regelwerk));
        }
    }
    println!("{} Konnektoren.", quellen.konnektoren.len());

    // Strukturen gegen Texte pruefen.
    let rw_ordner = daten.join("rw");
    if rw_ordner.is_dir() {
        for e in std::fs::read_dir(&rw_ordner)? {
            let p = e?.path();
            if !p.is_dir() {
                continue;
            }
            let reg_pfad = p.join("fassungen.json");
            if !reg_pfad.exists() {
                continue;
            }
            let register: Fassungsregister = lies_json(&reg_pfad)?;
            for f in &register.fassungen {
                let ord = p.join(&f.id);
                let struktur: Struktur = lies_json(&ord.join("struktur.json"))?;
                let mut mit_text = Vec::new();
                sammle_textknoten(&struktur.knoten, &mut mit_text);
                for sprache in &struktur.sprachen {
                    let tp = ord.join(format!("text-{sprache}.json"));
                    if !tp.exists() {
                        fehler.push(format!("{}/{}: text-{sprache}.json fehlt", register.regelwerk, f.id));
                        continue;
                    }
                    let text: Textdatei = lies_json(&tp)?;
                    for id in &mit_text {
                        if !text.contains_key(id) {
                            fehler.push(format!(
                                "{}/{} ({sprache}): Knoten {id} ist als Text markiert, fehlt aber in der Textdatei",
                                register.regelwerk, f.id
                            ));
                        }
                    }
                }
                println!(
                    "  {} Fassung {}: {} Fundstellen, {} Sprachen.",
                    register.regelwerk,
                    f.id,
                    f.fundstellen,
                    struktur.sprachen.len()
                );
            }
        }
    }

    // Redaktionelle Zuordnungen gegen den Bestand pruefen: ein Pfad, den es nicht gibt,
    // faellt sonst erst in der App auf - und dort nur als leere Stelle.
    let mut pfade: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    if rw_ordner.is_dir() {
        for e in std::fs::read_dir(&rw_ordner)? {
            let p = e?.path();
            let reg_pfad = p.join("fassungen.json");
            if !p.is_dir() || !reg_pfad.exists() {
                continue;
            }
            let register: Fassungsregister = lies_json(&reg_pfad)?;
            let Some(aktuell) = register.fassungen.last() else { continue };
            let struktur: Struktur = lies_json(&p.join(&aktuell.id).join("struktur.json"))?;
            let mut menge = BTreeSet::new();
            sammle_alle_pfade(&struktur.knoten, &mut menge);
            pfade.insert(struktur.regelwerk.clone(), menge);
        }
    }

    let themen: serde_json::Value = lies_json(&daten.join("themen.json"))?;
    let themen_ids: BTreeSet<String> = themen["themen"]
        .as_array()
        .map(|a| a.iter().filter_map(|t| t["id"].as_str().map(String::from)).collect())
        .unwrap_or_default();

    let mut pruefe_stelle = |quelle: &str, rw: &str, pfad: Option<&str>, fehler: &mut Vec<String>| {
        if !ids.contains(rw) {
            fehler.push(format!("{quelle}: unbekanntes Regelwerk '{rw}'"));
            return;
        }
        let Some(p) = pfad else { return }; // Bezug auf das ganze Regelwerk ist zulaessig
        match pfade.get(rw) {
            Some(menge) if menge.contains(p) => {}
            Some(_) => fehler.push(format!("{quelle}: {rw} hat keine Fundstelle '{p}'")),
            None => fehler.push(format!("{quelle}: {rw} hat noch keinen Volltext, Pfad '{p}' nicht pruefbar")),
        }
    };

    let mut zaehler = (0, 0, 0, 0);
    if daten.join("themenzuordnung.json").exists() {
        let z: serde_json::Value = lies_json(&daten.join("themenzuordnung.json"))?;
        for e in z["zuordnungen"].as_array().unwrap_or(&vec![]) {
            zaehler.0 += 1;
            let rw = e["rw"].as_str().unwrap_or("");
            pruefe_stelle("themenzuordnung.json", rw, e["pfad"].as_str(), &mut fehler);
            for t in e["themen"].as_array().unwrap_or(&vec![]) {
                let t = t.as_str().unwrap_or("");
                if !themen_ids.contains(t) {
                    fehler.push(format!("themenzuordnung.json: unbekanntes Thema '{t}'"));
                }
            }
        }
    }
    if daten.join("beziehungen.json").exists() {
        let b: serde_json::Value = lies_json(&daten.join("beziehungen.json"))?;
        let typen: BTreeSet<String> = b["typen"]
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        for e in b["beziehungen"].as_array().unwrap_or(&vec![]) {
            zaehler.1 += 1;
            for seite in ["von", "nach"] {
                pruefe_stelle(
                    "beziehungen.json",
                    e[seite]["rw"].as_str().unwrap_or(""),
                    e[seite]["pfad"].as_str(),
                    &mut fehler,
                );
            }
            let typ = e["typ"].as_str().unwrap_or("");
            if !typen.contains(typ) {
                fehler.push(format!("beziehungen.json: unbekannter Typ '{typ}'"));
            }
            if e["begruendung"].as_str().unwrap_or("").len() < 40 {
                fehler.push(format!(
                    "beziehungen.json: '{}' ohne tragfaehige Begruendung",
                    e["id"].as_str().unwrap_or("?")
                ));
            }
        }
    }
    if daten.join("meldepflichten.json").exists() {
        let m: serde_json::Value = lies_json(&daten.join("meldepflichten.json"))?;
        for zeile in m["zeilen"].as_array().unwrap_or(&vec![]) {
            for (_, zelle) in zeile["zellen"].as_object().into_iter().flatten() {
                zaehler.2 += 1;
                pruefe_stelle(
                    "meldepflichten.json",
                    zelle["rw"].as_str().unwrap_or(""),
                    zelle["pfad"].as_str(),
                    &mut fehler,
                );
            }
        }
    }
    if daten.join("rollen.json").exists() {
        let r: serde_json::Value = lies_json(&daten.join("rollen.json"))?;
        let rollen: BTreeSet<String> = r["rollen"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x["id"].as_str().map(String::from)).collect())
            .unwrap_or_default();
        for a in r["anforderungen"].as_array().unwrap_or(&vec![]) {
            zaehler.3 += 1;
            pruefe_stelle("rollen.json", a["rw"].as_str().unwrap_or(""), a["pfad"].as_str(), &mut fehler);
            for (rolle, _) in a["raci"].as_object().into_iter().flatten() {
                if !rollen.contains(rolle) {
                    fehler.push(format!("rollen.json: unbekannte Rolle '{rolle}'"));
                }
            }
        }
    }
    println!(
        "{} Themenzuordnungen, {} fachliche Beziehungen, {} Matrixfelder, {} Rollenanforderungen geprueft.",
        zaehler.0, zaehler.1, zaehler.2, zaehler.3
    );

    if fehler.is_empty() {
        println!("Pruefung ohne Befund.");
        Ok(())
    } else {
        for f in &fehler {
            eprintln!("  ! {f}");
        }
        bail!("{} Befunde", fehler.len())
    }
}

fn sammle_alle_pfade(kn: &[Knoten], aus: &mut BTreeSet<String>) {
    for k in kn {
        if let Some(p) = &k.pfad {
            aus.insert(p.clone());
        }
        sammle_alle_pfade(&k.kinder, aus);
    }
}

fn sammle_textknoten(kn: &[Knoten], aus: &mut Vec<String>) {
    for k in kn {
        if k.text {
            aus.push(k.id.clone());
        }
        sammle_textknoten(&k.kinder, aus);
    }
}

// --------------------------------------------------------------------- Helfer

fn fundstelle(b: Vec<Block>) -> Fundstelle {
    let text = b.iter().map(|x| x.nur_text()).collect::<Vec<_>>().join("\n");
    Fundstelle { h: hash(&text), b }
}

fn hash(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())[..32].to_string()
}

fn gesamthash(t: &Textdatei) -> String {
    let mut h = Sha256::new();
    for (id, f) in t {
        h.update(id.as_bytes());
        h.update(b"\x1f");
        h.update(f.h.as_bytes());
        h.update(b"\n");
    }
    hex::encode(h.finalize())
}

fn jetzt() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// "Tue, 15 Oct 2024 11:53:52 GMT" -> "2024-10-15"
fn datum_aus(stand: &str) -> String {
    if let Ok(d) = DateTime::parse_from_rfc2822(stand) {
        return d.format("%Y-%m-%d").to_string();
    }
    if let Ok(d) = DateTime::parse_from_rfc3339(stand) {
        return d.format("%Y-%m-%d").to_string();
    }
    Utc::now().format("%Y-%m-%d").to_string()
}

fn kurz(sprache: &str) -> String {
    match sprache {
        "deu" | "de" => "de".into(),
        "eng" | "en" => "en".into(),
        s => s[..2.min(s.len())].to_string(),
    }
}

/// Titel einer weiteren Sprachfassung einsammeln (der Parser legt sie unter "de" ab).
fn titelkarte(kn: &[Knoten]) -> BTreeMap<String, String> {
    let mut aus = BTreeMap::new();
    fn rein(kn: &[Knoten], aus: &mut BTreeMap<String, String>) {
        for k in kn {
            if let Some(t) = k.titel.get("de") {
                aus.insert(k.id.clone(), t.clone());
            }
            rein(&k.kinder, aus);
        }
    }
    rein(kn, &mut aus);
    aus
}

fn titel_uebernehmen(kn: &mut [Knoten], karte: &BTreeMap<String, String>, sprache: &str) {
    for k in kn {
        if let Some(t) = karte.get(&k.id) {
            k.titel.insert(sprache.to_string(), t.clone());
        }
        titel_uebernehmen(&mut k.kinder, karte, sprache);
    }
}

fn lies_json<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T> {
    let roh = std::fs::read_to_string(p).with_context(|| format!("{} lesen", p.display()))?;
    serde_json::from_str(&roh).with_context(|| format!("{} auswerten", p.display()))
}

fn schreib_json<T: serde::Serialize>(p: &Path, wert: &T, schoen: bool) -> Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut s = if schoen {
        serde_json::to_string_pretty(wert)?
    } else {
        serde_json::to_string(wert)?
    };
    s.push('\n');
    std::fs::write(p, s).with_context(|| format!("{} schreiben", p.display()))?;
    Ok(())
}
