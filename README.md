# Aufsichtskompass

IT-Compliance und IT-Governance für deutsche Versicherungsunternehmen (Solvency II) – als
Wissensbasis, Analysewerkzeug, Frühwarnsystem und Lernplattform. Schwerpunkt: **DORA**.

**Seite:** https://rianvegeta1991.github.io/aufsichtskompass/

Private, nichtkommerzielle Anwendung. Keine Rechtsberatung. Rechtsverbindlich ist nur die
amtlich veröffentlichte Fassung.

## Was drin ist

- **Regelwerks-Bibliothek** mit 52 Katalogeinträgen – Gesetze, Verordnungen, Rundschreiben,
  Leitlinien, Normen und Frameworks, jeweils mit Typ, Herausgeber, Verbindlichkeit, Status,
  Darstellungsmodus, Tiefe und Prüfstatus.
- **Originaltext-Viewer** für 20 Regelwerke mit rund 3.750 adressierbaren Fundstellen:
  DORA (deutsch und englisch), die zwölf Level-2-Rechtsakte (RTS/ITS), NIS2, DSGVO sowie
  VAG, BSIG, BDSG vollständig und HGB und AO als IT-relevanter Auszug.
  Gliederungsbaum, Deep-Link auf jeden Absatz bzw. Paragrafen, Glossar aus den
  Begriffsbestimmungen, Zitat-Export mit Quellenangabe, Lesezeichen und Notizen.
- **Volltextsuche** über alle Fundstellen (BM25, deutsche Stammbildung), clientseitig.
- **Versionierung**: jede Fassung bleibt unveränderlich liegen, mit SHA-256 je Fundstelle als
  Grundlage der Änderungserkennung.
- Hell und dunkel, WCAG 2.2 AA, offlinefähig als PWA.
- **Auf dem Smartphone** ein eigenes Format: untere Navigationsleiste, Viewer mit den
  Reitern Text, Gliederung und Kontext, gestapelte Tabellen statt Querscrollen.

## Wie es gebaut ist

Zwei Teile, bewusst getrennt:

- **App** – statische Seite, Vanilla-ES-Module, kein Build und kein Node. Sie liest nur JSON
  aus `daten/` und speichert Eigenes (Lesezeichen, Notizen, Relevanz, Erscheinungsbild)
  ausschließlich im Browser.
- **Werkzeug** – Rust-Programm `kompass` unter `werkzeug/`. Es holt die Quellen, erkennt
  Änderungen, schreibt neue Fassungen und baut die Suchindizes.

```
cd werkzeug
cargo build --release
./target/release/kompass.exe abruf      # Quellen abrufen, Änderungen übernehmen
./target/release/kompass.exe pruefen    # Datenbestand prüfen
```

Alle fachlichen Inhalte liegen als Daten im Repository, nicht im Code: ein neues Regelwerk
braucht einen Eintrag in `daten/regelwerke.json` und `daten/quellen.json` – keinen Code-Patch.

## Quellen

Ausschließlich kostenlose, amtliche Quellen ohne Anmeldung:

| Quelle | Weg | Nutzung |
|---|---|---|
| EU-Recht | CELLAR des Amts für Veröffentlichungen (`publications.europa.eu/resource/celex/…`) | Weiterverwendung mit Quellenangabe (Beschluss 2011/833/EU) |
| Level-2-Rechtsakte | SPARQL-Dienst des Amts für Veröffentlichungen | wie oben |
| Deutsche Gesetze | gesetze-im-internet.de, XML-Fassung je Gesetz (ZIP) | amtliche Werke, § 5 UrhG |

Wo kein Volltext zulässig ist (ISO, COBIT, ITIL, CSA CCM …), stehen eigene Zusammenfassungen –
in eigenen Worten, mit Bezugsfassung, Herkunft, Prüfstatus und Verweis auf die Bezugsquelle.

## Stand

Phase 1 (Fundament) und Phase 2 sind fertig – EU-Recht über CELLAR, deutsche Gesetze über
gesetze-im-internet.de. Offen: BaFin-Veröffentlichungen (erst nach Klärung der
Nutzungsbedingungen), die eigenen Framework-Zusammenfassungen, Mappings und Matrizen,
das tägliche Screening und der Lernbereich. Der Ausbaustand steht auch in der App
auf der Startseite.
