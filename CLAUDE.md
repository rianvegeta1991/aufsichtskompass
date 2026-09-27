# Aufsichtskompass

Web-App zu **IT-Compliance und IT-Governance für deutsche Versicherungsunternehmen** (Solvency II):
Regelwerks-Bibliothek mit Originaltext-Viewer, Interdependenzen, tägliches Screening und Lernbereich.
Schwerpunkt ist **DORA**. **Deutsch ist die Quellsprache** (Code, Kommentare, Commits, Oberfläche);
zweisprachig sind nur die *Inhalte* dort, wo die Quelle sie liefert (EU-Recht DE/EN).

Der vollständige Auftrag steht in `C:\Users\Bastian\Downloads\prompt_it-compliance-app.md`
(Abschnitte 1–12, Phasenplan mit Abnahmekriterien). Er ist die Messlatte für alles Weitere.

## Live

- **Seite:** https://rianvegeta1991.github.io/aufsichtskompass/
- **Repo:** https://github.com/rianvegeta1991/aufsichtskompass
- Deploy = `git push origin main` → Workflow `.github/workflows/pages.yml` stellt den Ordner
  1:1 über GitHub Actions online (nicht „Deploy from a branch"). Von Hand: `gh workflow run pages.yml`.
- Lokal: `preview_start` mit dem Namen `aufsichtskompass` → Port **8798** (`serve.ps1`).

## Zwei Teile, die man nicht verwechseln darf

| Teil | Was | Wo |
|---|---|---|
| **App** | Statische PWA, reines Vanilla-ES-Modul-JS. Liest nur JSON. Kein Build, kein Node. | `index.html`, `js/`, `sw.js` |
| **Werkzeug** | Rust-Programm `kompass`. Holt Quellen, erkennt Änderungen, baut Suchindizes, schreibt JSON. | `werkzeug/` |

Die App **rechnet nichts aus, was sie nicht anzeigen muss**. Alles Fachliche steht als Daten in
`daten/` – ein neues oder geändertes Regelwerk darf **nie** einen Code-Patch erfordern, nur einen
Eintrag in `daten/regelwerke.json` bzw. `daten/quellen.json` und einen Lauf des Werkzeugs.

## Befehle

```
cd werkzeug
cargo build --release
./target/release/kompass.exe abruf                 # alle aktiven Konnektoren
./target/release/kompass.exe abruf --rw dora       # nur ein Regelwerk
./target/release/kompass.exe abruf --erzwingen     # Dateien der aktuellen Fassung neu erzeugen
./target/release/kompass.exe index                 # nur Suchindizes + bestand.json
./target/release/kompass.exe pruefen               # Datenbestand auf Konsistenz prüfen
cargo test                                          # Parser- und Stammbildungs-Tests
```

**`--erzwingen` legt keine neue Fassung an**, sondern schreibt die vorhandene mit dem aktuellen
Parser neu. Das ist der Weg für Parserkorrekturen – eine Fassung, die es bei der Quelle nie gab,
darf nicht entstehen. Ohne den Schalter entscheidet der Vergleich aus ETag und SHA-256 über
Text und Fundstellen, ob eine neue Fassung übernommen wird.

## Daten (`daten/`)

| Datei | Inhalt |
|---|---|
| `regelwerke.json` | Redaktioneller Katalog: 52 Einträge mit Typ, Herausgeber, Verbindlichkeit, Status, Darstellungsmodus, Tiefe, Relevanz, Themen, `geprueft`-Flag |
| `quellen.json` | Konnektoren je Regelwerk samt **dokumentierter Nutzungsbedingung** und Kosten |
| `themen.json` | 22 Themen (IKS, IKT-Risikomanagement, Change Management …) |
| `fussnoten.json` | KAMaRisk-Hinweise **und** die Liste der bewusst nicht aufgenommenen Regelwerke (VAIT, BAIT) |
| `bestand.json` | Vom Werkzeug erzeugt: welches Regelwerk hat Volltext, in welcher Fassung und Sprache |
| `suchindex.json` | Verzeichnis der Indexdateien |
| `aenderungen.json` | Protokoll übernommener Fassungen mit neu/geändert/entfallen je Fundstelle |
| `rw/<id>/fassungen.json` | Versionsregister: Stand, Abrufzeit, ETag, Hash, gültig-bis |
| `rw/<id>/<fassung>/struktur.json` | Gliederungsbaum mit stabilen Kennungen und Deep-Link-Pfaden |
| `rw/<id>/<fassung>/text-<de\|en>.json` | Text je Fundstelle mit SHA-256 |
| `rw/<id>/<fassung>/index-<de\|en>.json` | Vorgebauter BM25-Index |

Fassungsordner sind **unveränderlich**. Eine neue Fassung kommt daneben, die alte bleibt (Archiv).

## Deep-Links

Adressen laufen über den Hash, weil GitHub Pages keine Pfade auf `index.html` umschreiben kann:

```
#/rw/dora/art/28/abs/4     Artikel 28 Absatz 4, Absatz wird hervorgehoben
#/rw/dora/eg/47            Erwägungsgrund 47
#/rw/dora/kap/V            Kapitel V mit allen Artikeln
#/suche/Informationsregister
#/themen/iks  ·  #/bibliothek  ·  #/aenderungen  ·  #/quellen  ·  #/lesezeichen
```

`404.html` rechnet zusätzlich Pfadadressen (`…/dora/art/28`) in diese Hash-Form um.

## Konventionen

- **Farben:** Karmin `#C6093B` als Primärfarbe, Petrolgrau als Sekundärfarbe. Statusfarben sind
  **nicht grün/rot**, sondern **Petrol gegen Dunkelrot** – sie unterscheiden sich in Helligkeit
  *und* Farbton und bleiben bei Rot-Grün-Schwäche unterscheidbar. Jede Statusfarbe trägt zusätzlich
  eine Beschriftung oder ein Zeichen; **Farbe nie als einziges Merkmal**.
- **Kontrast:** WCAG 2.2 AA in beiden Modi. Geprüft wird nicht nach Gefühl, sondern über die
  gerenderten Elemente (siehe „Testen").
- **Version:** `APP_VERSION` in `js/app.js`. Bei jedem veröffentlichten Update die minor-Zahl um 1
  erhöhen – als **ganze Zahl** weiterzählen (nach 1.9 kommt 1.10).
- **Service Worker:** bei jeder Veröffentlichung `CACHE` in `sw.js` hochzählen. Der Worker cacht
  **einzeln** (`cache.add` je Datei), nie `addAll` – sonst legt eine fehlende Datei den ganzen
  Offline-Betrieb lahm.
- **Icons:** `icon.svg`/`icon-maskable.svg` sind die Vorlagen, die PNGs entstehen in PowerShell mit
  `System.Drawing` (`icons.ps1`) – auf dem Rechner gibt es keinen SVG-Renderer. **Bei Logoänderungen
  beide Stellen nachziehen.**

## Fallstricke (aus Erfahrung)

- **EUR-Lex ist für Automaten gesperrt.** `eur-lex.europa.eu/legal-content/...` antwortet mit
  HTTP 202 und leerem Körper (`x-amzn-waf-action: challenge`). Der amtliche Dienst **CELLAR**
  (`publications.europa.eu/resource/celex/<CELEX>`, Content-Negotiation auf XHTML) liefert dieselben
  Fassungen und schickt `ETag` und `Last-Modified` mit. Nicht zurück auf die Weboberfläche wechseln.
- **Level-2-Rechtsakte nicht von Hand pflegen.** Die zwölf RTS/ITS zu DORA stammen aus einer
  SPARQL-Abfrage (`publications.europa.eu/webapi/rdf/sparql`, Rechtsgrundlage CELEX 32022R2554).
  Dieselbe Abfrage findet künftige Rechtsakte; sie gehört in Phase 5 in das Screening.
- **Absatzmarken sind sprachabhängig:** Deutsch „(1)", Englisch „1.". Beide Formen räumt
  `marke_lesen`/`marke_weg` weg; die Tests in `cellar.rs` halten das fest.
- **Der Suchstamm muss auf beiden Seiten gleich sein.** `stamm()` gibt es zweimal: in
  `werkzeug/src/suche.rs` (Index) und in `js/suche.js` (Abfrage). Ändert sich eine Regel, muss sie
  **an beiden Stellen** geändert werden – sonst findet die Suche indexierte Wörter nicht mehr.
  Gegenprobe: `cargo test` und in der Browser-Konsole `kompass.suche.proben()`.
- **Service Worker beim Entwickeln:** Er fängt auf `localhost` bewusst **nichts** ab, und das
  Gerüst läuft in Produktion als stale-while-revalidate. Vorher wurde stundenlang eine Fassung
  geprüft, die es im Ordner längst nicht mehr gab.
- **`serve.ps1` schickt `Cache-Control: no-store`** – aus demselben Grund.
- **BaFin hat seine Seitenstruktur umgebaut:** die bekannten Deep-Links (`/DE/PublikationenDaten/...`)
  antworten mit 404. Keine Links raten – die betroffenen Katalogeinträge tragen `geprueft: false`
  und einen Suchbegriff statt einer erfundenen URL.
- **Screenshots im Browser-Fenster sind leer, wenn das Fenster versteckt ist.** Dann über
  `get_page_text`/`read_page` oder `javascript_tool` prüfen, nicht am Bild verzweifeln.

## Testen

Verifizieren statt hoffen: `preview_start` (Port 8798), Konsole auf Fehler prüfen, Zustand per DOM
auslesen. Die Kontrastprüfung läuft über die gerenderten Elemente – Verhältnis aus
`getComputedStyle(color)` und der ersten deckenden Hintergrundfarbe gegen 4.5:1 (bzw. 3:1 für große
Schrift), in **beiden** Erscheinungsbildern. Für das Werkzeug: `cargo test` und
`kompass pruefen` (prüft Katalogfelder, Konnektorbezüge und ob jeder als „Text" markierte Knoten
auch wirklich Text hat).

## Stand und offene Punkte

Fertig: Phase 1 (Fundament) und Phase 2 für **EU-Recht** – 15 Regelwerke im Volltext
(DORA DE/EN, zwölf Level-2-Rechtsakte, NIS2, DSGVO), rund 1.900 adressierbare Fundstellen,
Viewer mit Gliederungsbaum, Deep-Links, Glossar aus den Begriffsbestimmungen, Zitat-Export,
Lesezeichen und Notizen, BM25-Suche.

Offen, in dieser Reihenfolge sinnvoll:

1. **Phase 2b** – Konnektor `gii` für gesetze-im-internet.de (VAG `vag_2016`, BSIG `bsig_2025`,
   BDSG `bdsg_2018`, HGB, AO `ao_1977`; XML-Fassung je Gesetz) und `bafin-pdf` für MaGo und MaRisk.
   Die Katalogeinträge und Konnektoren stehen schon, nur auf `aktiv: false`.
2. **Phase 3** – eigene Zusammenfassungen (ISO 27001 inkl. 93 Annex-A-Controls, COBIT 2019 mit
   40 Objectives, ITIL 4 mit 34 Practices, CSA CCM; grob C5, NIST CSF 2.0, GDV). Das ist vor allem
   Schreibarbeit und braucht mehrere Sitzungen. Pflicht je Eintrag: eigene Worte, Bezugsfassung,
   Herkunft, Prüfstatus und der sichtbare Hinweis „Eigene Zusammenfassung – ersetzt nicht das Original".
3. **Phase 4** – Relation-Modell und Matrizen.
4. **Phase 5** – Screening (Feeds, Seitenüberwachung, GDELT, Taxonomie, Digest) als Actions-Lauf
   um 06:30 Europe/Berlin; GitHub-Cron läuft in UTC, die Sommerzeit muss das Werkzeug prüfen.

## Rechtliches

- Originaltext nur dort, wo er zulässig ist: EU-Recht (EUR-Lex, Weiterverwendung mit Quellenangabe),
  deutsche Gesetze (§ 5 UrhG), NIST (gemeinfrei). Alles andere bekommt **eigene Zusammenfassungen**,
  keine enge Paraphrase, keine übernommenen Sätze.
- Jede Anzeige eines Originaltextes trägt Quelle, Fassung, Abrufdatum und den Hinweis
  „Rechtsverbindlich ist nur die amtlich veröffentlichte Fassung."
- Keine Firmennamen, keine Logos, keine fremden Marken in der App.
