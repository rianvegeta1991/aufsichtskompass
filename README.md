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

Die Phasen 1, 2, 2b, 4, 5 und 7 sind fertig – EU-Recht über CELLAR, deutsche Gesetze über
gesetze-im-internet.de, Themenseiten, Beziehungen, Matrizen, das tägliche Screening und der
Lernbereich. Offen: BaFin-Veröffentlichungen (erst nach Klärung der Nutzungsbedingungen),
die eigenen Framework-Zusammenfassungen zu ISO, COBIT, ITIL und CSA CCM sowie die
Härtung nach ASVS. Der Ausbaustand steht auch in der App auf der Startseite.
