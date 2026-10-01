# Betrieb

Was im Alltag zu tun ist, was von allein läuft, und wie man etwas zurückholt.
Stand: 01.10.2026 (v1.8). Sicherheitsfragen stehen in `SICHERHEIT.md`.

## Was von allein läuft

| Wann | Was | Wo |
|---|---|---|
| täglich 06:30 Europe/Berlin | Screening: 14 Feeds, 2 überwachte Seiten, neue Level-2-Rechtsakte über den SPARQL-Dienst; committet die Daten und stößt die Veröffentlichung an | `.github/workflows/screening.yml` |
| bei jedem Push auf `main` | Veröffentlichung der Seite (ca. 20 Sekunden) | `.github/workflows/pages.yml` |
| bei jedem Push und jedem Pull Request | Lints, 56 Tests, `kompass pruefen` über den ganzen Datenbestand | `.github/workflows/pruefung.yml` |

Der Cron von GitHub läuft in UTC; der Auftrag feuert deshalb zweimal (04:30 und
05:30 UTC) und das Werkzeug entscheidet mit `--nur-um 06:30` selbst, welcher Lauf
arbeitet. So bleibt es sommers wie winters bei einem Lauf pro Tag.

**Wichtig:** Ein Commit mit dem Standard-Token löst keine weiteren Aufträge aus
(GitHub verhindert so Endlosschleifen). Darum stößt `screening.yml` die
Veröffentlichung selbst an (`gh workflow run pages.yml`). Ohne diesen Schritt
lägen die Daten im Repository, wären aber nicht online.

## Von Hand arbeiten

```bash
cd werkzeug
cargo build --release
./target/release/kompass pruefen              # Datenbestand prüfen (tut nichts weh)
./target/release/kompass abruf                # alle Quellen abrufen
./target/release/kompass abruf --rw vag       # nur ein Regelwerk
./target/release/kompass abruf --rw vag --erzwingen   # vorhandene Fassung neu erzeugen
./target/release/kompass zusammenfassungen    # eigene Zusammenfassungen übernehmen
./target/release/kompass index                # Suchindizes und Verweise neu bauen
./target/release/kompass screening            # Screening sofort, ohne Zeitfenster
./target/release/kompass lernstand            # Lerninhalte gegen den Bestand prüfen
./target/release/kompass lernstand --bestaetigen
```

Alle Befehle nehmen `--daten <pfad>`; ohne Angabe wird `daten` bzw. `../daten`
gesucht. Lokal ansehen lässt sich die Seite mit `serve.ps1` (Port 8798).

## Nach einer Änderung

1. `cargo clippy --all-targets -- -D warnings` und `cargo test` im Ordner `werkzeug`.
2. `kompass pruefen` – muss „Pruefung ohne Befund“ melden.
3. Bei Änderungen an Dateien, die der Service Worker vorhält: `CACHE` in `sw.js`
   hochzählen (`aufsichtskompass-vN`), sonst bekommen Wiederkehrer die alte Fassung.
4. `APP_VERSION` in `js/app.js` erhöhen.
5. Push nach `main`; der Deploy läuft an. Prüfen lässt sich das Ergebnis am
   schnellsten so:

```bash
curl -s https://rianvegeta1991.github.io/aufsichtskompass/js/app.js | grep APP_VERSION
```

Der Browser hält Dateien von GitHub Pages bis zu zehn Minuten (`max-age=600`).
Wer gleich nachsehen will, lädt mit `fetch(url, {cache:'reload'})` oder hart neu.

## Datenhaltung und Sicherung

Es gibt **keine Datenbank**. Alles liegt als JSON im Repository, und damit ist das
Repository die Sicherung – mit Verlauf, nicht nur mit Stand:

- `daten/rw/<id>/<fassung>/` – jede übernommene Fassung, **unveränderlich**.
  Nichts darin wird je überschrieben; eine neue Fassung bekommt einen neuen Ordner.
- `daten/rw/<id>/fassungen.json` – Versionsregister mit Hash, ETag, Abrufzeit.
- `daten/aenderungen.json` – Protokoll, welche Fundstellen neu, geändert oder
  entfallen sind.
- `daten/news/<jahr>-<monat>.json`, `daten/screening-laeufe.json` – Meldungen und
  Laufprotokolle.
- `daten/screening-stand.json` – ETags und Hashes der Quellen. Darf verlorengehen:
  der nächste Lauf holt dann einmal alles neu.

**Zweite Kopie:** Wer sich nicht auf GitHub allein verlassen will, legt einen
Spiegel an (`git clone --mirror` und regelmäßig `git remote update`) – damit liegt
der gesamte Verlauf auch außerhalb.

### Etwas zurückholen

| Fall | Weg |
|---|---|
| Eine Fassung versehentlich überschrieben | `git checkout <commit> -- daten/rw/<id>/<fassung>` – die alten Dateien stehen im Verlauf. |
| Ein Screening-Lauf hat Unsinn eingesammelt | Commit des Laufs zurücknehmen (`git revert <commit>`), danach `daten/screening-stand.json` prüfen: steht dort schon der neue ETag, holt der nächste Lauf die Quelle nicht erneut – dann den betroffenen Eintrag dort entfernen. |
| Index oder Verweise kaputt | `kompass index` baut beide neu; sie sind abgeleitet und jederzeit wiederherstellbar. |
| Parser korrigiert, Bestand soll nachziehen | `kompass abruf --rw <id> --erzwingen` – schreibt die **vorhandene** Fassung neu und erfindet keine, die es bei der Quelle nie gab. |
| Seite online veraltet, Repository aktuell | `gh workflow run pages.yml`. Erst prüfen: `git rev-list --left-right --count main...origin/main` (0/0 = alles gepusht), dann die Version der Pages-URL gegen `raw.githubusercontent.com` vergleichen. |

### Was der Nutzer auf seinem Gerät hat

Lesezeichen, Notizen, Relevanz-Einstufungen, Lernfortschritt und das
Erscheinungsbild liegen im `localStorage` – sie sind **nicht** Teil der Sicherung
und nicht wiederherstellbar, wenn jemand seinen Browserspeicher leert. Unter
„Lesezeichen & Notizen“ gibt es deshalb einen Export als JSON (und einen
Löschknopf, siehe `SICHERHEIT.md`).

## Wenn etwas klemmt

- **Eine Quelle liefert nichts mehr:** Das Protokoll des Laufs nennt Status und
  Grund je Quelle (in der App unter „Screening“). Bei „Host steht nicht in
  quellen.json“ fehlt der Eintrag unter `erlaubte_hosts` – das ist Absicht, siehe
  `SICHERHEIT.md`.
- **Parser bricht ab („nur N Fundstellen geparst“):** Die Quelle hat ihren Aufbau
  geändert. Der Bestand bleibt unberührt – erst den Parser nachziehen, dann
  abrufen. Nichts erzwingen, solange die Zahl nicht stimmt.
- **Deploy bleibt hängen:** `gh run list`, dann `gh api repos/OWNER/REPO/pages/builds`
  (Dauer 0 = nie gestartet). Die Statusseite von GitHub meldet solche
  Warteschlangen-Hänger nicht.
- **Suche findet nichts Neues:** Die Indizes sind abgeleitet; `kompass index` neu
  bauen und prüfen, ob `daten/suchindex.json` die Datei des Regelwerks führt.
