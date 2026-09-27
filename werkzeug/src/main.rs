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
mod modell;
mod suche;

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
            let n = alle_indizes(&daten)?;
            println!("{n} Indexdateien gebaut.");
            Ok(())
        }
        "pruefen" => pruefen(&daten),
        _ => {
            println!(
                "kompass <befehl>\n\
                 \n\
                   abruf [--rw <id>] [--erzwingen]  Quellen abrufen, neue Fassungen uebernehmen\n\
                   index                            Suchindizes aus den vorhandenen Daten bauen\n\
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
            sonst => println!("[{}] Konnektortyp '{sonst}' noch nicht umgesetzt - uebersprungen", k.regelwerk),
        }
    }
    if behandelt == 0 {
        println!("Kein passender Konnektor in quellen.json gefunden.");
    }
    Ok(())
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

    let reg_pfad = daten.join("rw").join(rw).join("fassungen.json");
    let mut register: Fassungsregister = if reg_pfad.exists() {
        lies_json(&reg_pfad)?
    } else {
        Fassungsregister { regelwerk: rw.clone(), fassungen: Vec::new() }
    };
    let letzte = register.fassungen.last().cloned();
    let etag = if erzwingen {
        None
    } else {
        letzte.as_ref().and_then(|f| f.etag.clone())
    };

    println!("[{rw}] CELEX {celex}, Sprachen {sprachen:?} - Abruf laeuft ...");
    let Some(haupt) = cellar::hole(celex, &leit, etag.as_deref())? else {
        println!("[{rw}] unveraendert (ETag der Quelle stimmt) - nichts zu tun.");
        return Ok(());
    };

    let geparst = cellar::parse(&haupt.koerper)?;
    let mut texte: Textdatei = BTreeMap::new();
    for (id, bloecke) in &geparst.texte {
        texte.insert(id.clone(), fundstelle(bloecke.clone()));
    }
    if texte.len() < 10 {
        bail!("[{rw}] nur {} Fundstellen geparst - Quellstruktur pruefen, Daten bleiben unberuehrt", texte.len());
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
            register.fassungen[n].etag = haupt.etag.clone();
            register.fassungen[n].abgerufen = jetzt();
            schreib_json(&reg_pfad, &register, true)?;
            return Ok(());
        }
        if neuschreiben && l.hash != gesamt {
            println!(
                "[{rw}] Achtung: der Text der Fassung {} weicht vom gespeicherten Hash ab.                  Mit --erzwingen wird er an Ort und Stelle ersetzt, keine neue Fassung angelegt.",
                l.id
            );
        }
    }

    let stand = if neuschreiben {
        letzte.as_ref().unwrap().stand.clone()
    } else {
        haupt.stand.clone().unwrap_or_else(jetzt)
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

    let quelle = Quelle {
        name: "EUR-Lex / CELLAR (Amt für Veröffentlichungen der EU)".into(),
        url: format!("{}/{celex}", cellar::BASIS),
        celex: Some(celex.to_string()),
        hinweis: "Rechtsverbindlich ist nur die amtlich veröffentlichte Fassung.".into(),
    };

    // Weitere Sprachfassungen: gleiche Kennungen, nur andere Texte und Titel.
    let mut knoten = geparst.knoten;
    let mut sprachkuerzel = vec![kurz(&leit)];
    let mut weitere: Vec<(String, Textdatei)> = Vec::new();
    for s in sprachen.iter().skip(1) {
        match cellar::hole(celex, s, None)? {
            Some(a) => {
                let g = cellar::parse(&a.koerper)?;
                let ks = kurz(s);
                titel_uebernehmen(&mut knoten, &titelkarte(&g.knoten), &ks);
                let mut t: Textdatei = BTreeMap::new();
                for (id, b) in &g.texte {
                    t.insert(id.clone(), fundstelle(b.clone()));
                }
                println!("[{rw}] Sprachfassung {s}: {} Fundstellen.", t.len());
                weitere.push((ks.clone(), t));
                sprachkuerzel.push(ks);
            }
            None => println!("[{rw}] Sprachfassung {s} nicht geliefert - uebersprungen."),
        }
    }

    let struktur = Struktur {
        regelwerk: rw.clone(),
        fassung: fassung_id.clone(),
        abgerufen: jetzt(),
        quelle: quelle.clone(),
        sprachen: sprachkuerzel.clone(),
        knoten,
    };
    schreib_json(&ordner.join("struktur.json"), &struktur, true)?;
    schreib_json(&ordner.join(format!("text-{}.json", kurz(&leit))), &texte, true)?;
    for (s, t) in &weitere {
        schreib_json(&ordner.join(format!("text-{s}.json")), t, true)?;
    }

    if neuschreiben {
        // Nur die Dateien wurden erneuert - kein regulatorisches Ereignis.
        let n = register.fassungen.len() - 1;
        register.fassungen[n].etag = haupt.etag.clone();
        register.fassungen[n].abgerufen = jetzt();
        register.fassungen[n].hash = gesamt;
        register.fassungen[n].fundstellen = texte.len();
        schreib_json(&reg_pfad, &register, true)?;
        println!("[{rw}] Fassung {fassung_id} neu erzeugt: {} Fundstellen.", texte.len());
        let n = alle_indizes(daten)?;
        println!("[{rw}] {n} Indexdateien gebaut.");
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
        quelle: quelle.url.clone(),
    };
    if let Some(l) = &letzte {
        let alt_pfad = daten
            .join("rw")
            .join(rw)
            .join(&l.id)
            .join(format!("text-{}.json", kurz(&leit)));
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
        quelle,
        etag: haupt.etag.clone(),
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

    let n = alle_indizes(daten)?;
    println!("[{rw}] {n} Indexdateien gebaut.");
    Ok(())
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
