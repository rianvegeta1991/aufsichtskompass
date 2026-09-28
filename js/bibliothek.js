// Regelwerks-Bibliothek: Katalog mit Filtern. Der Katalog selbst ist eine Datei
// (daten/regelwerke.json) – ein neues Regelwerk erscheint hier ohne Code-Änderung.

import { el, leere, chip, datum, zahl, titel, VERBINDLICHKEIT, MODUS, TIEFE, kurz } from './ui.js';
import * as daten from './daten.js';
import * as nutzer from './nutzer.js';

const GRUPPEN = {
  dora: 'DORA',
  'dora-level2': 'DORA – Level-2-Rechtsakte (RTS/ITS)',
  'aufsicht-de': 'Aufsichtspraxis Deutschland (BaFin)',
  'recht-de': 'Deutsches Recht',
  'eu-recht': 'EU-Recht',
  'eu-aufsicht': 'Europäische Aufsicht und Notenbanken',
  framework: 'Frameworks, Normen und Kontrollkataloge',
  pruefungsstandard: 'Prüfungsstandards',
  verband: 'Verbandsregelungen',
};

const filter = { text: '', gruppe: '', typ: '', herausgeber: '', status: '', verbindlichkeit: '', modus: '', relevanz: '', thema: '' };

export async function zeigen(wurzel) {
  titel('Regelwerks-Bibliothek');
  const [k, th] = await Promise.all([daten.katalog(), daten.themen()]);
  const bestand = await Promise.all(k.regelwerke.map(async (r) => ({ ...r, _fassung: await daten.aktuelleFassung(r.id) })));

  leere(wurzel);
  wurzel.append(
    el('div.kopfzeile',
      el('div.wachs',
        el('h1', 'Regelwerks-Bibliothek'),
        el('p.unterzeile', `${bestand.length} Regelwerke im Katalog, ${bestand.filter((r) => r._fassung).length} davon mit abgerufenem Volltext. Stand des Katalogs: ${datum(k.stand)}.`)),
    ),
    el('div.hinweis.recht',
      el('div', el('strong', 'Prüfstatus: '),
        'Einträge mit ', el('span.chip.warn', 'ungeprüft'),
        ' sind noch nicht gegen die amtliche Quelle abgeglichen. Die zwölf Level-2-Rechtsakte zu DORA stammen aus einer SPARQL-Abfrage des CELLAR-Dienstes und tragen den amtlichen Titel.')),
  );

  const leiste = el('div.filterleiste');
  const treffer = el('div');
  // Auf dem Handy stehen acht Auswahlfelder sonst als Wand vor dem Ergebnis: dort
  // steckt die Leiste in einem zugeklappten Block, auf breiten Schirmen ist sie offen.
  const zahlAnzeige = el('span.chip', '0');
  const box = el('details.filter-box', { open: innerWidth > 900 ? true : null },
    el('summary',
      el('span', 'Filter'),
      el('span', { style: 'margin-left:auto;display:flex;gap:6px;align-items:center' }, zahlAnzeige, el('span', 'Treffer'))),
    leiste);
  wurzel.append(el('div', { style: 'margin-top:18px' }, box, treffer));

  const werte = (feld) => [...new Set(bestand.map((r) => r[feld]).filter(Boolean))].sort((a, b) => a.localeCompare(b, 'de'));

  const auswahl = (feld, beschriftung, liste, karte) => {
    const s = el('select', { 'aria-label': beschriftung, onchange: (e) => { filter[feld] = e.target.value; zeichne(); } },
      el('option', { value: '' }, beschriftung));
    for (const w of liste) s.append(el('option', { value: w }, karte ? karte(w) : w));
    return s;
  };

  leiste.append(
    el('input', {
      type: 'search', placeholder: 'Im Katalog filtern …', 'aria-label': 'Katalog nach Text filtern',
      oninput: (e) => { filter.text = e.target.value.toLowerCase(); zeichne(); },
    }),
    auswahl('gruppe', 'Alle Bereiche', Object.keys(GRUPPEN).filter((g) => bestand.some((r) => r.gruppe === g)), (g) => GRUPPEN[g]),
    auswahl('typ', 'Alle Typen', werte('typ')),
    auswahl('herausgeber', 'Alle Herausgeber', werte('herausgeber')),
    auswahl('verbindlichkeit', 'Jede Verbindlichkeit', werte('verbindlichkeit'), (v) => (VERBINDLICHKEIT[v] || [null, v])[1]),
    auswahl('modus', 'Beide Darstellungen', werte('modus'), (m) => (MODUS[m] || [null, m])[1]),
    auswahl('status', 'Jeder Status', werte('status')),
    auswahl('relevanz', 'Jede Relevanz', ['relevant', 'referenz', 'nichtrelevant'],
      (r) => ({ relevant: 'relevant', referenz: 'Referenz', nichtrelevant: 'nicht relevant' }[r])),
    auswahl('thema', 'Alle Themen', th.themen.map((t) => t.id), (id) => (th.themen.find((t) => t.id === id) || {}).name),
  );

  function passt(r) {
    if (filter.gruppe && r.gruppe !== filter.gruppe) return false;
    if (filter.typ && r.typ !== filter.typ) return false;
    if (filter.herausgeber && r.herausgeber !== filter.herausgeber) return false;
    if (filter.status && r.status !== filter.status) return false;
    if (filter.verbindlichkeit && r.verbindlichkeit !== filter.verbindlichkeit) return false;
    if (filter.modus && r.modus !== filter.modus) return false;
    if (filter.relevanz && nutzer.relevanz(r) !== filter.relevanz) return false;
    if (filter.thema && !(r.themen || []).includes(filter.thema)) return false;
    if (filter.text) {
      const heu = `${r.kurzname} ${r.langtitel} ${r.herausgeber} ${r.typ} ${(r.quelle && r.quelle.celex) || ''}`.toLowerCase();
      if (!heu.includes(filter.text)) return false;
    }
    return true;
  }

  function zeichne() {
    leere(treffer);
    const liste = bestand.filter(passt);
    if (!liste.length) {
      zahlAnzeige.textContent = '0';
      treffer.append(el('p.leer', 'Kein Regelwerk passt zu dieser Auswahl.'));
      return;
    }
    treffer.append(el('p.unterzeile', { style: 'margin:0 0 12px' }, `${liste.length} von ${bestand.length} Regelwerken`));
    zahlAnzeige.textContent = String(liste.length);
    for (const g of Object.keys(GRUPPEN)) {
      const teil = liste.filter((r) => r.gruppe === g);
      if (!teil.length) continue;
      treffer.append(
        el('h2', { style: 'margin:22px 0 10px' }, GRUPPEN[g], ' ', el('span.chip', String(teil.length))),
        el('div.gitter.zwei', teil.map((r) => karte(r, th))),
      );
    }
  }

  zeichne();
}

function karte(r, th) {
  const v = VERBINDLICHKEIT[r.verbindlichkeit] || ['', r.verbindlichkeit, ''];
  const m = MODUS[r.modus] || ['', r.modus, ''];
  const rel = nutzer.relevanz(r);
  const ziel = r._fassung ? `#/rw/${r.id}` : null;

  const relWahl = el('select', {
    'aria-label': `Relevanz von ${r.kurzname}`, style: 'font-size:.76rem;padding:2px 6px',
    onchange: (e) => {
      nutzer.relevanzSetzen(r.id, e.target.value === (r.relevanz || 'relevant') ? '' : e.target.value);
      e.target.closest('.karte').dataset.relevanz = e.target.value;
    },
  });
  for (const [w, t] of [['relevant', 'relevant'], ['referenz', 'Referenz'], ['nichtrelevant', 'nicht relevant']]) {
    relWahl.append(el('option', { value: w, selected: rel === w ? true : null }, t));
  }

  return el('div.karte.rw-karte', { dataset: { relevanz: rel } },
    el('div.reihe',
      chip(v[1], v[0], v[2]),
      chip(m[1], m[0], m[2]),
      chip(TIEFE[r.tiefe] || r.tiefe),
      r.status !== 'in Kraft' ? chip(r.status, 'warn') : null,
      !r.geprueft ? chip('ungeprüft', 'warn', 'Noch nicht gegen die amtliche Quelle abgeglichen') : null,
    ),
    el('h3', ziel ? el('a', { href: ziel }, r.kurzname) : r.kurzname),
    el('p.lang', kurz(r.langtitel, 180)),
    r.hinweis ? el('p.lang', { style: 'font-style:italic' }, kurz(r.hinweis, 200)) : null,
    el('div.reihe', (r.themen || []).slice(0, 5).map((t) => {
      const name = (th.themen.find((x) => x.id === t) || {}).name || t;
      return el('a.chip', { href: `#/themen/${t}`, style: 'text-decoration:none' }, name);
    })),
    el('div.fuss',
      el('span', r.herausgeber),
      r._fassung ? el('span', `Fassung ${datum(r._fassung.id)} · ${zahl(r._fassung.fundstellen)} Fundstellen`) : el('span', 'noch kein Volltext'),
      r.quelle && r.quelle.url ? el('a', { href: r.quelle.url, target: '_blank', rel: 'noopener' }, 'amtliche Quelle ↗') : null,
      el('span', { style: 'margin-left:auto' }, relWahl),
    ),
  );
}
