# Aufsichtskompass

IT-Compliance und IT-Governance für deutsche Versicherungsunternehmen (Solvency II) – als
Wissensbasis, Analysewerkzeug, Frühwarnsystem und Lernplattform. Schwerpunkt: **DORA**.

**Seite:** https://rianvegeta1991.github.io/aufsichtskompass/

Private, nichtkommerzielle Anwendung. Keine Rechtsberatung. Rechtsverbindlich ist nur die
amtlich veröffentlichte Fassung.

## Was drin ist

- **Regelwerks-Bibliothek** mit 53 Katalogeinträgen – Gesetze, Verordnungen, Rundschreiben,
  Leitlinien, Normen und Frameworks, jeweils mit Typ, Herausgeber, Verbindlichkeit, Status,
  Darstellungsmodus, Tiefe und Prüfstatus.
- **Originaltext-Viewer** für 28 Regelwerke: DORA (deutsch und englisch), die zwölf
  Level-2-Rechtsakte (RTS/ITS), NIS2, DSGVO, Solvency II (Richtlinie und Delegierte
  Verordnung), AI Act, Cyber Resilience Act, Data Act, dazu VAG, BSIG, BDSG vollständig
  und HGB und AO als IT-relevanter Auszug – und von der BaFin die **MaGo für SII-VU**
  (Rundschreiben 09/2025 (VA), adressierbar je Randziffer: `rz/31`) sowie die
  **FAQs zu DORA** (44 Fragen) und **NIST SP 800-53 Rev. 5** (1.196 Controls, englisch).
  Dazu 24 Werke als eigene Zusammenfassung –
  zusammen **52 Regelwerke mit 9.460 Fundstellen**.
  Gliederungsbaum, Deep-Link auf jeden Absatz bzw. Paragrafen, Glossar aus den
  Begriffsbestimmungen, Zitat-Export mit Quellenangabe, Lesezeichen und Notizen.
- **Volltextsuche** über alle Fundstellen (BM25, deutsche Stammbildung), clientseitig.
- **Interdependenzen**: Mapping-Matrix, Heatmap Thema × Regelwerk, Meldepflichten-Matrix
  (DORA / BSIG / DSGVO), Rollen- und Three-Lines-Sicht, Netzwerkgraph und Zeitstrahl –
  jeweils mit CSV-Export. Dazu rund 2.700 Verweise, die das Werkzeug aus dem Wortlaut der
  Texte selbst gewonnen hat, jeder mit seiner Belegstelle.
- **Themenseiten** zu 22 Themen mit Zielbild, Fundstellen über alle Regelwerke hinweg,
  Prüfungsschwerpunkten und typischen Nachweisen.
- **Tägliches Screening** um 06:30 Uhr: 14 geprüfte Feeds von BaFin, BSI, CERT-Bund,
  EBA, ESMA, Kommission, GDV und anderen, Seitenüberwachung für EIOPA und ENISA,
  neue DORA-Rechtsakte über den SPARQL-Dienst. Jede Meldung nennt die Begriffe, die
  ihre Relevanz begründen; dazu Newsfeed, Tages-Digest und ein Protokoll jedes Laufs.
- **Lernbereich**: drei Lektionen mit Lernpfad, drei Quizzes mit sechs Fragearten,
  eine Fallstudie mit Entscheidungsbaum, 24 Karteikarten mit verteilter Wiederholung,
  Cheat Sheets und eine druckbare Teilnahmebestätigung. Jede Erläuterung verlinkt die
  Fundstelle; ändert sich dort der Text, meldet das Werkzeug die Stelle zur Überprüfung.
- **Versionierung**: jede Fassung bleibt unveränderlich liegen, mit SHA-256 je Fundstelle als
  Grundlage der Änderungserkennung.
- Hell und dunkel, WCAG 2.2 AA, offlinefähig als PWA.
- **Gehärtet**: Content-Security-Policy ohne Drittinhalte, Allowlist für alle Abrufe
  des Werkzeugs (Schutz vor SSRF), Export und Löschung der eigenen Daten, 75 Tests
  (Kernlogik zu 90 % abgedeckt). Einzelheiten in [SICHERHEIT.md](SICHERHEIT.md)
  und [BETRIEB.md](BETRIEB.md).
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
| BaFin-Veröffentlichungen | bafin.de, HTML-Fassung je Veröffentlichung | Nutzungsbedingungen der BaFin: Wiedergabe unverändert und mit Quellenangabe („© Bundesanstalt für Finanzdienstleistungsaufsicht / www.bafin.de") |
| NIST SP 800-53 | OSCAL-Katalog, den NIST im Repository `usnistgov/oscal-content` pflegt | Werk der US-Bundesregierung, in den USA gemeinfrei; Quellenangabe |

Wo kein Volltext zulässig ist, stehen eigene Zusammenfassungen – in eigenen Worten, mit
Bezugsfassung, Herkunft, Prüfstatus und Verweis auf die Bezugsquelle. Enthalten sind
**ISO/IEC 27001:2022** (Klauseln 4–10 und alle 93 Annex-A-Controls), **ISO/IEC 27002**
(93 Controls), 27005, 27017, 27018 und 22301, **COBIT 2019** (40 Objectives), **ITIL 4**
(34 Practices), **BSI C5**, die **GDV-Verhaltensregeln**, **CSA CCM v4**, **NIST CSF 2.0**,
**COSO**, **IDW PS 330/860/951**, **ISAE 3402**, die **GoBD**, **MaRisk** (Rundschreiben
06/2026), die beiden **BaFin-Mitteilungen** zu DORA, die **EIOPA-Leitlinien** zu
Governance und Cloud sowie **TIBER-DE**. Sie laufen durch dieselbe Maschinerie
wie die Originaltexte und sind deshalb genauso durchsuchbar, verlinkbar und in den
Matrizen verankert. Jede Fundstelle trägt den Hinweis, dass hier nicht der Originaltext
steht; die Kennungen der kostenpflichtigen Werke sind noch gegen die Norm abzugleichen.

## Stand

**Alle Phasen des Auftrags sind abgearbeitet** – Phase 1 (Fundament), 2 (EU-Recht über
CELLAR), 2b (deutsche Gesetze), 2c (BaFin), 3 (eigene Zusammenfassungen), 4 (Themenseiten,
Beziehungen, Matrizen), 5 (tägliches Screening), 6 (Versionierung und Änderungserkennung,
Diff-Ansicht offen), 7 (Lernbereich) und 8 (Härtung nach ASVS Level 2).

Offen bleiben: der fachliche Abgleich der Zusammenfassungen – ISO, COBIT, ITIL, COSO, IDW, ISAE
und GoBD stammen aus dem Gedächtnis und stehen überall als „ungeprüft" da; MaRisk und die
BaFin-Mitteilungen sind nach dem Original zusammengefasst, aber nur als PDF verfügbar
(dafür fehlt ein PDF-Textextraktor). Die MaGo für kleine Versicherungsunternehmen ließ sich
nicht auffinden. Solvency II liegt in der Ursprungsfassung vor, nicht konsolidiert.
Außerdem offen: ein regelmäßiger `cargo audit`, die Diff-Ansicht sowie XLSX- und PDF-Export. Der Ausbaustand steht auch
in der App auf der Startseite.
