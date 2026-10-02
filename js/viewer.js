// Strukturierter Viewer: Gliederungsbaum links, Text in der Mitte, Kontext rechts.
// Jede Fundstelle ist über einen stabilen Deep-Link erreichbar, z. B.
// #/rw/dora/art/28/abs/4 oder #/rw/dora/eg/47.

import { el, leere, chip, datum, zahl, kurz, titel, ton, tabelle, externURL, VERBINDLICHKEIT, MODUS, fehlerkarte } from './ui.js';
import * as daten from './daten.js';
import * as nutzer from './nutzer.js';
import * as analyse from './analyse.js';

export async function zeigen(wurzel, rwId, pfad) {
  const rw = await daten.regelwerk(rwId);
  if (!rw) {
    leere(wurzel).append(fehlerkarte(`Das Regelwerk „${rwId}" steht nicht im Katalog.`));
    return;
  }
  titel(rw.kurzname);

  const geladen = await daten.fassungLaden(rwId, nutzer.hol('sprache'));
  if (!geladen) {
    leere(wurzel).append(ohneVolltext(rw));
    return;
  }
  const { fassung, struktur, text, sprache } = geladen;
  const karten = daten.knotenkarten(struktur.knoten);
  const glossar = begriffeSammeln(struktur.knoten, text, sprache);
  const begriffeAn = nutzer.hol('begriffe') === true;

  leere(wurzel);
  const baum = el('nav.karte.baum', { 'aria-label': 'Gliederung' });
  const lesen = el('article.karte.lesen');
  const kontext = el('aside.karte.kontext', { 'aria-label': 'Kontext zur Fundstelle' });
  // Auf schmalen Schirmen steht immer nur einer der drei Bereiche - sonst muesste man
  // erst an 193 Gliederungseintraegen vorbeiscrollen, bevor der Text anfaengt.
  const flaeche = el('div.viewer', { dataset: { ansicht: 'text' } },
    baum, el('div.lesen-spalte', lesen), kontext);
  wurzel.append(
    el('div.kopfzeile',
      el('div.wachs',
        el('p.brotkrumen', el('a', { href: '#/bibliothek' }, 'Bibliothek'), ' › ', rw.kurzname),
        el('h1', rw.kurzname),
        el('p.unterzeile', rw.langtitel)),
    ),
    reiterleiste(flaeche),
    flaeche,
  );

  baumZeichnen(baum, struktur, sprache, pfad);
  zeichneText();

  function zeichneText() {
    const ziel = pfad ? karten.nachPfad.get(pfad) : null;
    if (pfad && !ziel) {
      leere(lesen).append(fehlerkarte(
        `Die Fundstelle „${pfad}" gibt es in dieser Fassung nicht.`,
        'Vielleicht ist sie nur in einer älteren Fassung enthalten – unter „Änderungen & Archiv" stehen alle übernommenen Fassungen.'));
      leere(kontext).append(...kontextTeile(rw, fassung, struktur, null, sprache));
      return;
    }
    leere(lesen);
    lesen.append(werkzeugzeile(rw, fassung, struktur, ziel, sprache, glossar, begriffeAn));

    if (rw.modus === 'zusammenfassung') lesen.append(zusammenfassungshinweis(rw, fassung));

    if (!ziel) {
      lesen.append(...uebersicht(rw, fassung, struktur, sprache, karten));
    } else {
      const { kopf, knoten } = wahl(ziel, karten);
      if (kopf) lesen.append(kopf);
      for (const k of knoten) lesen.append(...knotenZeichnen(k, text, sprache, rwId, karten));
      hervorhebenUndSpringen(lesen, ziel, glossar, begriffeAn);
      lesen.append(blaettern(ziel, karten, rwId, sprache));
    }
    leere(kontext).append(...kontextTeile(rw, fassung, struktur, ziel, sprache, karten, text));
  }
}

/** Reiter Text / Gliederung / Kontext. Im breiten Format blendet das CSS sie aus. */
function reiterleiste(flaeche) {
  const leiste = el('div.reiter', { role: 'tablist', 'aria-label': 'Ansicht' });
  const knoepfe = [['text', 'Text'], ['baum', 'Gliederung'], ['kontext', 'Kontext']].map(([wert, name]) => {
    const knopf = el('button', {
      type: 'button', role: 'tab', 'aria-selected': wert === 'text' ? 'true' : 'false',
    }, name);
    knopf.addEventListener('click', () => {
      flaeche.dataset.ansicht = wert;
      for (const k of knoepfe) k.setAttribute('aria-selected', k === knopf ? 'true' : 'false');
      scrollTo({ top: 0 });
    });
    return knopf;
  });
  leiste.append(...knoepfe);
  return leiste;
}

/**
 * Pflichthinweis für Werke ohne zulässigen Volltext. Er nennt Bezugsfassung,
 * Herkunft und Prüfstatus und verlinkt die Bezugsquelle - genau das verlangt der
 * Auftrag, und genau daran erkennt man beim Lesen, dass hier nicht der Normtext steht.
 */
function zusammenfassungshinweis(rw, fassung) {
  return el('div.hinweis.eigen', { style: 'margin-bottom:16px' },
    el('div',
      el('strong', 'Eigene Zusammenfassung – ersetzt nicht das Original. '),
      el('span', fassung.quelle.name),
      el('div', { style: 'margin-top:6px;display:flex;flex-wrap:wrap;gap:6px;align-items:center' },
        chip('Zusammenfassung', 'zusammenfassung'),
        chip(rw.geprueft ? 'Katalogangaben geprüft' : 'Katalogangaben ungeprüft', rw.geprueft ? '' : 'warn'),
        el('a', { href: externURL(rw.quelle.url), target: '_blank', rel: 'noopener' }, 'Zur Bezugsquelle ↗'))));
}

// ------------------------------------------------------------------ Auswahl

/** Welche Knoten werden für ein Ziel gezeigt? Absätze zeigen ihren ganzen Artikel. */
function wahl(ziel, karten) {
  if (ziel.art === 'absatz') {
    const vater = karten.eltern.get(ziel.id);
    return { kopf: null, knoten: [vater || ziel] };
  }
  if (['teil', 'kapitel', 'abschnitt', 'praeambel'].includes(ziel.art)) {
    const marke = ziel.bez || (ziel.art === 'praeambel' ? '' : `${daten.WORT[ziel.art][0]} ${ziel.nummer || ''}`);
    const kopf = el('div',
      el('p.kap', marke.trim()),
      el('h1', { style: 'margin-bottom:18px' }, (ziel.titel && (ziel.titel.de || ziel.titel.en)) || daten.WORT[ziel.art][0]));
    return { kopf, knoten: flachInhalt(ziel) };
  }
  return { kopf: null, knoten: [ziel] };
}

/** Artikel, Paragrafen bzw. Erwägungsgründe unterhalb eines Containers, in Reihenfolge. */
const TEXTARTEN = ['artikel', 'paragraf', 'control', 'anhang', 'erwaegungsgrund', 'bezugsvermerk'];

function flachInhalt(knoten) {
  const aus = [];
  (function gehe(liste) {
    for (const k of liste) {
      if (TEXTARTEN.includes(k.art)) aus.push(k);
      else if (k.kinder) gehe(k.kinder);
    }
  })(knoten.kinder || []);
  return aus;
}

// ------------------------------------------------------------------- Zeichnen

function knotenZeichnen(knoten, text, sprache, rwId, karten) {
  const aus = [];
  const bez = daten.bezeichnung(knoten, sprache);

  if (['artikel', 'paragraf', 'control'].includes(knoten.art)) {
    const kap = gliederungVon(knoten, karten);
    if (kap) aus.push(el('p.kap', kapitelBeschriftung(kap, sprache)));
    aus.push(el('h2.artikel', { dataset: { fs: knoten.id } },
      el('span', bez),
      el('a', {
        href: `#/rw/${rwId}/${knoten.pfad}`, style: 'float:right;font-size:.7rem;font-weight:400;text-decoration:none',
        title: 'Deep-Link auf diese Fundstelle', 'aria-label': `Deep-Link auf ${bez}`,
      }, '⧉ Link')));
  } else {
    aus.push(el('h2.artikel', { dataset: { fs: knoten.id } }, bez));
  }

  const koerper = el('div.text');
  const eigen = text[knoten.id];
  if (eigen) koerper.append(el('div.fundstelle', { dataset: { fs: knoten.id } }, ...bloecke(eigen.b)));
  for (const kind of knoten.kinder || []) {
    const t = text[kind.id];
    if (!t) continue;
    koerper.append(el('div.abs.fundstelle', { dataset: { fs: kind.id } },
      el('div.nr', el('a', {
        href: `#/rw/${rwId}/${kind.pfad}`, title: `Deep-Link auf ${daten.bezeichnung(kind, sprache)}`,
      }, `(${kind.nummer})`)),
      el('div', ...bloecke(t.b))));
  }
  aus.push(koerper);
  return aus;
}

function bloecke(liste) {
  return liste.map((b) => {
    if (b.art === 'p') return el('p', b.t);
    return punkte(b.p);
  });
}

function punkte(liste) {
  return el('ul.punkte', liste.map((p) => el('li',
    el('span.marke', p.m || '–'),
    el('div', p.t, p.u && p.u.length ? punkte(p.u) : null))));
}

/** Nächste Gliederungsebene über einer Fundstelle (Kapitel, Abschnitt, Teil). */
function gliederungVon(knoten, karten) {
  let k = karten.eltern.get(knoten.id);
  while (k && !['kapitel', 'abschnitt', 'teil'].includes(k.art)) k = karten.eltern.get(k.id);
  return k;
}

function kapitelBeschriftung(kap, sprache) {
  const t = kap.titel && (kap.titel[sprache] || kap.titel.de);
  const marke = kap.bez || `${daten.WORT[kap.art][sprache === 'en' ? 1 : 0]} ${kap.nummer || ''}`;
  return `${marke.trim()}${t ? ' · ' + t : ''}`;
}

function baumZeichnen(wurzel, struktur, sprache, pfad) {
  leere(wurzel);
  const liste = el('ul');
  for (const k of struktur.knoten) liste.append(baumEintrag(k, struktur.regelwerk, sprache, pfad));
  wurzel.append(el('p.gruppe', 'Gliederung'), liste);
}

function baumEintrag(knoten, rwId, sprache, pfad) {
  const bez = knotenKurz(knoten, sprache);
  const aktiv = pfad === knoten.pfad;
  const link = el('a', {
    href: `#/rw/${rwId}/${knoten.pfad || ''}`, 'aria-current': aktiv ? 'true' : null,
  }, bez);

  const kinder = (knoten.kinder || []).filter((k) => k.art !== 'absatz');
  if (!kinder.length) return el('li', link);

  const offen = aktiv || enthaelt(knoten, pfad);
  const d = el('details', { open: offen ? true : null },
    el('summary', link),
    el('ul', kinder.map((k) => baumEintrag(k, rwId, sprache, pfad))));
  return el('li', d);
}

function enthaelt(knoten, pfad) {
  if (!pfad) return false;
  return (knoten.kinder || []).some((k) => k.pfad === pfad || pfad.startsWith((k.pfad || '') + '/') || enthaelt(k, pfad));
}

function knotenKurz(knoten, sprache) {
  const w = daten.WORT[knoten.art] || [knoten.art];
  const wort = sprache === 'en' ? (w[1] || w[0]) : w[0];
  const t = knoten.titel && (knoten.titel[sprache] || knoten.titel.de);
  if (knoten.art === 'praeambel') return `${t || wort} (${(knoten.kinder || []).length})`;
  const kopf = knoten.bez || (knoten.nummer ? `${wort} ${knoten.nummer}` : null);
  if (kopf && t) return `${kopf} · ${t}`;
  if (kopf) return kopf;
  return t || wort;
}

// ------------------------------------------------------------------ Werkzeuge

function werkzeugzeile(rw, fassung, struktur, ziel, sprache, glossar, begriffeAn) {
  const zeile = el('div.werkzeugzeile');

  if (struktur.sprachen.length > 1) {
    const schalter = el('div.sprachschalter', { role: 'group', 'aria-label': 'Sprache des Originaltextes' });
    for (const s of struktur.sprachen) {
      schalter.append(el('button', {
        type: 'button', 'aria-pressed': s === sprache ? 'true' : 'false',
        onclick: () => { nutzer.setz('sprache', s); location.reload(); },
      }, s.toUpperCase()));
    }
    zeile.append(schalter);
  }

  if (ziel) {
    const gemerkt = nutzer.istGemerkt(rw.id, ziel.pfad);
    const merk = el('button.knopf', { type: 'button', 'aria-pressed': gemerkt ? 'true' : 'false' },
      gemerkt ? '★ gemerkt' : '☆ merken');
    merk.addEventListener('click', () => {
      const nun = nutzer.merken(rw.id, ziel.pfad, `${rw.kurzname} – ${daten.bezeichnung(ziel, sprache)}`);
      merk.setAttribute('aria-pressed', nun ? 'true' : 'false');
      merk.textContent = nun ? '★ gemerkt' : '☆ merken';
      ton(nun ? 'Lesezeichen gesetzt.' : 'Lesezeichen entfernt.');
    });
    zeile.append(merk);
    zeile.append(el('button.knopf', {
      type: 'button',
      onclick: () => zitatKopieren(rw, fassung, ziel, sprache),
    }, '⧉ Zitat kopieren'));
  }

  if (glossar.length) {
    const b = el('button.knopf', { type: 'button', 'aria-pressed': begriffeAn ? 'true' : 'false' },
      `Begriffe hervorheben (${glossar.length})`);
    b.addEventListener('click', () => {
      nutzer.setz('begriffe', !begriffeAn);
      location.reload();
    });
    zeile.append(b);
  }

  zeile.append(el('span.punktzahl', { style: 'margin-left:auto' },
    `Fassung ${datum(fassung.id)} · abgerufen ${datum(fassung.abgerufen)}`));
  return zeile;
}

async function zitatKopieren(rw, fassung, ziel, sprache) {
  const geladen = await daten.fassungLaden(rw.id, sprache);
  const teile = [];
  const sammle = (id) => {
    const t = geladen.text[id];
    if (t) teile.push(t.b.map((b) => b.art === 'p' ? b.t : b.p.map((p) => `${p.m} ${p.t}`).join('\n')).join('\n'));
  };
  sammle(ziel.id);
  for (const k of ziel.kinder || []) sammle(k.id);

  const url = location.href.split('#')[0] + `#/rw/${rw.id}/${ziel.pfad}`;
  const zitat = [
    `${rw.kurzname} – ${daten.bezeichnung(ziel, sprache)}`,
    '',
    teile.join('\n\n'),
    '',
    `Quelle: ${rw.langtitel}`,
    `${fassung.quelle.name}, Fassung vom ${datum(fassung.id)}, abgerufen am ${datum(fassung.abgerufen)}.`,
    'Rechtsverbindlich ist nur die amtlich veröffentlichte Fassung.',
    url,
  ].join('\n');
  try {
    await navigator.clipboard.writeText(zitat);
    ton('Zitat mit Quellenangabe kopiert.');
  } catch {
    ton('Kopieren hat der Browser abgelehnt.');
  }
}

// -------------------------------------------------------------------- Kontext

function kontextTeile(rw, fassung, struktur, ziel, sprache, karten, text) {
  const teile = [];
  const v = VERBINDLICHKEIT[rw.verbindlichkeit] || ['', rw.verbindlichkeit, ''];
  const m = MODUS[rw.modus] || ['', rw.modus, ''];

  if (ziel) {
    teile.push(el('h3', 'Fundstelle'),
      el('p', { style: 'margin:0 0 6px' }, el('strong', daten.bezeichnung(ziel, sprache))),
      el('p.punktzahl', { style: 'margin:0' }, `Kennung ${ziel.id} · Pfad ${ziel.pfad}`));
  }

  teile.push(el('h3', 'Einordnung'),
    el('div', { style: 'display:flex;flex-wrap:wrap;gap:6px' },
      chip(v[1], v[0], v[2]), chip(m[1], m[0], m[2]),
      !rw.geprueft ? chip('ungeprüft', 'warn') : null),
    el('dl',
      el('dt', 'Herausgeber'), el('dd', rw.herausgeber),
      el('dt', 'Typ'), el('dd', rw.typ),
      el('dt', 'Status'), el('dd', rw.status),
      rw.gueltigAb ? el('dt', 'Gültig ab') : null, rw.gueltigAb ? el('dd', datum(rw.gueltigAb)) : null,
      el('dt', 'Fassung'), el('dd', `${datum(fassung.id)} (${zahl(fassung.fundstellen)} Fundstellen)`),
      el('dt', 'Abgerufen'), el('dd', datum(fassung.abgerufen)),
    ));

  teile.push(el('h3', 'Quelle'),
    el('p', { style: 'margin:0 0 4px' }, fassung.quelle.name),
    el('p', { style: 'margin:0 0 6px' }, el('a', { href: externURL(rw.quelle.url), target: '_blank', rel: 'noopener' },
      rw.quelle.celex ? `CELEX ${rw.quelle.celex} ↗` : 'amtliche Quelle ↗')),
    el('div.hinweis.recht', { style: 'font-size:.8rem' }, fassung.quelle.hinweis));

  // Relevanz
  const relWahl = el('select', { 'aria-label': 'Relevanz dieses Regelwerks', style: 'width:100%' });
  for (const [w, t] of [['relevant', 'relevant'], ['referenz', 'Referenz'], ['nichtrelevant', 'nicht relevant']]) {
    relWahl.append(el('option', { value: w, selected: nutzer.relevanz(rw) === w ? true : null }, t));
  }
  relWahl.addEventListener('change', (e) => {
    nutzer.relevanzSetzen(rw.id, e.target.value);
    ton('Relevanz gespeichert – sie steuert Filter und Suche.');
  });
  teile.push(el('h3', 'Relevanz'), relWahl);

  // Konkretisierende Rechtsakte und Grundlage
  const bezugBox = el('div');
  teile.push(bezugBox);
  bezuegeNachladen(bezugBox, rw);

  if (rw.themen && rw.themen.length) {
    teile.push(el('h3', 'Themen'),
      el('div', { style: 'display:flex;flex-wrap:wrap;gap:6px' },
        rw.themen.map((t) => el('a.chip', { href: `#/themen/${t}`, style: 'text-decoration:none' }, t))));
  }

  const fussBox = el('div');
  teile.push(fussBox);
  fussnotenNachladen(fussBox, rw);

  if (ziel) {
    const id = ziel.id;
    const feld = el('textarea', {
      placeholder: 'Eigene Notiz zu dieser Fundstelle …', 'aria-label': 'Notiz zu dieser Fundstelle',
    });
    feld.value = nutzer.notiz(rw.id, id);
    let t = null;
    feld.addEventListener('input', () => {
      clearTimeout(t);
      t = setTimeout(() => nutzer.notizSetzen(rw.id, id, feld.value), 400);
    });
    teile.push(el('h3', 'Notiz'), feld,
      el('p.punktzahl', { style: 'margin:4px 0 0' }, 'Bleibt nur auf diesem Gerät.'));
  }

  const bezieBox = el('div');
  teile.push(bezieBox);
  beziehungenNachladen(bezieBox, rw, ziel);

  return teile;
}

async function bezuegeNachladen(ziel, rw) {
  const k = await daten.katalog();
  const kinder = k.regelwerke.filter((r) => r.konkretisiert === rw.id);
  const vater = rw.konkretisiert ? k.regelwerke.find((r) => r.id === rw.konkretisiert) : null;
  if (!kinder.length && !vater) return;
  leere(ziel);
  if (vater) {
    ziel.append(el('h3', 'Konkretisiert'),
      el('p', { style: 'margin:0' }, el('a', { href: `#/rw/${vater.id}` }, vater.kurzname)));
  }
  if (kinder.length) {
    ziel.append(el('h3', `Level-2-Rechtsakte (${kinder.length})`),
      el('ul', kinder.map((r) => el('li', el('a', { href: `#/rw/${r.id}` }, r.kurzname)))));
  }
}

/**
 * Beziehungen zur angezeigten Fundstelle: erst die redaktionellen (mit Begründung),
 * dann auf Wunsch die belegten Verweise. Letztere stehen in einer großen Datei und
 * werden deshalb nur auf Klick geholt.
 */
async function beziehungenNachladen(behaelter, rw, ziel) {
  const [b, k] = await Promise.all([analyse.beziehungen(), daten.katalog()]);
  const name = (id) => (k.regelwerke.find((r) => r.id === id) || {}).kurzname || id;
  const treffer = analyse.zuFundstelle(b.beziehungen, rw.id, ziel ? ziel.pfad : null);
  // Die Gegenstelle mit ihrer Bezeichnung zeigen, nicht mit ihrem Pfad: „APO12 – Risiken
  // steuern" sagt mehr als „o/apo12". Geladen wird dafür nur die Struktur, nicht der Text.
  const bez = await analyse.bezeichnungen(
    treffer.filter((t) => t.gegenueber.pfad).map((t) => t.gegenueber)).catch(() => new Map());
  const stelle = (g) => bez.get(`${g.rw}|${g.pfad}`) || g.pfad;

  leere(behaelter);
  behaelter.append(el('h3', `Beziehungen (${treffer.length})`));
  if (!treffer.length) {
    behaelter.append(el('p', { style: 'margin:0;color:var(--color-text-muted)' },
      ziel ? 'Zu dieser Fundstelle ist noch keine fachliche Beziehung erfasst.' : 'Für dieses Regelwerk ist noch keine Beziehung erfasst.'));
  }
  for (const t of treffer) {
    const g = t.gegenueber;
    behaelter.append(el('div', { style: 'padding:7px 0;border-bottom:1px solid var(--color-border)' },
      el('div', { style: 'display:flex;gap:5px;flex-wrap:wrap;margin-bottom:3px' },
        chip(analyse.TYP_NAME[t.typ] || t.typ, t.typ === 'spannungsfeld' ? 'warn' : ''),
        chip(t.konfidenz || '–'),
        t.geprueft ? chip('geprüft', 'original') : chip('ungeprüft', 'zusammenfassung')),
      el('p', { style: 'margin:0 0 3px' },
        el('a', { href: `#/rw/${g.rw}${g.pfad ? '/' + g.pfad : ''}` },
          `${name(g.rw)}${g.pfad ? ' · ' + stelle(g) : ''}`)),
      el('p', { style: 'margin:0;font-size:.82rem;color:var(--color-text-muted)' }, t.begruendung)));
  }

  const knopf = el('button.knopf', { type: 'button', style: 'margin-top:9px;width:100%' }, 'Belegte Verweise laden');
  knopf.addEventListener('click', async () => {
    knopf.disabled = true;
    knopf.textContent = 'lädt …';
    const v = await analyse.verweise();
    const vt = analyse.zuFundstelle(v.verweise, rw.id, ziel ? ziel.pfad : null);
    knopf.replaceWith(el('div',
      el('h3', `Belegte Verweise (${vt.length})`),
      vt.length
        ? el('div', vt.slice(0, 40).map((t) => el('p', { style: 'margin:0 0 7px;font-size:.82rem' },
            el('a', { href: `#/rw/${t.gegenueber.rw}/${t.gegenueber.pfad}` },
              `${t.richtung === 'von' ? '→ ' : '← '}${name(t.gegenueber.rw)} · ${t.gegenueber.pfad}`),
            t.beleg ? el('span', { style: 'display:block;color:var(--color-text-muted)' }, `„${t.beleg}"`) : null)))
        : el('p', { style: 'margin:0;color:var(--color-text-muted)' }, 'Keine.'),
      vt.length > 40 ? el('p.punktzahl', `Es werden 40 von ${vt.length} gezeigt.`) : null));
  });
  behaelter.append(knopf);
}

async function fussnotenNachladen(ziel, rw) {
  const f = await daten.fussnoten().catch(() => null);
  if (!f) return;
  const passend = (f.fussnoten || []).filter((x) =>
    (x.gilt_fuer.regelwerke || []).includes(rw.id) ||
    (x.gilt_fuer.themen || []).some((t) => (rw.themen || []).includes(t)));
  if (!passend.length) return;
  leere(ziel);
  ziel.append(el('h3', 'Fußnoten'),
    ...passend.map((x) => el('p', { style: 'margin:0 0 8px;font-size:.82rem' },
      el('strong', x.quelle + ': '), x.text, ' ',
      externURL(x.url) ? el('a', { href: externURL(x.url), target: '_blank', rel: 'noopener' }, '↗') : null)));
}

// ----------------------------------------------------------- Übersicht / Rest

function uebersicht(rw, fassung, struktur, sprache, karten) {
  const alle = [...karten.nachId.values()];
  const paragrafen = alle.filter((k) => k.art === 'paragraf');
  const artikel = alle.filter((k) => k.art === 'artikel');
  const eg = alle.filter((k) => k.art === 'erwaegungsgrund');
  const kern = paragrafen.length
    ? kennzahl(String(paragrafen.length), 'Paragrafen')
    : kennzahl(String(artikel.length), 'Artikel');
  return [
    el('div.gitter.drei', { style: 'margin-bottom:20px' },
      kern,
      eg.length ? kennzahl(String(eg.length), 'Erwägungsgründe') : null,
      kennzahl(zahl(fassung.fundstellen), 'Fundstellen mit Text')),
    el('h2', 'Inhalt'),
    tabelle(['Gliederung', 'Fundstellen'], struktur.knoten.map((k) => [
      el('a', { href: `#/rw/${rw.id}/${k.pfad}` }, knotenKurz(k, sprache)),
      String(flachInhalt(k).length || (TEXTARTEN.includes(k.art) ? 1 : 0))])),
    el('p.unterzeile', { style: 'margin-top:14px' },
      'Links wählt eine Fundstelle; jede hat einen eigenen Deep-Link.'),
  ];
}

function kennzahl(wert, beschriftung) {
  return el('div.karte', el('div.kennzahl-titel', beschriftung), el('div.kennzahl', wert));
}

function blaettern(ziel, karten, rwId, sprache) {
  // Ein Absatz blaettert in der Ebene seines Artikels bzw. Paragrafen weiter.
  const bezug = (ziel.art === 'absatz' ? karten.eltern.get(ziel.id) : ziel) || ziel;
  const gleiche = [...karten.nachId.values()].filter((k) => k.art === bezug.art);
  const i = gleiche.findIndex((k) => k.id === bezug.id);
  if (i < 0) return null;
  // Lange Paragrafenueberschriften muessen umbrechen duerfen, sonst zieht der Knopf
  // die ganze Seite breiter als den Schirm (gemessen: 394 px bei 375 px Fenster).
  const zeile = el('div.blaettern');
  if (i > 0) {
    zeile.append(el('a.knopf', { href: `#/rw/${rwId}/${gleiche[i - 1].pfad}` },
      '← ' + kurz(daten.bezeichnung(gleiche[i - 1], sprache), 52)));
  }
  if (i < gleiche.length - 1) {
    zeile.append(el('a.knopf.weiter', { href: `#/rw/${rwId}/${gleiche[i + 1].pfad}` },
      kurz(daten.bezeichnung(gleiche[i + 1], sprache), 52) + ' →'));
  }
  return zeile;
}

function ohneVolltext(rw) {
  return el('div',
    el('div.kopfzeile', el('div.wachs',
      el('p.brotkrumen', el('a', { href: '#/bibliothek' }, 'Bibliothek'), ' › ', rw.kurzname),
      el('h1', rw.kurzname), el('p.unterzeile', rw.langtitel))),
    el('div.karte',
      el('h2', 'Für dieses Regelwerk liegt noch kein Text vor'),
      el('p', rw.modus === 'original'
        ? 'Der Originaltext ist zulässig, der passende Konnektor ist aber noch nicht umgesetzt oder noch nicht aktiviert.'
        : 'Hier darf kein Volltext stehen, und eine eigene Zusammenfassung gibt es dafür noch nicht. Maßgeblich ist die Quelle.'),
      rw.hinweis ? el('div.hinweis.recht', rw.hinweis) : null,
      el('p', { style: 'margin-top:14px' },
        el('a.knopf.haupt', { href: externURL(rw.quelle.url), target: '_blank', rel: 'noopener' }, 'Zur amtlichen Quelle ↗'),
        rw.quelle.suchbegriff ? el('span.punktzahl', { style: 'margin-left:10px' }, `Suchbegriff dort: „${rw.quelle.suchbegriff}"`) : null)),
  );
}

// -------------------------------------------------------- Glossar / Zielmarke

/** Begriffsbestimmungen aus dem Definitionsartikel ziehen (DORA Art. 3: 65 Begriffe). */
function begriffeSammeln(knoten, text, sprache) {
  let def = null;
  (function gehe(liste) {
    for (const k of liste) {
      const t = (k.titel && (k.titel.de || k.titel.en) || '').toLowerCase();
      if (k.art === 'artikel' && (t.includes('begriffsbestimmung') || t === 'definitions')) def = k;
      if (k.kinder) gehe(k.kinder);
    }
  })(knoten);
  if (!def) return [];
  const quellen = [def.id, ...(def.kinder || []).map((k) => k.id)];
  const aus = [];
  for (const id of quellen) {
    const t = text[id];
    if (!t) continue;
    for (const b of t.b) {
      if (b.art !== 'liste') continue;
      for (const p of b.p) {
        const m = p.t.match(/^[„"»]([^"«"]{3,80})["«"]\s*(.+)$/);
        if (m) aus.push({ begriff: m[1], text: m[2], pfad: def.pfad, nummer: p.m });
      }
    }
  }
  return aus;
}

/** Zielmarke setzen, dorthin springen, optional Begriffe auszeichnen. */
function hervorhebenUndSpringen(wurzel, ziel, glossar, begriffeAn) {
  if (begriffeAn && glossar.length) begriffeAuszeichnen(wurzel, glossar);
  const marke = wurzel.querySelector(`[data-fs="${CSS.escape(ziel.id)}"]`);
  if (!marke) return;
  if (ziel.art === 'absatz') marke.classList.add('ziel');
  requestAnimationFrame(() => marke.scrollIntoView({ block: 'start', behavior: 'auto' }));
}

function begriffeAuszeichnen(wurzel, glossar) {
  const nachLaenge = [...glossar].sort((a, b) => b.begriff.length - a.begriff.length).slice(0, 120);
  const muster = new RegExp('(' + nachLaenge.map((g) => g.begriff.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|') + ')', 'g');
  const karte = new Map(nachLaenge.map((g) => [g.begriff, g]));
  const laeufer = document.createTreeWalker(wurzel, NodeFilter.SHOW_TEXT);
  const kandidaten = [];
  while (laeufer.nextNode()) {
    const n = laeufer.currentNode;
    if (n.parentElement.closest('h2, .marke, abbr')) continue;
    if (muster.test(n.nodeValue)) kandidaten.push(n);
    muster.lastIndex = 0;
  }
  for (const n of kandidaten) {
    const teile = n.nodeValue.split(muster);
    const ersatz = document.createDocumentFragment();
    teile.forEach((s, i) => {
      const g = i % 2 === 1 ? karte.get(s) : null;
      if (g) {
        ersatz.append(el('abbr', {
          title: `${g.begriff}: ${g.text}`,
          style: 'text-decoration:underline dotted var(--color-secondary);cursor:help',
        }, s));
      } else if (s) {
        ersatz.append(document.createTextNode(s));
      }
    });
    n.parentNode.replaceChild(ersatz, n);
  }
}
