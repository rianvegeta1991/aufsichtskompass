// Gerüst und Router. Alle Adressen laufen über den Hash, damit Deep-Links auch auf
// GitHub Pages funktionieren (dort gibt es kein Rewrite auf index.html):
//   #/                       Start
//   #/bibliothek             Katalog
//   #/rw/dora/art/28/abs/4   Fundstelle im Viewer
//   #/suche/Informationsregister
//   #/themen  ·  #/themen/iks  ·  #/aenderungen  ·  #/quellen  ·  #/lesezeichen
// 404.html rechnet zusätzlich Pfad-Adressen (…/dora/art/28) in diese Hash-Form um.

import { el, leere, chip, datum, zahl, titel, kurz, fehlerkarte, ton } from './ui.js';
import * as daten from './daten.js';
import * as nutzer from './nutzer.js';
import * as bibliothek from './bibliothek.js';
import * as viewer from './viewer.js';
import * as suche from './suche.js';

export const APP_VERSION = '1.0';

const seite = document.getElementById('seite');

// ------------------------------------------------------------ Erscheinungsbild

function themaKnopfBeschriften() {
  const k = document.getElementById('thema-knopf');
  const w = nutzer.thema();
  const zeichen = { system: '◐', hell: '☀', dunkel: '☾' }[w];
  const wort = { system: 'Systemeinstellung', hell: 'Hell', dunkel: 'Dunkel' }[w];
  k.textContent = `${zeichen} ${wort}`;
  k.setAttribute('title', `Erscheinungsbild: ${wort} – zum Wechseln klicken`);
}

function themaEinrichten() {
  nutzer.themaAnwenden();
  themaKnopfBeschriften();
  document.getElementById('thema-knopf').addEventListener('click', () => {
    const folge = { system: 'hell', hell: 'dunkel', dunkel: 'system' };
    nutzer.themaSetzen(folge[nutzer.thema()]);
    themaKnopfBeschriften();
  });
}

// ------------------------------------------------------------------ Navigation

const MENUE = [
  { pfad: '', name: 'Start', zeichen: '⌂' },
  { pfad: 'bibliothek', name: 'Bibliothek', zeichen: '▤', zahl: 'regelwerke' },
  { pfad: 'themen', name: 'Themen', zeichen: '◈', zahl: 'themen' },
  { pfad: 'suche', name: 'Suche', zeichen: '⌕' },
  { trenner: 'Aktualität' },
  { pfad: 'aenderungen', name: 'Änderungen & Archiv', zeichen: '⟳', zahl: 'aenderungen' },
  { pfad: 'newsfeed', name: 'Screening', zeichen: '◎', stufe: 'Phase 5' },
  { trenner: 'Analyse' },
  { pfad: 'matrizen', name: 'Matrizen & Graph', zeichen: '⊞', stufe: 'Phase 4' },
  { pfad: 'lernen', name: 'Lernbereich', zeichen: '✎', stufe: 'Phase 7' },
  { trenner: 'Eigenes' },
  { pfad: 'lesezeichen', name: 'Lesezeichen & Notizen', zeichen: '★' },
  { pfad: 'quellen', name: 'Quellen & Betrieb', zeichen: '⚙' },
];

async function navZeichnen(aktiv) {
  const nav = document.getElementById('nav');
  const [k, th, ae] = await Promise.all([daten.katalog(), daten.themen(), daten.aenderungen()]);
  const zahlen = {
    regelwerke: k.regelwerke.length,
    themen: th.themen.length,
    aenderungen: (ae.ereignisse || []).length,
  };
  leere(nav);
  for (const m of MENUE) {
    if (m.trenner) { nav.append(el('h2', m.trenner)); continue; }
    nav.append(el('a', {
      href: '#/' + m.pfad,
      'aria-current': aktiv === m.pfad ? 'page' : null,
    },
      el('span', { 'aria-hidden': 'true', style: 'width:1.1em;text-align:center' }, m.zeichen),
      el('span', m.name),
      m.zahl ? el('span.zahl', zahl(zahlen[m.zahl])) : null,
      m.stufe ? el('span.stufe', m.stufe) : null));
  }
  nav.append(el('p', { style: 'margin:22px 10px 0;font-size:.72rem;color:var(--color-text-muted)' },
    `Version ${APP_VERSION}`));
}

// ---------------------------------------------------------------------- Router

function adresse() {
  const roh = decodeURIComponent(location.hash.replace(/^#\/?/, ''));
  const teile = roh.split('/').filter((t) => t !== '');
  return { roh, teile };
}

async function leiten() {
  const { teile } = adresse();
  const wurzel = teile[0] || '';
  await navZeichnen(wurzel);
  leere(seite).append(el('p.leer', el('span.lade', { 'aria-hidden': 'true' }), ' lädt …'));
  document.body.classList.remove('nav-offen');
  document.getElementById('nav-knopf').setAttribute('aria-expanded', 'false');

  try {
    switch (wurzel) {
      case '': await start(seite); break;
      case 'bibliothek': await bibliothek.zeigen(seite); break;
      case 'rw': await viewer.zeigen(seite, teile[1], teile.slice(2).join('/')); break;
      case 'suche': await suchseite(seite, teile.slice(1).join('/')); break;
      case 'themen': await themenseite(seite, teile[1]); break;
      case 'aenderungen': await aenderungsseite(seite); break;
      case 'lesezeichen': await lesezeichenseite(seite); break;
      case 'quellen': await quellenseite(seite); break;
      case 'newsfeed': platzhalter(seite, 'Markt- & Compliance-Screening', 5,
        'Täglich 06:30 Europe/Berlin über einen GitHub-Actions-Lauf, dazu ein Knopf für den Lauf von Hand: Feeds von BaFin, EIOPA, BSI, ENISA und anderen, Seitenüberwachung für Quellen ohne Feed, Deduplizierung, begründete Relevanzbewertung und Tages-Digest.'); break;
      case 'matrizen': platzhalter(seite, 'Interdependenzen, Matrizen und Graph', 4,
        'Mapping-Matrix Regelwerk × Regelwerk, Heatmap Thema × Regelwerk, Meldepflichten-Matrix (DORA / BSIG / DSGVO), RACI- und Three-Lines-Sicht, Netzwerkgraph und Zeitstrahl – mit Export als CSV und XLSX.'); break;
      case 'lernen': platzhalter(seite, 'Lernbereich', 7,
        'Lektionen je Regelwerk und Thema, Cheat Sheets, sechs Quiz-Typen, Fallstudien mit Entscheidungsbaum, Karteikarten mit Spaced Repetition und Lernfortschritt.'); break;
      default:
        leere(seite).append(fehlerkarte(`Die Adresse „${location.hash}" kennt die App nicht.`));
    }
  } catch (e) {
    console.error(e);
    leere(seite).append(fehlerkarte('Beim Laden der Daten ist etwas schiefgegangen.', e.message));
  }
  document.getElementById('inhalt').focus({ preventScroll: true });
}

// ------------------------------------------------------------------- Startseite

async function start(wurzel) {
  titel(null);
  const [k, ae] = await Promise.all([daten.katalog(), daten.aenderungen()]);
  const mitText = await Promise.all(k.regelwerke.map(async (r) => ({ r, f: await daten.aktuelleFassung(r.id) })));
  const volltexte = mitText.filter((x) => x.f);
  const fundstellen = volltexte.reduce((s, x) => s + x.f.fundstellen, 0);
  const level2 = k.regelwerke.filter((r) => r.gruppe === 'dora-level2').length;

  leere(wurzel);
  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('h1', 'Aufsichtskompass'),
      el('p.unterzeile', 'IT-Compliance und IT-Governance für deutsche Versicherungsunternehmen unter Solvency II – Wissensbasis, Analysewerkzeug, Frühwarnsystem und Lernplattform.'))),
    el('div.gitter.drei',
      kennzahl(zahl(k.regelwerke.length), 'Regelwerke im Katalog', '#/bibliothek'),
      kennzahl(zahl(volltexte.length), 'davon mit Volltext', '#/bibliothek'),
      kennzahl(zahl(fundstellen), 'adressierbare Fundstellen', '#/suche/Informationsregister'),
      kennzahl(zahl(level2), 'DORA-Level-2-Rechtsakte', '#/rw/dora')),
  );

  const schnell = [
    ['DORA – Artikel 28: Allgemeine Prinzipien (Drittparteienrisiko)', '#/rw/dora/art/28'],
    ['DORA – Artikel 6: IKT-Risikomanagementrahmen', '#/rw/dora/art/6'],
    ['DORA – Artikel 3: Begriffsbestimmungen (65 Begriffe)', '#/rw/dora/art/3'],
    ['DORA – Artikel 19: Meldung schwerwiegender IKT-Vorfälle', '#/rw/dora/art/19'],
    ['RTS 2024/1774 – IKT-Risikomanagement im Detail', '#/rw/dora-rts-1774'],
    ['ITS 2024/2956 – Informationsregister', '#/rw/dora-its-2956'],
  ];

  wurzel.append(el('div.gitter.zwei', { style: 'margin-top:18px' },
    el('div.karte',
      el('h2', 'Schnellzugriff'),
      el('ul', { style: 'margin:0;padding-left:1.1em' }, schnell.map(([t, h]) => el('li', { style: 'margin:5px 0' }, el('a', { href: h }, t))))),
    el('div.karte',
      el('h2', 'Zuletzt übernommen'),
      (ae.ereignisse || []).length
        ? el('table.liste', el('tbody', [...ae.ereignisse].reverse().slice(0, 6).map((e) => el('tr',
            el('td', el('a', { href: `#/rw/${e.regelwerk}` }, kurzname(k, e.regelwerk))),
            el('td', e.art),
            el('td', datum(e.zeitpunkt))))))
        : el('p.unterzeile', 'Noch keine Änderung protokolliert.'),
      el('p', { style: 'margin:10px 0 0' }, el('a', { href: '#/aenderungen' }, 'Alle Änderungen ansehen →'))),
  ));

  const lz = nutzer.lesezeichen();
  if (lz.length) {
    wurzel.append(el('div.karte', { style: 'margin-top:18px' },
      el('h2', 'Deine Lesezeichen'),
      el('ul', { style: 'margin:0;padding-left:1.1em' },
        lz.slice(0, 6).map((l) => el('li', el('a', { href: `#/rw/${l.rw}/${l.pfad}` }, l.bez))))));
  }

  wurzel.append(el('div.karte', { style: 'margin-top:18px' },
    el('h2', 'Ausbaustand'),
    el('table.liste',
      el('thead', el('tr', el('th', 'Phase'), el('th', 'Inhalt'), el('th', 'Stand'))),
      el('tbody',
        phasenzeile('1', 'Fundament: Datenmodell, Design-Tokens hell/dunkel, Gerüst, Werkzeugkette', 'fertig'),
        phasenzeile('2', 'Bibliothek und Viewer: CELLAR-Konnektor, DORA (DE/EN) und Level-2-Rechtsakte, Deep-Links, Suche', 'fertig für EU-Recht'),
        phasenzeile('2b', 'Konnektor für gesetze-im-internet.de (VAG, BSIG, BDSG, HGB, AO) und BaFin-PDF (MaGo, MaRisk)', 'offen'),
        phasenzeile('3', 'Eigene Zusammenfassungen: ISO 27001, COBIT 2019, ITIL 4, CSA CCM, C5, NIST CSF, GDV', 'offen'),
        phasenzeile('4', 'Themenseiten, Mappings, Matrizen, Heatmap, Graph, Export', 'offen'),
        phasenzeile('5', 'Tägliches Screening mit Newsfeed und Digest', 'offen'),
        phasenzeile('6', 'Change Detection, Archiv, Diff-Ansicht, Benachrichtigung', 'Grundlage steht (Hashes, Änderungslog)'),
        phasenzeile('7', 'Lernbereich mit Quizzes, Fallstudien, Karteikarten', 'offen'),
        phasenzeile('8', 'Härtung: Tests, Security-Review, Betriebsdoku', 'teilweise (Werkzeug-Tests)'),
      ))));
}

function phasenzeile(nr, inhalt, stand) {
  const fertig = stand.startsWith('fertig');
  return el('tr', el('td', el('strong', nr)), el('td', inhalt),
    el('td', chip(stand, fertig ? 'original' : stand === 'offen' ? '' : 'zusammenfassung')));
}

function kurzname(katalog, id) {
  const r = katalog.regelwerke.find((x) => x.id === id);
  return r ? r.kurzname : id;
}

function kennzahl(wert, beschriftung, ziel) {
  const inhalt = [el('div.kennzahl-titel', beschriftung), el('div.kennzahl', wert)];
  return ziel
    ? el('a.karte', { href: ziel, style: 'text-decoration:none;color:inherit' }, ...inhalt)
    : el('div.karte', ...inhalt);
}

// ------------------------------------------------------------------ Suchseite

async function suchseite(wurzel, frage) {
  titel(frage ? `Suche: ${frage}` : 'Suche');
  leere(wurzel);
  wurzel.append(el('div.kopfzeile', el('div.wachs',
    el('h1', 'Suche'),
    el('p.unterzeile', 'Durchsucht alle abgerufenen Fundstellen. Gewichtung nach BM25; Wortstämme, damit „Meldungen" auch „Meldung" findet.'))));

  const feld = el('input', {
    type: 'search', value: frage || '', style: 'width:100%;max-width:560px;padding:10px 12px',
    'aria-label': 'Suchbegriff', placeholder: 'z. B. Informationsregister, Ausstiegsstrategie, TLPT',
  });
  const form = el('form', { onsubmit: (e) => { e.preventDefault(); location.hash = '#/suche/' + encodeURIComponent(feld.value); } },
    el('div.filterleiste', feld, el('button.knopf.haupt', { type: 'submit' }, 'Suchen')));
  const raum = el('div');
  wurzel.append(form, raum);
  if (!frage) {
    raum.append(el('p.leer', 'Begriff eingeben. Beispiele: ',
      el('a', { href: '#/suche/Informationsregister' }, 'Informationsregister'), ' · ',
      el('a', { href: '#/suche/Ausstiegsstrategie' }, 'Ausstiegsstrategie'), ' · ',
      el('a', { href: '#/suche/Notfall' }, 'Notfall')));
    feld.focus();
    return;
  }

  raum.append(el('p.leer', el('span.lade', { 'aria-hidden': 'true' }), ' sucht …'));
  const t0 = performance.now();
  const { treffer, begriffe, durchsucht, dokumente } = await suche.suchen(frage, { sprache: nutzer.hol('sprache') });
  const dauer = Math.round(performance.now() - t0);

  leere(raum);
  raum.append(el('p.unterzeile', { style: 'margin:6px 0 10px' },
    `${zahl(treffer.length)} Treffer in ${zahl(dokumente)} Fundstellen aus ${durchsucht} Indexdateien · ${dauer} ms · Stämme: `,
    ...begriffe.map((b) => chip(b))));

  if (!treffer.length) {
    raum.append(el('p.leer', 'Nichts gefunden. Regelwerke mit der Einstufung „nicht relevant" bleiben außen vor.'));
    return;
  }
  const liste = el('div.karte');
  for (const t of treffer.slice(0, 80)) {
    liste.append(el('div.treffer',
      el('div.zeile',
        el('a.chip', { href: `#/rw/${t.regelwerk}`, style: 'text-decoration:none' }, t.kurzname),
        chip(t.modus === 'original' ? 'Originaltext' : 'Zusammenfassung', t.modus === 'original' ? 'original' : 'zusammenfassung'),
        el('span.punktzahl', `Punkte ${t.punkte.toFixed(2)} · ${t.gefunden.length}/${begriffe.length} Begriffe`)),
      el('h3', el('a', { href: `#/rw/${t.regelwerk}/${t.pfad}` }, t.bezeichnung)),
      el('p.ausschnitt', ...suche.hervorheben(t.ausschnitt, begriffe))));
  }
  raum.append(liste);
  if (treffer.length > 80) raum.append(el('p.unterzeile', { style: 'margin-top:10px' }, `Es werden die 80 besten von ${zahl(treffer.length)} Treffern gezeigt.`));
}

// ------------------------------------------------------------------ Themenseite

async function themenseite(wurzel, id) {
  const [th, k] = await Promise.all([daten.themen(), daten.katalog()]);
  leere(wurzel);

  if (id) {
    const t = th.themen.find((x) => x.id === id);
    if (!t) { wurzel.append(fehlerkarte(`Das Thema „${id}" gibt es nicht.`)); return; }
    titel(t.name);
    const passend = k.regelwerke.filter((r) => (r.themen || []).includes(id));
    wurzel.append(
      el('div.kopfzeile', el('div.wachs',
        el('p.brotkrumen', el('a', { href: '#/themen' }, 'Themen'), ' › ', t.name),
        el('h1', t.name), el('p.unterzeile', t.kurz))),
      el('div.hinweis.phase',
        el('div', el('strong', 'Phase 4: '), 'Zielbild, Fundstellentabelle über alle Regelwerke, Prüfungsschwerpunkte und Best-Practice-Einordnungen kommen mit den Mappings. Bis dahin zeigt die Seite die Regelwerke, die dieses Thema berühren.')),
      el('h2', { style: 'margin-top:20px' }, `Regelwerke zu diesem Thema (${passend.length})`),
      el('div.gitter.zwei', passend.map((r) => el('div.karte',
        el('h3', { style: 'margin:0 0 4px' }, el('a', { href: `#/rw/${r.id}` }, r.kurzname)),
        el('p.lang', { style: 'margin:0;color:var(--color-text-muted);font-size:.85rem' }, kurz(r.langtitel, 140))))),
    );
    return;
  }

  titel('Themenübersichten');
  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('h1', 'Themenübersichten'),
      el('p.unterzeile', `${th.themen.length} Themen. Sie liegen als Daten vor – ein neues Thema braucht keine Code-Änderung.`))),
    el('div.gitter.drei', th.themen.map((t) => {
      const n = k.regelwerke.filter((r) => (r.themen || []).includes(t.id)).length;
      return el('a.karte', { href: `#/themen/${t.id}`, style: 'text-decoration:none;color:inherit' },
        el('h3', { style: 'margin:0 0 4px' }, t.name),
        el('p', { style: 'margin:0 0 8px;font-size:.86rem;color:var(--color-text-muted)' }, t.kurz),
        chip(`${n} Regelwerke`));
    })),
  );
}

// -------------------------------------------------------------- Änderungsseite

async function aenderungsseite(wurzel) {
  titel('Änderungen & Archiv');
  const [ae, k] = await Promise.all([daten.aenderungen(), daten.katalog()]);
  const ereignisse = [...(ae.ereignisse || [])].reverse();
  leere(wurzel);
  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('h1', 'Änderungen & Archiv'),
      el('p.unterzeile', 'Jede übernommene Fassung bleibt unverändert liegen. Grundlage sind SHA-256-Hashes je Fundstelle – daraus entstehen in Phase 6 Diff-Ansicht und „zu prüfen"-Markierungen.'))),
    el('div.hinweis.phase', el('div', el('strong', 'Phase 6: '),
      'Die Erfassung läuft schon (Hash je Fundstelle, Änderungslog, unveränderliche Fassungsordner). Es fehlen die Wort-Diff-Ansicht, das Benachrichtigungszentrum und die Folgenabschätzung auf Mappings und Lerninhalte.')),
    el('h2', { style: 'margin-top:20px' }, `Protokoll (${ereignisse.length})`),
    ereignisse.length
      ? el('div.karte', { style: 'padding:0;overflow:auto' }, el('table.liste',
          el('thead', el('tr', el('th', 'Regelwerk'), el('th', 'Fassung'), el('th', 'Art'), el('th', 'neu'), el('th', 'geändert'), el('th', 'entfallen'), el('th', 'Zeitpunkt'))),
          el('tbody', ereignisse.map((e) => el('tr',
            el('td', el('a', { href: `#/rw/${e.regelwerk}` }, kurzname(k, e.regelwerk))),
            el('td', datum(e.fassung)),
            el('td', e.art),
            el('td', String((e.neu || []).length)),
            el('td', String((e.geaendert || []).length)),
            el('td', String((e.entfallen || []).length)),
            el('td', datum(e.zeitpunkt)))))))
      : el('p.leer', 'Noch kein Ereignis protokolliert.'),
  );
}

// ------------------------------------------------------------ Lesezeichenseite

async function lesezeichenseite(wurzel) {
  titel('Lesezeichen & Notizen');
  const k = await daten.katalog();
  const lz = nutzer.lesezeichen();
  const notizen = [];
  for (const r of k.regelwerke) {
    for (const n of nutzer.notizenZu(r.id)) notizen.push({ rw: r, ...n });
  }
  leere(wurzel);
  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('h1', 'Lesezeichen & Notizen'),
      el('p.unterzeile', 'Bleibt ausschließlich auf diesem Gerät (localStorage). Nichts davon wird übertragen.'))),
    el('h2', { style: 'margin-top:14px' }, `Lesezeichen (${lz.length})`),
    lz.length
      ? el('div.karte', el('ul', { style: 'margin:0;padding-left:1.1em' }, lz.map((l) => el('li', { style: 'margin:5px 0' },
          el('a', { href: `#/rw/${l.rw}/${l.pfad}` }, l.bez),
          el('span.punktzahl', ' · ' + datum(l.zeit))))))
      : el('p.leer', 'Noch nichts gemerkt. Im Viewer auf „☆ merken" tippen.'),
    el('h2', { style: 'margin-top:20px' }, `Notizen (${notizen.length})`),
    notizen.length
      ? el('div.karte', notizen.map((n) => el('div', { style: 'padding:9px 0;border-bottom:1px solid var(--color-border)' },
          el('p', { style: 'margin:0 0 3px' }, el('strong', n.rw.kurzname), el('span.punktzahl', ' · Kennung ' + n.id)),
          el('p', { style: 'margin:0;white-space:pre-wrap' }, n.text))))
      : el('p.leer', 'Noch keine Notiz.'),
  );
}

// ----------------------------------------------------------------- Quellenseite

async function quellenseite(wurzel) {
  titel('Quellen & Betrieb');
  const q = await daten.quellen();
  const k = await daten.katalog();
  leere(wurzel);
  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('h1', 'Quellen & Betrieb'),
      el('p.unterzeile', 'Je Quelle: Abrufweg, Format, Nutzungsbedingung, Kosten. Alles hier ist kostenlos und ohne Anmeldung nutzbar.'))),
    el('div.karte', { style: 'padding:0;overflow:auto' }, el('table.liste',
      el('thead', el('tr', el('th', 'Regelwerk'), el('th', 'Abrufweg'), el('th', 'Kennung'), el('th', 'Sprachen'), el('th', 'Nutzung'), el('th', 'Kosten'), el('th', 'Aktiv'))),
      el('tbody', q.konnektoren.map((c) => el('tr',
        el('td', el('a', { href: `#/rw/${c.regelwerk}` }, kurzname(k, c.regelwerk))),
        el('td', c.typ),
        el('td', el('code', c.celex || c.kennung || '–')),
        el('td', (c.sprachen || []).join(', ').toUpperCase()),
        el('td', q.nutzungsbedingungen[c.nutzung] ? kurz(q.nutzungsbedingungen[c.nutzung], 90) : c.nutzung || '–'),
        el('td', 'kostenlos'),
        el('td', c.aktiv === false ? chip('inaktiv') : chip('aktiv', 'original')))))),
    ),
    el('h2', { style: 'margin-top:22px' }, 'Konnektortypen'),
    el('div.karte', el('dl', { style: 'display:grid;grid-template-columns:auto 1fr;gap:6px 14px;margin:0' },
      Object.entries(q.typen).flatMap(([t, b]) => [el('dt', el('code', t)), el('dd', b)]))),
    el('h2', { style: 'margin-top:22px' }, 'Betrieb'),
    el('div.karte',
      el('p', el('strong', 'Abruf und Indexbau: '), 'Das Werkzeug ', el('code', 'kompass'), ' (Rust) unter ', el('code', 'werkzeug/'),
        ' holt die Quellen, erkennt Änderungen über ETag und Hash, schreibt neue Fassungen nach ', el('code', 'daten/'),
        ' und baut die Suchindizes. Befehle: ', el('code', 'kompass abruf'), ', ', el('code', 'kompass index'), ', ', el('code', 'kompass pruefen'), '.'),
      el('p', el('strong', 'Warum nicht direkt EUR-Lex: '), 'Die Weboberfläche von EUR-Lex weist automatisierte Abrufe über eine WAF ab (HTTP 202, leerer Körper). Der amtliche Dienst CELLAR liefert dieselben Fassungen mit ETag und Last-Modified.'),
      el('p', el('strong', 'Level-2-Rechtsakte: '), 'Die zwölf Rechtsakte zu DORA wurden nicht von Hand gepflegt, sondern über den SPARQL-Dienst des Amts für Veröffentlichungen ermittelt (Rechtsgrundlage CELEX 32022R2554).'),
      el('p', { style: 'margin:0' }, el('strong', 'Diese Seite ist statisch: '), 'Sie liest nur JSON-Dateien. Der tägliche Lauf um 06:30 Europe/Berlin läuft ab Phase 5 als GitHub-Actions-Auftrag und schreibt die Daten ins Repository.')),
  );
}

function platzhalter(wurzel, name, phase, beschreibung) {
  titel(name);
  leere(wurzel).append(
    el('div.kopfzeile', el('div.wachs', el('h1', name),
      el('p.unterzeile', `Dieses Modul ist für Phase ${phase} vorgesehen und noch nicht gebaut.`))),
    el('div.karte',
      el('h2', 'Was hier entsteht'),
      el('p', beschreibung),
      el('div.hinweis.phase', el('div',
        'Der Platzhalter steht bewusst hier, statt das Modul halbfertig anzudeuten: was in der App zu sehen ist, soll auch belastbar sein.'))),
  );
}

// ------------------------------------------------------------------- Aufsetzen

function suchfeldEinrichten() {
  const form = document.getElementById('kopf-suche');
  const feld = document.getElementById('suchfeld');
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const q = feld.value.trim();
    if (q) location.hash = '#/suche/' + encodeURIComponent(q);
  });
  addEventListener('keydown', (e) => {
    if (e.key === '/' && document.activeElement.tagName !== 'INPUT' && document.activeElement.tagName !== 'TEXTAREA') {
      e.preventDefault();
      feld.focus();
    }
  });
}

function navKnopfEinrichten() {
  const k = document.getElementById('nav-knopf');
  k.addEventListener('click', () => {
    const offen = document.body.classList.toggle('nav-offen');
    k.setAttribute('aria-expanded', offen ? 'true' : 'false');
  });
}

function serviceWorker() {
  if (!('serviceWorker' in navigator) || location.protocol === 'file:') return;
  navigator.serviceWorker.register('sw.js').catch(() => { /* ohne Offline-Kopie läuft die App auch */ });
}

themaEinrichten();
suchfeldEinrichten();
navKnopfEinrichten();
document.getElementById('fuss-version').textContent = `Version ${APP_VERSION}`;
addEventListener('hashchange', leiten);
leiten();
serviceWorker();

// Für die Konsole: suche.proben() prüft den Gleichlauf der Stammbildung mit dem Werkzeug.
window.kompass = { suche, daten, nutzer, ton, APP_VERSION };
