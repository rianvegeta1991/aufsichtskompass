// Newsfeed und Screening-Protokoll.
//
// Die Meldungen liegen nach Monaten getrennt (daten/news/<jahr>-<monat>.json); geladen
// werden nur die Monate, die gebraucht werden. Was der Nutzer damit macht - gelesen,
// gemerkt, Rückmeldung zur Relevanz - bleibt ausschließlich auf seinem Gerät.

import { el, leere, chip, datum, zahl, kurz, titel, tabelle, exportknoepfe, fehlerkarte, ton, externURL } from './ui.js';
import * as daten from './daten.js';

/**
 * Link auf eine Meldung. Die Adresse stammt aus einem fremden Feed, deshalb geht
 * sie durch `externURL`; trägt sie etwas anderes als http(s), bleibt der Titel
 * stehen – nur ohne Link (Phase 8).
 */
function aussenlink(url, ...inhalt) {
  const u = externURL(url);
  return u
    ? el('a', { href: u, target: '_blank', rel: 'noopener noreferrer' }, ...inhalt)
    : el('span', { title: 'Die Quelle hat keine verwendbare Adresse mitgeliefert.' }, ...inhalt);
}
import * as nutzer from './nutzer.js';

const WURZEL = 'daten/news/';
const gecacht = new Map();

function hole(datei) {
  if (!gecacht.has(datei)) {
    gecacht.set(datei, fetch(WURZEL + datei).then((a) => {
      if (!a.ok) throw new Error(`${datei}: HTTP ${a.status}`);
      return a.json();
    }).catch((e) => { gecacht.delete(datei); throw e; }));
  }
  return gecacht.get(datei);
}

export const verzeichnis = () => hole('index.json').catch(() => ({ monate: [] }));
export const monatsdatei = (m) => hole(`${m}.json`).catch(() => ({ meldungen: [] }));

/** Die neuesten Meldungen über mehrere Monatsdateien hinweg. */
export async function neueste(anzahl = 12) {
  const v = await verzeichnis();
  const monate = (v.monate || []).slice(0, 2);
  const teile = await Promise.all(monate.map((m) => monatsdatei(m.monat)));
  return teile
    .flatMap((t) => t.meldungen || [])
    .sort((a, b) => String(b.datum).localeCompare(String(a.datum)))
    .slice(0, anzahl);
}

const KATEGORIE_KLASSE = {
  'Neue Regulierung': 'bindend',
  'Änderung': 'zusammenfassung',
  Konsultation: 'auslegung',
  Aufsichtspraxis: 'auslegung',
  Sanktion: 'warn',
  'Vorfall & Bedrohungslage': 'warn',
  Fachartikel: '',
};

// ------------------------------------------------------------------ Newsfeed

export async function zeigen(wurzel, unterseite = '') {
  if (unterseite === 'protokoll') return protokoll(wurzel);
  titel('Screening');
  leere(wurzel);

  const v = await verzeichnis();
  if (!v.monate || !v.monate.length) {
    wurzel.append(
      el('div.kopfzeile', el('div.wachs', el('h1', 'Markt- & Compliance-Screening'))),
      el('div.karte',
        el('h2', 'Noch kein Lauf'),
        el('p', 'Es liegen noch keine Meldungen vor. Der Lauf startet täglich um 06:30 Uhr über GitHub Actions; von Hand geht er mit ', el('code', 'kompass screening'), '.')),
    );
    return;
  }

  const gesamt = v.monate.reduce((n, m) => n + m.meldungen, 0);
  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('h1', 'Markt- & Compliance-Screening'),
      el('p.unterzeile', `${zahl(gesamt)} Meldungen aus ${v.monate.length} Monaten. Täglich 06:30 Uhr; die Bewertung nennt die Begriffe, die gegriffen haben.`)),
      el('div', el('a.knopf', { href: '#/newsfeed/protokoll' }, 'Protokoll der Läufe'))),
  );

  const filter = { text: '', kategorie: '', quelle: '', monat: v.monate[0].monat, nurUngelesen: false, nurGemerkt: false };
  const leiste = el('div.filterleiste');
  const liste = el('div');
  wurzel.append(leiste, liste);

  let meldungen = [];

  async function laden() {
    liste.replaceChildren(el('p.leer', el('span.lade', { 'aria-hidden': 'true' }), ' lädt …'));
    const d = await monatsdatei(filter.monat);
    meldungen = d.meldungen || [];
    aufbauenLeiste();
    zeichnen();
  }

  function aufbauenLeiste() {
    leere(leiste);
    const quellen = [...new Set(meldungen.map((m) => m.quelle_name))].sort((a, b) => a.localeCompare(b, 'de'));
    const kategorien = [...new Set(meldungen.map((m) => m.kategorie))].sort();

    leiste.append(
      el('input', {
        type: 'search', placeholder: 'In den Meldungen suchen …', 'aria-label': 'Meldungen filtern',
        value: filter.text, oninput: (e) => { filter.text = e.target.value.toLowerCase(); zeichnen(); },
      }),
      auswahl('Alle Monate', v.monate.map((m) => [m.monat, `${m.monat} (${m.meldungen})`]), filter.monat,
        (w) => { filter.monat = w; laden(); }),
      auswahl('Alle Kategorien', kategorien.map((k) => [k, k]), filter.kategorie,
        (w) => { filter.kategorie = w; zeichnen(); }, true),
      auswahl('Alle Quellen', quellen.map((q) => [q, q]), filter.quelle,
        (w) => { filter.quelle = w; zeichnen(); }, true),
      schalter('nur ungelesene', filter.nurUngelesen, (w) => { filter.nurUngelesen = w; zeichnen(); }),
      schalter('nur gemerkte', filter.nurGemerkt, (w) => { filter.nurGemerkt = w; zeichnen(); }),
    );
  }

  function zeichnen() {
    const passend = meldungen.filter((m) => {
      if (filter.kategorie && m.kategorie !== filter.kategorie) return false;
      if (filter.quelle && m.quelle_name !== filter.quelle) return false;
      if (filter.nurUngelesen && nutzer.istGelesen(m.id)) return false;
      if (filter.nurGemerkt && !nutzer.istGemerktNews(m.id)) return false;
      if (filter.text) {
        const heu = `${m.titel} ${m.zusammenfassung} ${m.quelle_name}`.toLowerCase();
        if (!heu.includes(filter.text)) return false;
      }
      return true;
    });

    leere(liste);
    liste.append(el('p.unterzeile', { style: 'margin:4px 0 12px' },
      `${passend.length} von ${meldungen.length} Meldungen · ${meldungen.filter((m) => !nutzer.istGelesen(m.id)).length} ungelesen`));
    if (!passend.length) {
      liste.append(el('p.leer', 'Keine Meldung passt zu dieser Auswahl.'));
      return;
    }
    for (const m of passend) liste.append(karte(m));
    liste.append(exportknoepfe(`screening-${filter.monat}`,
      ['Datum', 'Titel', 'Quelle', 'Kategorie', 'Punkte', 'Begründung', 'Adresse'],
      () => passend.map((m) => [datum(m.datum), m.titel, m.quelle_name, m.kategorie, m.punkte, (m.begruendung || []).join(', '), m.url])));
  }

  laden();
}

function auswahl(beschriftung, werte, gewaehlt, beiWahl, leerErlaubt = true) {
  const s = el('select', { 'aria-label': beschriftung, onchange: (e) => beiWahl(e.target.value) });
  if (leerErlaubt) s.append(el('option', { value: '' }, beschriftung));
  for (const [w, t] of werte) {
    s.append(el('option', { value: w, selected: w === gewaehlt ? true : null }, t));
  }
  return s;
}

function schalter(beschriftung, an, beiWechsel) {
  return el('label', { style: 'display:flex;align-items:center;gap:7px;min-height:44px;font-size:.88rem' },
    el('input', { type: 'checkbox', checked: an ? true : null, onchange: (e) => beiWechsel(e.target.checked) }),
    el('span', beschriftung));
}

function karte(m) {
  const gelesen = nutzer.istGelesen(m.id);
  const gemerkt = nutzer.istGemerktNews(m.id);
  const rueck = nutzer.rueckmeldung(m.id);

  const k = el('article.karte.meldung', { dataset: { gelesen: gelesen ? 'ja' : 'nein' } });
  k.append(
    el('div.reihe', { style: 'display:flex;flex-wrap:wrap;gap:6px;align-items:center;margin-bottom:6px' },
      chip(m.kategorie, KATEGORIE_KLASSE[m.kategorie] ?? ''),
      chip(m.quelle_name),
      el('span.punktzahl', datum(m.datum)),
      el('span.punktzahl', { title: (m.begruendung || []).join(' · ') }, `Relevanz ${m.punkte}`)),
    el('h2', { style: 'font-size:1.03rem;margin:0 0 5px' },
      aussenlink(m.url, m.titel, ' ↗')),
    m.zusammenfassung ? el('p', { style: 'margin:0 0 8px;color:var(--color-text-muted)' }, kurz(m.zusammenfassung, 300)) : null,
  );

  if (m.begruendung && m.begruendung.length) {
    k.append(el('details', { style: 'margin-bottom:8px' },
      el('summary', { style: 'cursor:pointer;font-size:.82rem;color:var(--color-text-muted)' }, 'Warum ist das hier gelandet?'),
      el('div', { style: 'display:flex;flex-wrap:wrap;gap:6px;margin-top:6px' },
        m.begruendung.map((b) => chip(b)))));
  }

  const bezuege = [
    ...(m.regelwerke || []).map((r) => el('a.chip', { href: `#/rw/${r}`, style: 'text-decoration:none' }, r)),
    ...(m.themen || []).map((t) => el('a.chip', { href: `#/themen/${t}`, style: 'text-decoration:none' }, t)),
  ];
  if (bezuege.length) {
    k.append(el('div', { style: 'display:flex;flex-wrap:wrap;gap:6px;margin-bottom:8px' }, bezuege));
  }

  if (m.vorschlag) {
    k.append(el('div.hinweis.phase', { style: 'font-size:.84rem;margin-bottom:8px' },
      el('div', el('strong', 'Vorschlag: '), 'In die Bibliothek aufnehmen – CELEX ', el('code', m.vorschlag.celex),
        '. Dazu in ', el('code', 'daten/regelwerke.json'), ' und ', el('code', 'daten/quellen.json'),
        ' je einen Eintrag ergänzen und ', el('code', 'kompass abruf'), ' laufen lassen.')));
  }

  const knopfGelesen = el('button.knopf', { type: 'button' }, gelesen ? '✓ gelesen' : 'als gelesen merken');
  knopfGelesen.addEventListener('click', () => {
    const nun = nutzer.gelesenUmschalten(m.id);
    knopfGelesen.textContent = nun ? '✓ gelesen' : 'als gelesen merken';
    k.dataset.gelesen = nun ? 'ja' : 'nein';
  });
  const knopfMerken = el('button.knopf', { type: 'button', 'aria-pressed': gemerkt ? 'true' : 'false' },
    gemerkt ? '★ gemerkt' : '☆ merken');
  knopfMerken.addEventListener('click', () => {
    const nun = nutzer.newsMerken(m.id, m.titel, m.url);
    knopfMerken.textContent = nun ? '★ gemerkt' : '☆ merken';
    knopfMerken.setAttribute('aria-pressed', nun ? 'true' : 'false');
  });

  const rueckKnoepfe = el('div', { style: 'display:flex;gap:6px;margin-left:auto' },
    ...[['relevant', 'passt'], ['nicht', 'passt nicht']].map(([wert, text]) => {
      const b = el('button.knopf', { type: 'button', 'aria-pressed': rueck === wert ? 'true' : 'false' }, text);
      b.addEventListener('click', () => {
        nutzer.rueckmeldungSetzen(m.id, rueck === wert ? '' : wert, m.begruendung || []);
        ton(rueck === wert ? 'Rückmeldung zurückgenommen.' : 'Rückmeldung gespeichert – sie steht unter „Eigenes".');
        for (const x of rueckKnoepfe.children) x.setAttribute('aria-pressed', 'false');
        if (rueck !== wert) b.setAttribute('aria-pressed', 'true');
      });
      return b;
    }));

  k.append(el('div', { style: 'display:flex;flex-wrap:wrap;gap:8px;align-items:center;padding-top:8px;border-top:1px solid var(--color-border)' },
    knopfGelesen, knopfMerken, rueckKnoepfe));
  return k;
}

// ------------------------------------------------------------------ Protokoll

async function protokoll(wurzel) {
  titel('Screening-Protokoll');
  leere(wurzel);
  const d = await fetch('daten/screening-laeufe.json').then((a) => a.json()).catch(() => null);
  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('p.brotkrumen', el('a', { href: '#/newsfeed' }, 'Screening'), ' › Protokoll'),
      el('h1', 'Protokoll der Läufe'),
      el('p.unterzeile', 'Jeder Lauf mit Startzeit, Dauer, Quellenstatus und Trefferzahl. Fehler einer Quelle brechen den Lauf nicht ab – sie stehen hier.'))),
  );
  if (!d || !d.laeufe || !d.laeufe.length) {
    wurzel.append(el('p.leer', 'Noch kein Lauf protokolliert.'));
    return;
  }
  for (const l of d.laeufe.slice(0, 12)) {
    const fehler = l.quellen.filter((q) => q.status === 'Fehler' || q.status === 'Warnung');
    wurzel.append(el('div.karte', { style: 'margin-bottom:14px' },
      el('div', { style: 'display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin-bottom:8px' },
        el('strong', datum(l.start)),
        el('span.punktzahl', new Date(l.start).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' }) + ' Uhr'),
        chip(`${l.dauer_s} s`),
        chip(`${zahl(l.geprueft)} geprüft`),
        chip(`${l.neu} neu`, l.neu ? 'original' : ''),
        chip(l.ausloeser),
        fehler.length ? chip(`${fehler.length} auffällig`, 'warn') : chip('alle Quellen ok', 'original')),
      tabelle(['Quelle', 'Status', 'gefunden', 'übernommen', 'Hinweis'],
        l.quellen.map((q) => [
          q.name, q.status, String(q.gefunden), String(q.uebernommen), q.fehler || '',
        ]))));
  }
}

// -------------------------------------------------------------------- Digest

/** Tages-Digest für die Startseite: das Neueste aus den letzten sieben Tagen. */
export async function digest(anzahl = 6) {
  const liste = await neueste(40);
  const grenze = Date.now() - 7 * 24 * 3600 * 1000;
  const frisch = liste.filter((m) => new Date(m.datum).getTime() >= grenze);
  return (frisch.length ? frisch : liste).slice(0, anzahl);
}
