# Sicherheit

Stand: 01.10.2026 (v1.8). Maßstab ist **OWASP ASVS 5.0, Level 2**, wie im Auftrag
vorgegeben. Diese Datei hält fest, was geprüft wurde, was erfüllt ist, was nicht
zutrifft und was offen bleibt. Sie ist Teil der Betriebsdokumentation – wer etwas
ändert, zieht sie nach.

## Was es überhaupt zu schützen gibt

Die Anwendung besteht aus zwei Teilen, und das bestimmt die Angriffsfläche:

| Teil | Läuft wo | Verarbeitet |
|---|---|---|
| **App** | im Browser des Nutzers, ausgeliefert als statische Dateien über GitHub Pages | nur JSON aus dem eigenen Ursprung; Eigenes (Lesezeichen, Notizen, Lernfortschritt) ausschließlich im `localStorage` |
| **Werkzeug** `kompass` | auf dem Rechner der Redaktion und im GitHub-Actions-Lauf | ruft öffentliche Quellen ab, schreibt JSON ins Repository |

Es gibt **keinen Server, keine Datenbank, keine Anmeldung, keine Konten, keine
Sitzungen, keine Cookies, keine Zählpixel** und keine Drittinhalte. Vieles aus
ASVS trifft deshalb nicht zu – das ist keine Lücke, sondern eine Folge der
Bauweise. Was bleibt, ist in zwei Punkten echt: **die Inhalte fremder Quellen**
(Feeds, Seiten) und **die Abrufe des Werkzeugs**.

## ASVS-L2-Checkliste

| Kapitel | Lage | Begründung / Umsetzung |
|---|---|---|
| V1 Architektur, Design | **erfüllt** | Zwei Teile, bewusst getrennt; Datenflüsse in `CLAUDE.md` und hier beschrieben; keine Geheimnisse im Repository (geprüft). |
| V2 Authentifizierung | **nicht zutreffend** | Keine Konten. Die Seite ist öffentlich und nur lesend. |
| V3 Sitzungsverwaltung | **nicht zutreffend** | Keine Sitzungen, keine Cookies. |
| V4 Zugriffskontrolle | **nicht zutreffend / erfüllt** | Nichts zu autorisieren. Schreibend ist nur der Weg über Git: Schreibrechte am Repository, Pull-Request-Historie als Protokoll. |
| V5 Validierung, Ausgabe-Kodierung | **erfüllt** | Die Oberfläche wird aus DOM-Knoten gebaut (`el()` in `js/ui.js`), Texte landen ausschließlich als `createTextNode`. Es gibt **keine** Verwendung von `innerHTML`, `insertAdjacentHTML`, `document.write`, `eval` oder `new Function` – geprüft per Suche über alle Dateien. Adressen aus fremden Feeds laufen durch `externURL()` und werden nur als Link gesetzt, wenn sie `http`/`https` sind; `javascript:` und `data:` werden verworfen (Test im Browser durchgeführt). Das Werkzeug verwirft solche Adressen schon beim Einsammeln (`ist_webadresse`, Test `nur_webadressen_kommen_aus_dem_feed`). |
| V6 Kryptografie | **nicht zutreffend** | Keine eigenen Geheimnisse, keine Verschlüsselung eigener Daten. SHA-256 dient der Änderungserkennung, nicht dem Schutz. |
| V7 Fehlerbehandlung, Protokollierung | **erfüllt** | Jeder Screening-Lauf schreibt ein Protokoll je Quelle mit Status und Grund (`daten/screening-laeufe.json`, in der App sichtbar). Fehlermeldungen enthalten keine Geheimnisse. |
| V8 Datenschutz | **erfüllt** | Es werden keine personenbezogenen Daten verarbeitet oder übertragen. Was der Nutzer anlegt – Lesezeichen, Notizen, Lernfortschritt und der optionale Name auf der Teilnahmebestätigung – bleibt im `localStorage` seines Geräts. Unter „Lesezeichen & Notizen“ steht, was gespeichert ist, dazu ein Export als JSON und **„Alles löschen“** (`nutzer.loeschen()`, löscht den Schlüssel und setzt den Zustand zurück; geprüft). |
| V9 Kommunikation | **erfüllt** | Auslieferung über HTTPS; `github.io` steht in der HSTS-Preload-Liste. Das Werkzeug ruft **ausschließlich** über `https` ab (Leine, siehe unten). |
| V10 Schadcode | **erfüllt** | Kein Build, kein Paketmanager, kein CDN auf der Seite: die App hat **null** Abhängigkeiten. Das Werkzeug hat 12 direkte und 190 Kisten insgesamt, festgehalten in `werkzeug/abhaengigkeiten.txt`, mit `Cargo.lock` im Repository. Offen: regelmäßiger Lauf von `cargo audit` (siehe „Offene Punkte“). |
| V11 Geschäftslogik | **erfüllt** | Der Datenbestand wird vor jedem Push maschinell geprüft (`kompass pruefen` im Auftrag „Pruefung“). Ein Parser, der weniger als drei Fundstellen liefert, bricht ab, statt den Bestand zu überschreiben (Test `kaputte_quelle_ueberschreibt_den_bestand_nicht`). Fassungen sind unveränderlich; `--erzwingen` schreibt die vorhandene Fassung neu, erfindet aber keine. |
| V12 Dateien, Ressourcen | **erfüllt** | Die App lädt keine Dateien hoch und nimmt keine an. Das Werkzeug packt ZIP-Archive von gesetze-im-internet.de aus und nimmt daraus **genau eine** XML-Datei nach Namensendung – keine Pfade aus dem Archiv werden zum Schreiben verwendet. |
| V13 API | **nicht zutreffend** | Keine eigene Schnittstelle. |
| V14 Konfiguration | **erfüllt** | Sicherheitsvorgaben als `meta`-Angaben in `index.html`, weil GitHub Pages keine eigenen Kopfzeilen setzt: `Content-Security-Policy` mit `default-src 'none'`, `script-src 'self'`, `frame-ancestors 'none'`, `base-uri 'none'`, `form-action 'none'`, dazu `referrer: strict-origin-when-cross-origin`. `'unsafe-inline'` steht **nur** bei `style-src` (Stilblock im Kopf und `style`-Attribute im Code), nicht bei `script-src`. Alle zwölf Ansichten wurden mit einem Lauscher auf `securitypolicyviolation` durchgeklickt: keine Verstöße, keine Fehler. |

## Die Leine: Schutz vor SSRF

Der Auftrag verlangt eine Allowlist der Quell-Domains und keine Abrufe interner
Adressen. Das erledigt `werkzeug/src/netz.rs` für **alle** Abrufe – Konnektoren,
Feeds, Seitenüberwachung und SPARQL-Dienst:

1. nur `https`, kein anderer Port als 443,
2. Host steht in `daten/quellen.json` unter `erlaubte_hosts` (echte Unterdomänen
   eingeschlossen, `bsi.bund.de.boese.test` aber nicht),
3. keine Adressliterale – damit sind `127.0.0.1`, `[::1]` und `169.254.169.254`
   ausgeschlossen, ohne dass DNS befragt werden muss,
4. keine Zugangsdaten in der Adresse,
5. **Umleitungen werden erneut geprüft**; mehr als fünf brechen ab.

Die Liste ist Daten, nicht Code: eine neue Quelle braucht einen Eintrag in
`quellen.json`. `kompass pruefen` hält jede konfigurierte Adresse dagegen – zurzeit
42 Adressen gegen 13 erlaubte Hosts. Gegenprobe durchgeführt: mit drei
eingeschmuggelten Quellen (`169.254.169.254`, fremder Host, `http` statt `https`)
endet `pruefen` mit Fehler und nennt jeden Grund einzeln.

## Der Auslöser des täglichen Laufs

Es gibt keinen öffentlichen Endpunkt, der etwas auslösen kann. Der Lauf startet
über den Cron von GitHub Actions oder von Hand über „Run workflow“ – beides setzt
Schreibrechte am Repository voraus. Der Auftrag `screening.yml` arbeitet mit dem
Standard-Token (`contents: write`, `actions: write`), kein eigenes Geheimnis.
`concurrency: screening` verhindert, dass zwei Läufe gleichzeitig schreiben, und
`--nur-um 06:30` begrenzt die geplanten Läufe auf einen pro Tag.

## Was bewusst so bleibt

- **Die Seite ist öffentlich lesbar.** Sie enthält nur veröffentlichte
  Rechtstexte, eigene Zusammenfassungen und öffentliche Meldungen – nichts
  Unternehmensinternes. Wer sie für interne Inhalte nutzen will, braucht einen
  anderen Betriebsweg (siehe `BETRIEB.md`).
- **Kein Integritätsschutz der Daten gegenüber dem Browser.** Wer die Seite
  ausliefert, bestimmt den Inhalt; der Schutz liegt im Git-Verlauf, nicht in einer
  Signatur.
- **`localStorage` statt verschlüsselter Ablage.** Notizen sind keine
  Geheimnisse; wer das Gerät hat, hat sie. Dafür gibt es den Löschknopf.

## Offene Punkte

1. **`cargo audit` regelmäßig laufen lassen** – am besten als eigener Schritt im
   Auftrag „Pruefung“. Hier auf dem Rechner ist das Werkzeug nicht installiert;
   die Abhängigkeitsliste liegt bereits als `werkzeug/abhaengigkeiten.txt` vor.
2. **Abhängigkeiten der Actions auf Commit-Stand festnageln** (`actions/checkout@<sha>`
   statt `@v4`) – schützt vor einem übernommenen Tag.
3. **Kopfzeilen statt `meta`** – CSP als echte HTTP-Kopfzeile samt
   `X-Content-Type-Options` und `Permissions-Policy` geht erst, wenn die Seite
   nicht mehr über GitHub Pages läuft.

## Testabdeckung

Gemessen mit `cargo llvm-cov` (Stand 01.10.2026, 56 Tests):

| Teil | Zeilen abgedeckt |
|---|---|
| `verweise.rs` (Verweise aus dem Wortlaut) | 99 % |
| `suche.rs` (Stammbildung, Index) | 96 % |
| `zusammenfassung.rs` (eigene Zusammenfassungen) | 96 % |
| `screening.rs` (Feeds, Relevanz, Dedup, Datum) | 90 % |
| `netz.rs` (Leine) | 89 % |
| `modell.rs` (Datenmodell) | 84 % |
| `cellar.rs` (EU-Recht-Parser) | 84 % |
| `gii.rs` (Parser deutsche Gesetze) | 81 % |
| **Kernlogik zusammen** | **89,3 %** |
| `main.rs` (Ablaufsteuerung, Versionierung) | 55 % |
| ganzes Werkzeug | 77 % |

Das Kriterium des Auftrags („Kernlogik ≥ 80 %“) ist damit erfüllt. Was in
`main.rs` offen bleibt, sind die **Abrufpfade** – sie werden in Tests bewusst nicht
ausgeführt, weil Tests ohne Netz laufen sollen; der Auftrag „Pruefung“ hält das
mit einer Probe fest. Die Ablauftests in `werkzeug/tests/ablauf.rs` gehen den
ganzen Weg durch das fertige Programm: Erstaufnahme, neue Fassung mit erkannten
Änderungen, unveränderliches Archiv, Abbruch bei kaputter Quelle, `--erzwingen`,
Ableitung von Index und Verweisen, Prüfbefunde, Lernstand und Zeitfenster.
