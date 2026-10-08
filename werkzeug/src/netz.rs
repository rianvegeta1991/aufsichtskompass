//! Netzzugriff an der Leine (Phase 8, ASVS L2: Schutz vor SSRF).
//!
//! Alle Abrufe des Werkzeugs - Konnektoren, Feeds, Seitenueberwachung, SPARQL -
//! laufen ueber diesen Weg. Die Leine prueft **vor** dem Verbindungsaufbau:
//!
//! 1. nur `https`, kein anderer Port als 443,
//! 2. Host steht auf der Liste in `daten/quellen.json` (`erlaubte_hosts`),
//! 3. keine Adressliterale (damit niemand `https://127.0.0.1/` oder
//!    `https://[::1]/` unterschiebt) und keine Zugangsdaten in der Adresse,
//! 4. Umleitungen werden **erneut** geprueft - sonst fuehrte eine Quelle mit
//!    einem `Location`-Kopf aus dem erlaubten Bereich heraus.
//!
//! Die Liste ist bewusst Daten, nicht Code: eine neue Quelle braucht einen
//! Eintrag in `quellen.json`, keinen Patch hier. `kompass pruefen` haelt jede
//! konfigurierte Adresse gegen die Leine, damit ein fehlender Eintrag vor dem
//! naechsten Lauf auffaellt und nicht erst mitten darin.

use anyhow::{Result, bail};
use reqwest::Url;
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct Leine {
    hosts: Vec<String>,
}

impl Leine {
    pub fn neu<I: IntoIterator<Item = String>>(hosts: I) -> Leine {
        Leine {
            hosts: hosts
                .into_iter()
                .map(|h| h.trim().trim_end_matches('.').to_ascii_lowercase())
                .filter(|h| !h.is_empty())
                .collect(),
        }
    }

    /// Liest die Liste aus einer schon geladenen `quellen.json`.
    pub fn aus_quellen(quellen: &serde_json::Value) -> Result<Leine> {
        let Some(liste) = quellen.get("erlaubte_hosts").and_then(|v| v.as_array()) else {
            bail!("quellen.json: 'erlaubte_hosts' fehlt - ohne Liste wird nichts abgerufen");
        };
        let leine = Leine::neu(liste.iter().filter_map(|v| v.as_str().map(String::from)));
        if leine.hosts.is_empty() {
            bail!("quellen.json: 'erlaubte_hosts' ist leer");
        }
        Ok(leine)
    }

    pub fn hosts(&self) -> &[String] {
        &self.hosts
    }

    /// Prueft eine Adresse und gibt sie geparst zurueck. Der Fehler nennt den
    /// Grund, damit im Protokoll des Laufs steht, **warum** nichts abgerufen wurde.
    pub fn erlaubt(&self, url: &str) -> Result<Url> {
        let Ok(u) = Url::parse(url) else {
            bail!("'{url}' ist keine gueltige Adresse");
        };
        if u.scheme() != "https" {
            bail!("{url}: nur https ist erlaubt (hier '{}')", u.scheme());
        }
        if !u.username().is_empty() || u.password().is_some() {
            bail!("{url}: Zugangsdaten in der Adresse sind nicht erlaubt");
        }
        if let Some(port) = u.port()
            && port != 443 {
                bail!("{url}: Port {port} ist nicht erlaubt");
            }
        let Some(host) = u.host_str() else {
            bail!("{url}: ohne Host");
        };
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        // Adressliterale von vornherein abweisen: ein Name muss her, und der muss
        // auf der Liste stehen. Damit sind 127.0.0.1, 169.254.169.254, [::1] und
        // alle anderen internen Adressen ausgeschlossen, ohne DNS befragen zu muessen.
        if host.starts_with('[') || IpAddr::from_str(&host).is_ok() {
            bail!("{url}: Adressliterale sind nicht erlaubt, nur Namen von der Liste");
        }
        if !self.host_erlaubt(&host) {
            bail!("{url}: Host '{host}' steht nicht in quellen.json unter 'erlaubte_hosts'");
        }
        Ok(u)
    }

    /// Exakte Treffer und echte Unterdomaenen (`a.b.de` zu `b.de`), aber nicht
    /// `boesartig-b.de` - deshalb wird auf den Punkt geprueft.
    fn host_erlaubt(&self, host: &str) -> bool {
        self.hosts.iter().any(|h| host == h || host.ends_with(&format!(".{h}")))
    }

    /// Klient ohne automatische Umleitung. Umleitungen folgt `senden` selbst -
    /// so laeuft **jede** Station durch die Leine, und `http` laesst sich auf
    /// `https` anheben (siehe dort).
    pub fn klient(&self, kennung: &str, sekunden: u64) -> Result<reqwest::blocking::Client> {
        Ok(reqwest::blocking::Client::builder()
            .user_agent(kennung.to_string())
            .timeout(Duration::from_secs(sekunden))
            .redirect(reqwest::redirect::Policy::none())
            .build()?)
    }

    /// Schickt eine GET-Anfrage und folgt Umleitungen - jede Station wird
    /// gegen die Leine geprueft, hoechstens fuenf.
    ///
    /// Warum nicht die automatische Umleitung von reqwest: CELLAR leitet auf
    /// `http://publications.europa.eu/resource/cellar/...` um, also auf **dieselbe**
    /// erlaubte Adresse, nur unverschluesselt. Das einfach zu verbieten legte den
    /// Abruf aller EU-Rechtstexte lahm (so geschehen in v1.8). Stattdessen wird `http`
    /// auf `https` angehoben, bevor die Leine pruefen darf: der Abruf bleibt
    /// verschluesselt und der Host muss trotzdem auf der Liste stehen. Strenger als
    /// vorher, nicht lockerer.
    pub fn senden(
        &self,
        klient: &reqwest::blocking::Client,
        url: &str,
        kopfzeilen: &[(&str, &str)],
    ) -> Result<reqwest::blocking::Response> {
        let mut aktuell = self.erlaubt(url)?;
        for _ in 0..=5 {
            let mut anfrage = klient.get(aktuell.clone());
            for (name, wert) in kopfzeilen {
                anfrage = anfrage.header(*name, *wert);
            }
            let antwort = anfrage
                .send()
                .map_err(|f| anyhow::anyhow!("Abruf {aktuell}: {f}"))?;
            let status = antwort.status();
            // 304 ist kein Umleitungsfall, auch wenn reqwest ihn zu den 3xx zaehlt.
            if !status.is_redirection() || status == reqwest::StatusCode::NOT_MODIFIED {
                return Ok(antwort);
            }
            let Some(ziel) = antwort
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
            else {
                bail!("{aktuell}: Umleitung ({status}) ohne Location-Kopf");
            };
            let naechste = naechste_adresse(&aktuell, ziel)?;
            // Die Leine entscheidet ueber die angehobene Adresse.
            aktuell = self.erlaubt(naechste.as_str())?;
        }
        bail!("{url}: zu viele Umleitungen")
    }
}

/// Zieladresse einer Umleitung: relative Angaben werden aufgeloest, `http` wird
/// auf `https` angehoben. Reine Rechenarbeit, ohne Netz - deshalb testbar.
pub fn naechste_adresse(aktuell: &Url, location: &str) -> Result<Url> {
    let mut u = aktuell
        .join(location)
        .map_err(|_| anyhow::anyhow!("{aktuell}: Umleitungsziel '{location}' ist keine Adresse"))?;
    if u.scheme() == "http" {
        u.set_scheme("https")
            .map_err(|_| anyhow::anyhow!("{u}: Schema laesst sich nicht auf https anheben"))?;
    }
    Ok(u)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leine() -> Leine {
        Leine::neu(["publications.europa.eu".to_string(), "bsi.bund.de".to_string()])
    }

    #[test]
    fn nur_erlaubte_hosts() {
        let l = leine();
        assert!(l.erlaubt("https://publications.europa.eu/resource/celex/32022R2554").is_ok());
        // echte Unterdomaene: erlaubt
        assert!(l.erlaubt("https://www.bsi.bund.de/feed").is_ok());
        // fremder Host: abgewiesen
        assert!(l.erlaubt("https://beispiel.test/feed").is_err());
        // Praefix-Trick: 'nicht-bsi.bund.de.boese.test' darf nicht durchkommen
        assert!(l.erlaubt("https://bsi.bund.de.boese.test/feed").is_err());
        assert!(l.erlaubt("https://xbsi.bund.de/feed").is_err());
    }

    #[test]
    fn interne_adressen_und_schemata_abgewiesen() {
        let l = Leine::neu(["127.0.0.1".to_string(), "bsi.bund.de".to_string()]);
        // selbst wenn jemand ein Adressliteral auf die Liste setzt: bleibt verboten
        assert!(l.erlaubt("https://127.0.0.1/geheim").is_err());
        assert!(l.erlaubt("https://[::1]/geheim").is_err());
        assert!(l.erlaubt("https://169.254.169.254/latest/meta-data/").is_err());
        assert!(l.erlaubt("http://www.bsi.bund.de/feed").is_err(), "http ist nicht erlaubt");
        assert!(l.erlaubt("file:///C:/Windows/win.ini").is_err());
        assert!(l.erlaubt("https://nutzer:geheim@www.bsi.bund.de/").is_err());
        assert!(l.erlaubt("https://www.bsi.bund.de:8443/").is_err());
    }

    #[test]
    fn liste_kommt_aus_den_daten() {
        let gut = serde_json::json!({ "erlaubte_hosts": ["beispiel.test"] });
        let l = Leine::aus_quellen(&gut).unwrap();
        assert_eq!(l.hosts(), ["beispiel.test"]);
        assert!(l.erlaubt("https://beispiel.test/x").is_ok());

        // Ohne Liste wird nicht abgerufen - lieber Abbruch als stiller Freifahrtschein.
        assert!(Leine::aus_quellen(&serde_json::json!({})).is_err());
        assert!(Leine::aus_quellen(&serde_json::json!({ "erlaubte_hosts": [] })).is_err());
    }

    #[test]
    fn punkt_am_ende_und_grossschreibung_zaehlen_nicht() {
        let l = leine();
        assert!(l.erlaubt("https://WWW.BSI.BUND.DE./feed").is_ok());
    }

    #[test]
    fn umleitung_wird_aufgeloest_und_auf_https_angehoben() {
        let von = Url::parse("https://publications.europa.eu/resource/celex/32022R2554").unwrap();
        // genau der Fall von CELLAR: dieselbe Adresse, aber unverschluesselt
        let a = naechste_adresse(
            &von,
            "http://publications.europa.eu/resource/cellar/abc.0004.03/DOC_1",
        )
        .unwrap();
        assert_eq!(a.as_str(), "https://publications.europa.eu/resource/cellar/abc.0004.03/DOC_1");
        // relative Angaben werden gegen die aktuelle Adresse aufgeloest
        let b = naechste_adresse(&von, "/resource/cellar/xyz").unwrap();
        assert_eq!(b.as_str(), "https://publications.europa.eu/resource/cellar/xyz");
        assert!(naechste_adresse(&von, "http://[kaputt").is_err());
    }

    #[test]
    fn angehobene_adresse_muss_trotzdem_auf_die_liste() {
        let l = leine();
        let von = Url::parse("https://publications.europa.eu/resource/celex/1").unwrap();
        // Auf die Liste: nach dem Anheben erlaubt.
        let ok = naechste_adresse(&von, "http://publications.europa.eu/x").unwrap();
        assert!(l.erlaubt(ok.as_str()).is_ok());
        // Fremder Host: auch angehoben abgewiesen - Verschluesselung ersetzt keine Erlaubnis.
        let fremd = naechste_adresse(&von, "http://beispiel.test/x").unwrap();
        assert!(l.erlaubt(fremd.as_str()).is_err());
        // Adressliteral und interne Adresse: ebenfalls.
        let intern = naechste_adresse(&von, "http://169.254.169.254/latest/meta-data/").unwrap();
        assert!(l.erlaubt(intern.as_str()).is_err());
    }
}
