// Interdependenzen: Mapping-Matrix, Heatmap, Meldepflichten, Rollen, Graph, Zeitstrahl.
//
// Grundsatz der Darstellung: In jeder Zelle steht die Zahl oder ein Zeichen - Farbe ist
// immer nur Zugabe, nie der einzige Träger der Aussage. Die Heatmap arbeitet deshalb mit
// einer Helligkeitsstufung eines Farbtons statt mit einem Ampelverlauf.

import { el, leere, chip, datum, zahl, kurz, titel, tabelle, exportknoepfe, fehlerkarte } from './ui.js';
import * as daten from './daten.js';
import * as analyse from './analyse.js';
import * as nutzer from './nutzer.js';

const ANSICHTEN = [
  ['', 'Mapping-Matrix'],
  ['heatmap', 'Heatmap Thema × Regelwerk'],
  ['meldepflichten', 'Meldepflichten'],
  ['rollen', 'Rollen & Three Lines'],
  ['graph', 'Graph'],
  ['zeitstrahl', 'Zeitstrahl'],
];

export async function zeigen(wurzel, ansicht = '') {
  titel('Interdependenzen');
  leere(wurzel);
  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('h1', 'Interdependenzen'),
      el('p.unterzeile', 'Wie die Vorgaben zusammenhängen: fachliche Beziehungen mit Begründung, dazu die Verweise, die in den Texten selbst stehen.'))),
    el('div.reiter.reiter-breit', { role: 'tablist' },
      ANSICHTEN.map(([pfad, name]) => el('a', {
        href: '#/matrizen' + (pfad ? '/' + pfad : ''),
        role: 'tab',
        'aria-selected': ansicht === pfad ? 'true' : 'false',
      }, name))),
  );
  const raum = el('div');
  wurzel.append(raum);

  try {
    switch (ansicht) {
      case '': await mappingMatrix(raum); break;
      case 'heatmap': await heatmap(raum); break;
      case 'meldepflichten': await meldeMatrix(raum); break;
      case 'rollen': await rollenMatrix(raum); break;
      case 'graph': await graph(raum); break;
      case 'zeitstrahl': await zeitstrahl(raum); break;
      default: raum.append(fehlerkarte(`Die Ansicht „${ansicht}" gibt es nicht.`));
    }
  } catch (e) {
    console.error(e);
    raum.append(fehlerkarte('Die Ansicht ließ sich nicht aufbauen.', e.message));
  }
}

// --------------------------------------------------------------- Mapping-Matrix

async function mappingMatrix(raum) {
  const [b, k] = await Promise.all([analyse.beziehungen(), daten.katalog()]);
  const name = (id) => (k.regelwerke.find((r) => r.id === id) || {}).kurzname || id;

  let mitVerweisen = false;
  let liste = b.beziehungen.slice();
  const inhalt = el('div');
  const schalter = el('button.knopf', { type: 'button' }, 'Belegte Verweise dazunehmen');
  schalter.addEventListener('click', async () => {
    schalter.disabled = true;
    schalter.textContent = 'lädt …';
    if (!mitVerweisen) {
      const v = await analyse.verweise();
      liste = b.beziehungen.concat(v.verweise.filter((x) => x.von.rw !== x.nach.rw));
      mitVerweisen = true;
      schalter.textContent = 'Nur fachliche Beziehungen';
    } else {
      liste = b.beziehungen.slice();
      mitVerweisen = false;
      schalter.textContent = 'Belegte Verweise dazunehmen';
    }
    schalter.disabled = false;
    zeichne();
  });

  raum.append(
    el('div.hinweis.recht', { style: 'margin:14px 0' },
      el('div',
        el('strong', 'Zwei Quellen: '),
        'Die fachlichen Beziehungen sind redaktionell gepflegt und tragen je eine Begründung, eine Konfidenz und einen Prüfstatus. ',
        'Die belegten Verweise stammen aus dem Wortlaut der Texte selbst – das Werkzeug hat sie gefunden, jede trägt ihre Fundstelle mit.')),
    el('div.filterleiste', schalter),
    inhalt,
  );

  function zeichne() {
    leere(inhalt);
    const paare = analyse.paare(liste);
    const beteiligt = [...new Set(liste.flatMap((x) => [x.von.rw, x.nach.rw]))]
      .sort((a, z) => name(a).localeCompare(name(z), 'de'));

    if (!beteiligt.length) {
      inhalt.append(el('p.leer', 'Noch keine Beziehungen erfasst.'));
      return;
    }

    const kopf = el('tr', el('th', { scope: 'col' }, 'Regelwerk'),
      beteiligt.map((r) => el('th', { scope: 'col', class: 'schraeg' }, el('span', name(r)))));
    const zeilen = beteiligt.map((a) => el('tr',
      el('th', { scope: 'row' }, el('a', { href: `#/rw/${a}` }, name(a))),
      beteiligt.map((z) => {
        if (a === z) return el('td.diagonal', { 'aria-hidden': 'true' }, '');
        const e = paare.get([a, z].sort().join('|'));
        if (!e) return el('td', '');
        const zeichen = [...e.typen].map((t) => analyse.TYP_ZEICHEN[t] || '·').join('');
        const zelle = el('td.zelle',
          el('button', {
            type: 'button',
            title: `${e.anzahl} Beziehung(en): ${[...e.typen].map((t) => analyse.TYP_NAME[t] || t).join(', ')}`,
            onclick: () => detail(inhalt, name(a), name(z), e.eintraege, k),
          }, el('span.zahl', String(e.anzahl)), el('span.zeichen', zeichen)));
        return zelle;
      })));

    inhalt.append(
      el('div.karte', { style: 'overflow:auto;padding:12px' },
        el('table.matrix', el('thead', kopf), el('tbody', zeilen))),
      el('p.unterzeile', { style: 'margin:10px 0' },
        'Zeichen in den Zellen: ',
        ...Object.entries(analyse.TYP_ZEICHEN).map(([t, z]) =>
          el('span', { style: 'margin-right:12px;white-space:nowrap' }, el('strong', z), ' ', analyse.TYP_NAME[t]))),
      exportknoepfe('mapping-matrix',
        ['von', 'nach', 'Typ', 'Begründung', 'Quelle', 'Konfidenz', 'geprüft'],
        () => liste.map((x) => [
          `${name(x.von.rw)} ${x.von.pfad || ''}`.trim(),
          `${name(x.nach.rw)} ${x.nach.pfad || ''}`.trim(),
          analyse.TYP_NAME[x.typ] || x.typ,
          x.begruendung || x.beleg || '',
          x.quelle || '',
          x.konfidenz || '',
          x.geprueft ? 'ja' : 'nein',
        ])),
      el('div', { id: 'matrix-detail' }),
    );
  }

  zeichne();
}

function detail(wurzel, vonName, nachName, eintraege, katalog) {
  const ziel = wurzel.querySelector('#matrix-detail');
  if (!ziel) return;
  const name = (id) => (katalog.regelwerke.find((r) => r.id === id) || {}).kurzname || id;
  leere(ziel).append(
    el('h2', { style: 'margin-top:22px' }, `${vonName} ↔ ${nachName}`, ' ', chip(`${eintraege.length}`)),
    el('div.karte', eintraege.map((b) => el('div', { style: 'padding:10px 0;border-bottom:1px solid var(--color-border)' },
      el('p', { style: 'margin:0 0 4px' },
        el('a', { href: `#/rw/${b.von.rw}${b.von.pfad ? '/' + b.von.pfad : ''}` }, `${name(b.von.rw)}${b.von.pfad ? ' · ' + b.von.pfad : ''}`),
        ' ', el('strong', analyse.TYP_ZEICHEN[b.typ] || '→'), ' ',
        el('a', { href: `#/rw/${b.nach.rw}${b.nach.pfad ? '/' + b.nach.pfad : ''}` }, `${name(b.nach.rw)}${b.nach.pfad ? ' · ' + b.nach.pfad : ''}`)),
      el('div', { style: 'display:flex;gap:6px;flex-wrap:wrap;margin-bottom:5px' },
        chip(analyse.TYP_NAME[b.typ] || b.typ, b.typ === 'spannungsfeld' ? 'warn' : ''),
        b.konfidenz ? chip('Konfidenz: ' + b.konfidenz) : null,
        chip(b.quelle || 'automatisch'),
        b.geprueft ? chip('geprüft', 'original') : chip('ungeprüft', 'zusammenfassung')),
      el('p', { style: 'margin:0;font-size:.9rem' }, b.begruendung || (b.beleg ? `Beleg: „${b.beleg}"` : ''))))),
  );
  ziel.scrollIntoView({ behavior: 'smooth', block: 'start' });
}

// --------------------------------------------------------------------- Heatmap

async function heatmap(raum) {
  const [th, k, z] = await Promise.all([daten.themen(), daten.katalog(), analyse.themenzuordnung()]);
  // Standardmäßig nur die als relevant eingestuften Regelwerke: mit allen 52 wird die
  // Tabelle so breit, dass man nichts mehr erkennt.
  let alleZeigen = false;
  const auswahl = () => k.regelwerke.filter((r) =>
    (r.themen || []).length && (alleZeigen || nutzer.relevanz(r) === 'relevant'));
  const schalter = el('label', { style: 'display:flex;align-items:center;gap:8px;min-height:44px' },
    el('input', {
      type: 'checkbox',
      onchange: (e) => { alleZeigen = e.target.checked; zeichne(); },
    }),
    el('span', 'Auch Referenz-Regelwerke zeigen'));
  const inhalt = el('div');
  const rw = auswahl();
  // Wert je Zelle: Fundstellen aus der Themenzuordnung, sonst 0; das Regelwerk selbst
  // zählt über seine Katalogthemen als "grobe" Abdeckung.
  const wert = (themaId, rwId) => {
    const fein = (z.zuordnungen || []).filter((e) => e.rw === rwId && (e.themen || []).includes(themaId)).length;
    const grob = (k.regelwerke.find((r) => r.id === rwId)?.themen || []).includes(themaId) ? 1 : 0;
    return { fein, grob };
  };
  const max = Math.max(1, ...th.themen.flatMap((t) => rw.map((r) => wert(t.id, r.id).fein)));

  raum.append(
    el('div.hinweis.recht', { style: 'margin:14px 0' },
      el('div', el('strong', 'Lesart: '),
        'Die Zahl ist die Anzahl einzeln zugeordneter Fundstellen. Ein Punkt heißt: Das Regelwerk berührt das Thema laut Katalog, die Zuordnung auf Fundstellenebene steht noch aus. ',
        'Die Färbung stuft nur die Helligkeit eines Farbtons – die Zahl trägt die Aussage.')),
    el('div.filterleiste', schalter),
    inhalt,
  );

  function zeichne() {
    const sichtbar = auswahl();
    const kopfZ = el('tr', el('th', { scope: 'col' }, 'Thema'),
      sichtbar.map((r) => el('th', { scope: 'col', class: 'schraeg' }, el('span', r.kurzname))));
    const zeilenZ = th.themen.map((t) => el('tr',
      el('th', { scope: 'row' }, el('a', { href: `#/themen/${t.id}` }, t.name)),
      sichtbar.map((r) => {
        const { fein, grob } = wert(t.id, r.id);
        if (!fein && !grob) return el('td', '');
        const stufe = fein ? Math.min(4, Math.ceil((fein / max) * 4)) : 0;
        return el('td.hitze', {
          dataset: { stufe: String(stufe) },
          title: fein
            ? `${r.kurzname} – ${t.name}: ${fein} zugeordnete Fundstelle(n)`
            : `${r.kurzname} berührt das Thema ${t.name} (Katalogzuordnung, noch keine einzelnen Fundstellen)`,
        }, fein ? String(fein) : '·');
      })));
    leere(inhalt).append(
      el('p.unterzeile', { style: 'margin:0 0 10px' },
        `${th.themen.length} Themen × ${sichtbar.length} Regelwerke`),
      el('div.karte', { style: 'overflow:auto;padding:12px' },
        el('table.matrix.hitzetabelle', el('thead', kopfZ), el('tbody', zeilenZ))),
      exportknoepfe('heatmap-thema-regelwerk',
        ['Thema', ...sichtbar.map((r) => r.kurzname)],
        () => th.themen.map((t) => [t.name, ...sichtbar.map((r) => {
          const { fein, grob } = wert(t.id, r.id);
          return fein || (grob ? 'berührt' : '');
        })])),
    );
  }

  zeichne();
}

// -------------------------------------------------------------- Meldepflichten

async function meldeMatrix(raum) {
  const m = await analyse.meldepflichten();
  const k = await daten.katalog();
  if (!m) {
    raum.append(fehlerkarte('Die Meldepflichten-Matrix ist noch nicht hinterlegt.'));
    return;
  }
  const name = (id) => (k.regelwerke.find((r) => r.id === id) || {}).kurzname || id;

  const kopf = el('tr', el('th', { scope: 'col' }, 'Merkmal'),
    m.spalten.map((s) => el('th', { scope: 'col' },
      el('div', el('a', { href: `#/rw/${s.rw}` }, s.name)),
      el('div', { style: 'font-weight:400;text-transform:none;letter-spacing:0;margin-top:3px' }, chip(s.gilt)))));

  const zeilen = m.zeilen.map((z) => el('tr',
    el('th', { scope: 'row' }, z.merkmal),
    m.spalten.map((s) => {
      const zelle = z.zellen[s.id];
      if (!zelle) return el('td', '–');
      return el('td',
        el('div', zelle.text),
        el('a.punktzahl', {
          href: `#/rw/${zelle.rw}${zelle.pfad ? '/' + zelle.pfad : ''}`,
          style: 'display:inline-block;margin-top:5px',
        }, `${name(zelle.rw)} · ${zelle.pfad || 'ganzes Regelwerk'} ↗`));
    })));

  raum.append(
    el('div.hinweis.recht', { style: 'margin:14px 0' }, el('div', m.hinweis)),
    el('div.karte', { style: 'overflow:auto;padding:12px' },
      el('table.matrix.textmatrix', el('thead', kopf), el('tbody', zeilen))),
    exportknoepfe('meldepflichten',
      ['Merkmal', ...m.spalten.map((s) => s.name)],
      () => m.zeilen.map((z) => [z.merkmal, ...m.spalten.map((s) => {
        const c = z.zellen[s.id];
        return c ? `${c.text} [${name(c.rw)} ${c.pfad || ''}]` : '';
      })])),
  );
}

// ----------------------------------------------------------------- Rollenmatrix

async function rollenMatrix(raum) {
  const r = await analyse.rollen();
  const k = await daten.katalog();
  if (!r) {
    raum.append(fehlerkarte('Die Rollenmatrix ist noch nicht hinterlegt.'));
    return;
  }
  const name = (id) => (k.regelwerke.find((x) => x.id === id) || {}).kurzname || id;

  const kopf = el('tr', el('th', { scope: 'col' }, 'Anforderung'),
    r.rollen.map((x) => el('th', { scope: 'col', class: 'schraeg' },
      el('span', { title: x.kurz }, x.name))));
  const zeilen = r.anforderungen.map((a) => el('tr',
    el('th', { scope: 'row' },
      el('div', a.name),
      el('a.punktzahl', { href: `#/rw/${a.rw}/${a.pfad}` }, `${name(a.rw)} · ${a.pfad} ↗`)),
    r.rollen.map((x) => {
      const w = a.raci[x.id];
      return el('td.raci', { dataset: { raci: w || '' }, title: w ? r.legende[w] : '' }, w || '');
    })));

  const linien = ['1', '2', '3', 'Aufsicht'];
  raum.append(
    el('div.hinweis.recht', { style: 'margin:14px 0' }, el('div', r.hinweis)),
    el('h2', { style: 'margin-top:18px' }, 'Drei Verteidigungslinien'),
    el('div.gitter.drei', linien.map((l) => el('div.karte',
      el('div.kennzahl-titel', l === 'Aufsicht' ? 'Aufsicht und Letztverantwortung' : `${l}. Linie`),
      el('ul', { style: 'margin:8px 0 0;padding-left:1.1em' },
        r.rollen.filter((x) => x.linie === l).map((x) => el('li', { style: 'margin:4px 0' },
          el('strong', x.name), el('div', { style: 'font-size:.84rem;color:var(--color-text-muted)' }, x.kurz))))))),
    el('h2', { style: 'margin-top:22px' }, 'Wer macht was'),
    el('div.karte', { style: 'overflow:auto;padding:12px' },
      el('table.matrix.raci-tabelle', el('thead', kopf), el('tbody', zeilen))),
    el('p.unterzeile', { style: 'margin:10px 0' },
      ...Object.entries(r.legende).map(([w, t]) => el('span', { style: 'margin-right:14px' }, el('strong', w), ' = ', t))),
    exportknoepfe('rollenmatrix',
      ['Anforderung', 'Fundstelle', ...r.rollen.map((x) => x.name)],
      () => r.anforderungen.map((a) => [a.name, `${name(a.rw)} ${a.pfad}`, ...r.rollen.map((x) => a.raci[x.id] || '')])),
  );
}

// ----------------------------------------------------------------------- Graph

async function graph(raum) {
  const [b, k] = await Promise.all([analyse.beziehungen(), daten.katalog()]);
  const name = (id) => (k.regelwerke.find((r) => r.id === id) || {}).kurzname || id;
  const paare = analyse.paare(b.beziehungen);
  const knoten = [...new Set(b.beziehungen.flatMap((x) => [x.von.rw, x.nach.rw]))];
  if (!knoten.length) {
    raum.append(el('p.leer', 'Noch keine Beziehungen erfasst.'));
    return;
  }

  // Radiale Anordnung: DORA in die Mitte (dort laufen die meisten Kanten zusammen),
  // alles Übrige gleichmäßig auf einen Kreis.
  const mitte = knoten.includes('dora') ? 'dora' : knoten[0];
  const aussen = knoten.filter((x) => x !== mitte);
  const B = 900, H = 620, cx = B / 2, cy = H / 2, r = Math.min(cx, cy) - 110;
  const pos = new Map([[mitte, [cx, cy]]]);
  aussen.forEach((id, i) => {
    const w = (i / aussen.length) * 2 * Math.PI - Math.PI / 2;
    pos.set(id, [cx + r * Math.cos(w), cy + r * Math.sin(w)]);
  });

  const svgNS = 'http://www.w3.org/2000/svg';
  const mk = (tag, attrs, ...kinder) => {
    const n = document.createElementNS(svgNS, tag);
    for (const [a, v] of Object.entries(attrs || {})) if (v !== null) n.setAttribute(a, v);
    n.append(...kinder.filter(Boolean));
    return n;
  };

  const svg = mk('svg', {
    viewBox: `0 0 ${B} ${H}`, role: 'img',
    'aria-label': `Netzwerk aus ${knoten.length} Regelwerken und ${paare.size} Beziehungspaaren`,
    style: 'width:100%;height:auto;max-height:70vh',
  });

  for (const [schluessel, e] of paare) {
    const [a, z] = schluessel.split('|');
    if (!pos.has(a) || !pos.has(z)) continue;
    const [x1, y1] = pos.get(a);
    const [x2, y2] = pos.get(z);
    const spannung = e.typen.has('spannungsfeld');
    const speziell = e.typen.has('lex-specialis');
    svg.append(mk('line', {
      x1, y1, x2, y2,
      stroke: spannung ? 'var(--color-danger)' : speziell ? 'var(--color-primary)' : 'var(--color-secondary)',
      'stroke-width': Math.min(6, 1 + e.anzahl * 0.8),
      'stroke-dasharray': spannung ? '6 4' : null,
      opacity: '.65',
    }, mk('title', {}, `${name(a)} ↔ ${name(z)}: ${e.anzahl}`)));
  }

  for (const [id, [x, y]] of pos) {
    const grad = [...paare.entries()].filter(([s]) => s.split('|').includes(id))
      .reduce((n, [, e]) => n + e.anzahl, 0);
    const rad = id === mitte ? 34 : 10 + Math.min(14, grad * 1.6);
    const g = mk('g', { style: 'cursor:pointer' });
    g.append(
      mk('circle', {
        cx: x, cy: y, r: rad,
        fill: id === mitte ? 'var(--color-primary)' : 'var(--color-surface)',
        stroke: id === mitte ? 'var(--color-primary)' : 'var(--color-border-strong)',
        'stroke-width': 2,
      }, mk('title', {}, `${name(id)}: ${grad} Beziehungen`)),
      mk('text', {
        x, y: y + rad + 15, 'text-anchor': 'middle',
        fill: 'var(--color-text)', 'font-size': '13', 'font-family': 'var(--schrift)',
      }, document.createTextNode(kurz(name(id), 26))),
      id === mitte ? mk('text', {
        x, y: y + 5, 'text-anchor': 'middle',
        fill: 'var(--color-primary-contrast)', 'font-size': '14', 'font-weight': '700',
        'font-family': 'var(--schrift)',
      }, document.createTextNode(String(grad))) : null,
    );
    g.addEventListener('click', () => { location.hash = `#/rw/${id}`; });
    svg.append(g);
  }

  raum.append(
    el('div.hinweis.recht', { style: 'margin:14px 0' },
      el('div', el('strong', 'Lesart: '),
        'Linienstärke = Anzahl der Beziehungen. Gestrichelt und dunkelrot = Spannungsfeld, karminrot = lex specialis. ',
        'Ein Klick auf einen Knoten öffnet das Regelwerk. Der Graph zeigt die fachlichen Beziehungen, nicht die belegten Verweise.')),
    el('div.karte', svg),
    el('div', { style: 'margin-top:12px' },
      tabelle(['Regelwerk', 'Beziehungen'],
        knoten.map((id) => [
          el('a', { href: `#/rw/${id}` }, name(id)),
          String([...paare.entries()].filter(([s]) => s.split('|').includes(id)).reduce((n, [, e]) => n + e.anzahl, 0)),
        ]).sort((a, z) => Number(z[1]) - Number(a[1])))),
  );
}

// ------------------------------------------------------------------- Zeitstrahl

async function zeitstrahl(raum) {
  const [k, b] = await Promise.all([daten.katalog(), daten.bestand()]);
  const punkte = [];
  for (const r of k.regelwerke) {
    if (r.gueltigAb) {
      punkte.push({ zeit: r.gueltigAb, rw: r.id, name: r.kurzname, was: 'gilt seit', art: 'recht' });
    }
    const e = b.bestand && b.bestand[r.id];
    if (e) {
      punkte.push({ zeit: e.fassung.id, rw: r.id, name: r.kurzname, was: 'Fassung im Bestand', art: 'fassung' });
    }
  }
  punkte.sort((a, z) => String(z.zeit).localeCompare(String(a.zeit)));

  const jahre = [...new Set(punkte.map((p) => String(p.zeit).slice(0, 4)))];
  raum.append(
    el('div.hinweis.recht', { style: 'margin:14px 0' },
      el('div', el('strong', 'Zwei Sorten Einträge: '),
        '„gilt seit" stammt aus dem Katalog und ist dort teils noch ungeprüft. „Fassung im Bestand" ist der Stand der Fassung, die die App abgerufen hat – nicht das Datum der Rechtsänderung.')),
    el('div.karte.zeitstrahl',
      jahre.map((j) => el('div.jahr',
        el('div.jahr-marke', j),
        el('div.jahr-liste', punkte.filter((p) => String(p.zeit).startsWith(j)).map((p) => el('div.zeitpunkt',
          el('span.chip' + (p.art === 'recht' ? '.bindend' : ''), p.was),
          el('a', { href: `#/rw/${p.rw}` }, p.name),
          el('span.punktzahl', datum(p.zeit))))))),
    ),
    exportknoepfe('zeitstrahl', ['Datum', 'Regelwerk', 'Ereignis'],
      () => punkte.map((p) => [p.zeit, p.name, p.was])),
  );
}
