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

## Konnektoren

| Typ | Quelle | Stand |
|---|---|---|
| `cellar` | EU-Recht über `publications.europa.eu/resource/celex/<CELEX>` | umgesetzt |
| `gii` | deutsche Gesetze über `gesetze-im-internet.de/<kennung>/xml.zip` | umgesetzt |
| `bafin-pdf` | BaFin-Rundschreiben als PDF | offen, erst nach Klärung der Nutzungsbedingungen |
| `rss` / `seite` | Feeds und Seitenüberwachung fürs Screening | offen (Phase 5) |

Beide umgesetzten Konnektoren holen nur ab und parsen; die Versionslogik liegt gemeinsam
in `uebernehmen()` in `main.rs` – Hash-Vergleich, neue Fassung, Änderungsereignis,
Indexbau. Ein dritter Konnektor braucht deshalb nur Abruf und Parser.

### gesetze-im-internet.de (`gii`)

- Je Gesetz eine ZIP mit einer XML-Datei; der Server schickt `ETag` und `Last-Modified`.
- `<norm>` ist alles: das Gesetz, jede Gliederungseinheit und jeder Paragraf.
  Die `gliederungskennzahl` ist ein Schlüssel aus **Dreiergruppen**, ihre Länge gibt die
  Ebene an (3 = Teil, 6 = Kapitel, 9 = Abschnitt). Daraus baut der Parser den Baum.
- **Deutsche Gesetze nummerieren mit Wortformen**: „Erstes Buch", „Zweiter Abschnitt" –
  das Typwort kann vorn oder hinten stehen. Deshalb trägt jeder Knoten das Feld `bez`
  mit der Beschriftung der Quelle; Anzeige und Suchindex bevorzugen sie gegenüber
  „Art + Nummer" (sonst stünde dort „Abschnitt Zweiter").
- Pfade der Gliederungsknoten laufen über die Kennzahl (`gl/030010010`), weil „Erstes"
  weder eine brauchbare URL noch eine Nummer ist. Paragrafen: `par/23`, `par/23/abs/1a`.
- **Absatzmarken hier**: `(1)`, aber auch `(1a)`, `(12b)` – anders als im EU-Recht.
- `paragraphen` im Konnektor beschränkt auf einzelne Vorschriften (HGB, AO). Bei einem
  solchen Auszug entfernt `aufraeumen()` die leeren Gliederungsäste – sonst stünden beim
  HGB 92 Kapitel ohne Inhalt im Baum – und Anlagen bleiben außen vor.

## Interdependenzen (Phase 4)

**Zwei Sorten Beziehungen, streng getrennt** – das ist der Kern:

| Datei | Herkunft | Inhalt |
|---|---|---|
| `daten/beziehungen.json` | redaktionell, von Hand | Typ, **Begründung**, Quelle der Zuordnung, Konfidenz, Prüfstatus |
| `daten/verweise.json` | `kompass verweise` aus dem Wortlaut | rund 2.700 belegte Verweise, jeder mit seiner Textstelle |

Was sich aus dem Text ergibt, wird **belegt**, nicht bewertet: Der Extraktor erkennt
„Artikel 6 Absatz 1", „§ 23 Absatz 1" und „Artikel 6 der Verordnung (EU) 2022/2554",
ordnet den Rechtsakt über die CELEX-Nummern des Katalogs zu und übernimmt einen Verweis
**nur, wenn das Ziel im Bestand existiert**. Verweise ins Leere wären schlimmer als keine.
Fachliche Beziehungen (entspricht, konkretisiert, lex specialis, Spannungsfeld) kann kein
Muster finden – die stehen redaktionell daneben, jede mit Begründung.

Weitere redaktionelle Dateien: `themenzuordnung.json` (Fundstelle → Thema),
`meldepflichten.json` (DORA / BSIG / DSGVO nebeneinander, jede Angabe mit Fundstelle),
`rollen.json` (RACI und drei Verteidigungslinien).

**`kompass pruefen` prüft diese Dateien gegen den Bestand**: Jeder Pfad muss existieren,
jedes Thema und jeder Beziehungstyp bekannt sein, jede Beziehung eine Begründung tragen.
Ein Tippfehler in `par/29` fällt damit beim Lauf auf und nicht erst als leere Stelle in der App.

Darstellung: `#/matrizen` mit sechs Ansichten (Mapping-Matrix, Heatmap, Meldepflichten,
Rollen, Graph, Zeitstrahl), jeweils mit CSV-Export. **In jeder Zelle steht eine Zahl oder
ein Zeichen** – Farbe ist immer nur Zugabe. Die Heatmap stuft deshalb die Helligkeit eines
Farbtons statt einen Ampelverlauf zu nutzen, und die Matrixzellen tragen die Typzeichen
(`=`, `⊂`, `∩`, `>`, `≠`, `§`) neben der Zahl.

## Screening (Phase 5)

`kompass screening [--nur-um 06:30] [--ausloeser <text>]` – täglich über
`.github/workflows/screening.yml`, jederzeit auch von Hand.

- **14 Feeds**, alle am 30.09.2026 einzeln geprüft: BaFin (Rundschreiben, Aufsicht,
  Presse, Maßnahmen), BSI, CERT-Bund, CERT-EU, EBA, ESMA, Europäische Kommission,
  GDV, Cloud Security Alliance.
- **Seitenüberwachung** für EIOPA und ENISA: Die beiden bieten keinen auffindbaren
  Feed. Der Bereich `#main-content` bzw. `main` wird geholt, gehasht und verglichen.
  Liefert der Selektor zu wenig Text, wird **gewarnt statt überschrieben** – sonst
  meldete eine geänderte Seitenstruktur täglich eine „Änderung".
- **Neue Level-2-Rechtsakte** über den SPARQL-Dienst: Was auf DORA beruht und nicht
  im Katalog steht, wird als Vorschlag „In Bibliothek aufnehmen" gemeldet.
- **Bewertung** über `daten/taxonomie.json`: Begriffe mit Gewicht, Synonymen, Themen
  und Regelwerken; Treffer in der Überschrift zählen doppelt; Ausschlussbegriffe
  ziehen ab. Jede Meldung trägt die Begriffe mit, die gegriffen haben.
- **Zusammenfassung** extraktiv ohne Sprachmodell: Sätze nach Begriffsüberdeckung
  gewichtet, die besten zwei in Originalreihenfolge.

Drei Fallen, die beim Bauen aufgefallen sind und in den Tests festgehalten sind:

1. **Wortgrenzen**: „ITS" traf mitten in „bereits". Gesucht wird jetzt mit Wortgrenze
   am Anfang – Komposita wie „IKT-Risikomanagementrahmen" greifen weiter.
2. **Akronyme groß**: Das englische „its" traf die Abkürzung ITS. Kurze
   Großbuchstaben-Kürzel werden in der Schreibweise der Quelle gesucht.
3. **Eigene Schwelle je Quelle** (`mindestpunkte`): CERT-Bund liefert 250 Hinweise
   zu beliebiger Software. Ohne eigene Schwelle landeten alle im Feed; mit 22 nur
   noch das, was zusätzlich zum Aufsichtsthema passt.

Ablage: `daten/news/<jahr>-<monat>.json`, Verzeichnis `news/index.json`, Protokoll
`screening-laeufe.json`, Abrufgedächtnis `screening-stand.json`. Von fremden
Beiträgen werden nur Titel, Adresse, Datum und ein kurzer Auszug gespeichert.

**Ein Commit aus Actions löst keinen Deploy aus.** GitHub startet für Commits mit dem
Standard-Token bewusst keine weiteren Workflows (Schleifenschutz). Der Screening-Lauf
stößt `pages.yml` deshalb am Ende selbst an (`gh workflow run pages.yml`, Berechtigung
`actions: write`). Ohne diesen Schritt lägen die täglichen Daten im Repo, aber nicht
auf der Seite – genau so war es beim ersten Lauf.

**Zeitsteuerung:** GitHub-Cron läuft in UTC. Der Auftrag feuert deshalb um 04:30 und
05:30 UTC; `--nur-um 06:30` prüft die Berliner Zeit und lässt nur den passenden Lauf
arbeiten. So bleibt es sommers wie winters bei einem Lauf pro Tag.

## Lernbereich (Phase 7)

Inhalte als Daten unter `daten/lernen/`: `lektionen.json` (drei Lektionen samt Lernpfad),
`quizzes.json` (drei Quizzes zu je zehn Fragen), `fallstudien.json` (Entscheidungsbaum mit
Musterlösung), `karten.json` (24 Karteikarten), `cheatsheets.json` (drei Kurzfassungen).
Eine neue Lektion oder Frage braucht **keine** Code-Änderung.

**Sechs Fragearten**, alle in `eingabefeld()` in `js/lernen.js`: `mc`, `mehrfach`,
`wahrfalsch`, `luecke`, `zuordnung`, `reihenfolge`. Die Reihenfolge-Frage wird über
Hoch-/Runter-Knöpfe sortiert, nicht per Ziehen – das bleibt mit Tastatur und auf dem
Handy bedienbar. Jede Frage nennt nach der Antwort ihre Erläuterung **und** die
Fundstelle, an der sich die Antwort überprüfen lässt.

**Verteilte Wiederholung** der Karteikarten: Abstände 1, 3, 7, 16 und 35 Tage aus der
Datendatei. Wer die Karte weiß, rückt eine Stufe vor; wer sie nicht weiß, fängt vorn an.

**`kompass lernstand`** prüft alle 140 Fundstellenbezüge der Lerninhalte gegen den
Bestand und merkt sich deren Hash. Ändert sich der Text einer Fundstelle, steht die
betroffene Lektion, Frage oder Karte beim nächsten Lauf als **„zu prüfen"** in
`daten/lernen/pruefstand.json` – damit erfüllt die App die Forderung, betroffene Fragen
bei Änderungen der Vorgaben automatisch zu markieren. Nach der Durchsicht:
`kompass lernstand --bestaetigen`. Ein Bezug „ohne Fundstelle" heißt: Tippfehler im Pfad.

**Teilnahmebestätigung**: eigene Druckansicht (`.urkunde`, `@media print`), gespeichert
wird sie über den Druckdialog als PDF. Sie ist bewusst als das ausgewiesen, was sie ist –
kein Zertifikat und kein Nachweis gegenüber Dritten.

**Fallstrick beim Schreiben der Inhalte:** Deutsche Anführungszeichen mit geradem
Schlusszeichen (`„so"`) beenden den JSON-String und machen die Datei ungültig. Richtig
ist `„so“`. Beide Dateien waren davon betroffen; `python -c "import json,io; json.load(...)"`
findet es sofort.

## Eigene Zusammenfassungen (Phase 3)

Für Werke ohne zulässigen Volltext (ISO, COBIT, ITIL, CSA CCM …) liegen eigene
Zusammenfassungen unter `daten/zusammenfassungen/<id>.json`. Je Eintrag: Kennung,
Titel, Zweck, Kernanforderungen, typische Nachweise und Bezüge.

**Der Kniff:** `kompass zusammenfassungen` formt sie in **dieselbe Form** wie die
Originaltexte um – Gliederungsbaum plus Text je Fundstelle – und übergibt sie an
dieselbe Versionslogik. Dadurch funktionieren Viewer, Deep-Links, Suche,
Themenzuordnung, Beziehungen und Matrizen ohne einen einzigen Sonderweg. Was sich
unterscheidet, steht im Katalog (`modus: zusammenfassung`) und im Quellenhinweis der
Fassung; der Viewer zeigt daraufhin den Pflichthinweis
„Eigene Zusammenfassung – ersetzt nicht das Original" samt Bezugsfassung, Herkunft
und Prüfstatus (`zusammenfassungshinweis()` in `js/viewer.js`).

Enthalten sind **sieben Werke** (Stand v1.7, zusammen 226 Fundstellen):

| Werk | Umfang | Pfade |
|---|---|---|
| ISO/IEC 27001:2022 | Klauseln 4–10 und alle 93 Annex-A-Controls (100) | `kl/6`, `a/5.19` |
| COBIT 2019 | alle 40 Objectives (EDM 5, APO 14, BAI 11, DSS 6, MEA 4) | `o/apo12` |
| ITIL 4 | alle 34 Practices (14 allgemeine, 17 Service, 3 technische) | `p/sm05` |
| BSI C5 | 17 Kriterienbereiche | `kb/sso` |
| GDV-Verhaltensregeln | 12 Regelungsfelder | `k/05` |
| CSA CCM v4 | 17 Domänen | `d/sta` |
| NIST CSF 2.0 | 6 Funktionen | `f/gv` |

**Wichtig zur Sorgfalt:** Der Normtext darf nicht wiedergegeben werden – die Texte sind
durchweg selbst formuliert. Kennungen und Titel stammen aus dem Gedächtnis und sind
**noch nicht gegen die Norm abgeglichen**; genau dafür steht `geprueft: false` in der
Datei und „ungeprüft" im Hinweis der App. Wo ein Werk seine Teile selbst **nicht**
nummeriert (ITIL, GDV-Verhaltensregeln), ist die eigene Zählung in der Datei **und** im
Katalog-Hinweis als solche gekennzeichnet – sonst sähe sie wie eine amtliche Kennung aus.

## Deep-Links

Adressen laufen über den Hash, weil GitHub Pages keine Pfade auf `index.html` umschreiben kann:

```
#/rw/dora/art/28/abs/4     Artikel 28 Absatz 4, Absatz wird hervorgehoben
#/rw/dora/eg/47            Erwägungsgrund 47
#/rw/dora/kap/V            Kapitel V mit allen Artikeln
#/rw/vag/par/23/abs/1a     § 23 Absatz 1a VAG
#/rw/hgb/gl/030010010      Gliederungseinheit eines deutschen Gesetzes
#/suche/Informationsregister
#/themen/iks  ·  #/bibliothek  ·  #/aenderungen  ·  #/quellen  ·  #/lesezeichen
```

`404.html` rechnet zusätzlich Pfadadressen (`…/dora/art/28`) in diese Hash-Form um.

## Mobiles Format (seit v1.1)

Drei Schwellen, alle gemessen und nicht geraten:

| Breite | Verhalten |
|---|---|
| ab 1181 px | Seitenmenü links, Viewer dreispaltig (Baum · Text · Kontext) |
| 901–1180 px | Viewer zweispaltig, Kontext-Panel darunter |
| bis 900 px | Seitenmenü wird zur Schublade, **untere Navigationsleiste** (`#tabbar`) mit Start, Bibliothek, Suche, Themen und „Mehr"; Kopf ohne Suchfeld (dafür ein Lupen-Knopf) und ohne Wort am Erscheinungsbild-Knopf; Filter der Bibliothek in einem zugeklappten Block |
| bis 760 px | Viewer bekommt **Reiter** Text · Gliederung · Kontext (`.viewer[data-ansicht]`) |
| bis 700 px | breite Tabellen werden zu gestapelten Karten (`table.stapel`, Spaltenname aus `data-spalte`), Karten einspaltig |

Warum die Reiter sein müssen: vorher lag der Gliederungsbaum mit 193 Einträgen **vor** dem
Text – auf 375 px begann der Artikel erst bei 924 px Scrolltiefe, das Kontext-Panel bei 8.325 px.
Mit Reitern beginnt der Text bei 341 px.

- Tabellen immer über `tabelle(spalten, zeilen)` aus `ui.js` bauen, nie von Hand: nur dann
  tragen die Zellen ihren Spaltennamen und stapeln sich auf dem Handy sauber.
- Die untere Leiste, die Schublade und der Kopf rechnen mit `env(safe-area-inset-*)`
  (Notch und Gestenbalken) und mit `--kopf-h` / `--tabbar-h`. Wer den Kopf ändert, ändert
  `--kopf-h` mit – daran hängen Baum, Kontext, Reiterleiste und die klebenden Tabellenköpfe.
- Tap-Ziele in Navigation, Reitern, Baum und Werkzeugzeile sind auf mindestens 44 px gesetzt.

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
- **Screenshots im Browser-Fenster sind leer oder laufen in einen Timeout, wenn das Fenster
  versteckt ist.** Dann über `get_page_text`/`read_page` oder `javascript_tool` prüfen, nicht
  am Bild verzweifeln.
- **CSS-Übergänge laufen in einem versteckten Fenster nicht.** `getComputedStyle` liefert dann
  dauerhaft den Startwert – die Schublade sah dadurch aus, als ginge sie nicht auf, obwohl die
  Regeln stimmten. Gegenprobe: `document.visibilityState` prüfen oder `transition: none` setzen.
- **Keine Backslashes durch Bash-Heredocs schicken.** Der Bash-Aufruf entschärft sie eine
  Ebene zu viel: aus `\r\n` im Patch-Skript wurde ein echter Zeilenumbruch mitten im
  JavaScript-String – die Datei war syntaktisch kaputt, und der Browser meldete nur
  „Invalid regular expression". Patch-Skripte mit Escapes gehören als Datei ins
  Scratchpad und werden von dort ausgeführt.
- **Beim Patchen per Skript erst lesen, dann schreiben.** `open(p,'w')` in derselben Zeile wie
  `open(p).read()` leert die Datei, bevor gelesen wird – so gingen `app.js` und `sw.js` einmal
  komplett verloren (aus dem letzten Commit wiederhergestellt). Die Hilfsskripte im
  Scratchpad machen es richtig: Inhalt lesen, ersetzen, prüfen, dann schreiben.

## Testen

Verifizieren statt hoffen: `preview_start` (Port 8798), Konsole auf Fehler prüfen, Zustand per DOM
auslesen. Die Kontrastprüfung läuft über die gerenderten Elemente – Verhältnis aus
`getComputedStyle(color)` und der ersten deckenden Hintergrundfarbe gegen 4.5:1 (bzw. 3:1 für große
Schrift), in **beiden** Erscheinungsbildern. Für das Werkzeug: `cargo test` und
`kompass pruefen` (prüft Katalogfelder, Konnektorbezüge und ob jeder als „Text" markierte Knoten
auch wirklich Text hat).

## Stand und offene Punkte

Fertig: Phase 1 (Fundament), Phase 2 (EU-Recht), Phase 2b (deutsche Gesetze),
Phase 3 (eigene Zusammenfassungen, sieben Werke), Phase 4 (Themenseiten, Beziehungen,
Matrizen), Phase 5 (Screening), Phase 7 (Lernbereich). Zusammen **27 Regelwerke mit
3.978 adressierbaren Fundstellen**, davon
**20 Regelwerke im Volltext mit rund 3.750 adressierbaren Fundstellen**: DORA (DE/EN),
die zwölf Level-2-Rechtsakte, NIS2, DSGVO, dazu VAG, BSIG und BDSG vollständig sowie HGB
und AO als IT-relevanter Auszug. Viewer mit Gliederungsbaum, Deep-Links, Glossar aus den
Begriffsbestimmungen, Zitat-Export, Lesezeichen und Notizen, BM25-Suche über beide
Rechtskreise. Seit v1.1 das mobile Format (siehe oben): geprüft auf 320, 375, 768 und
1440 px – kein Querscrollen, keine Tap-Ziele unter 40 px, WCAG-AA-Kontraste in beiden
Erscheinungsbildern.

Dazu seit v1.3: 22 Themenseiten mit Zielbild, Fundstellentabelle, Prüfungsschwerpunkten
und Nachweisen; 32 fachliche Beziehungen mit Begründung; rund 2.700 belegte Verweise;
sechs Matrix-Ansichten mit CSV-Export; Beziehungen im Kontext-Panel des Viewers.

Offen, in dieser Reihenfolge sinnvoll:

1. **Phase 2c** – BaFin-Veröffentlichungen (MaGo 09/2025 (VA), MaRisk, Aufsichtsmitteilung,
   DORA-FAQ). Zwei Hürden, beide fachlich: die Nutzungsbedingungen für den Volltext sind je
   Veröffentlichung zu prüfen und zu dokumentieren, und die BaFin hat ihre Seitenstruktur
   umgebaut – die Direktlinks müssen neu ermittelt werden. Bis dahin bleiben diese Einträge
   auf „Zusammenfassung" und `geprueft: false`.
2. **Kennungen abgleichen** – die Kennungen und Titel der kostenpflichtigen Werke (ISO,
   COBIT, ITIL) sind gegen die jeweilige Bezugsfassung zu prüfen; erst dann `geprueft: true`
   in `daten/zusammenfassungen/<id>.json` setzen und neu übernehmen. Ebenso die Fassung der
   GDV-Verhaltensregeln und der Direktlink zum C5-Katalog.
3. **Phase 4 vertiefen** – die Beziehungen zu ISO 27001, COBIT, ITIL und C5 hängen seit
   v1.6/v1.7 auf Control-, Objective- bzw. Bereichsebene (64 Beziehungen). Noch am ganzen
   Regelwerk hängen die Beziehungen zu MaGo und MaRisk – die brauchen Phase 2c.
   Ebenfalls offen: XLSX- und PDF-Export (zurzeit CSV und Druckansicht).
4. **Phase 8** – Härtung: Security-Review, ASVS-Checkliste, Testabdeckung, Betriebsdoku.

Hinweis zum Umfang: Die Suchindizes sind zusammen rund 2,6 MB (0,6 MB gzip) und werden bei
der ersten Suche vollständig geladen. `verweise.json` ist rund 800 KB groß und wird deshalb
**erst auf Anforderung** geholt – in der Matrix über einen Knopf, im Viewer über „Belegte
Verweise laden". Wächst der Bestand deutlich weiter, sollten beide nach Relevanz gestaffelt
nachgeladen werden.

## Rechtliches

- Originaltext nur dort, wo er zulässig ist: EU-Recht (EUR-Lex, Weiterverwendung mit Quellenangabe),
  deutsche Gesetze (§ 5 UrhG), NIST (gemeinfrei). Alles andere bekommt **eigene Zusammenfassungen**,
  keine enge Paraphrase, keine übernommenen Sätze.
- Jede Anzeige eines Originaltextes trägt Quelle, Fassung, Abrufdatum und den Hinweis
  „Rechtsverbindlich ist nur die amtlich veröffentlichte Fassung."
- Keine Firmennamen, keine Logos, keine fremden Marken in der App.
