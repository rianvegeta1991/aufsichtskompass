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

    /// Klient, der auch bei Umleitungen an der Leine bleibt.
    pub fn klient(&self, kennung: &str, sekunden: u64) -> Result<reqwest::blocking::Client> {
        let leine = self.clone();
        let regel = reqwest::redirect::Policy::custom(move |versuch| {
            if versuch.previous().len() >= 5 {
                return versuch.error("zu viele Umleitungen");
            }
            match leine.erlaubt(versuch.url().as_str()) {
                Ok(_) => versuch.follow(),
                Err(f) => versuch.error(f.to_string()),
            }
        });
        Ok(reqwest::blocking::Client::builder()
            .user_agent(kennung.to_string())
            .timeout(Duration::from_secs(sekunden))
            .redirect(regel)
            .build()?)
    }
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
}
