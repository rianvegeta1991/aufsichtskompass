// Lernbereich: Lektionen, Quizzes mit sechs Fragearten, Fallstudie mit
// Entscheidungsbaum, Karteikarten mit verteilter Wiederholung, Cheat Sheets und
// eine druckbare Teilnahmebestätigung.
//
// Der Lernfortschritt liegt ausschließlich im Browser des Nutzers. Die Inhalte
// selbst sind Daten (daten/lernen/*.json) – neue Lektionen und Fragen brauchen
// keine Code-Änderung.

import { el, leere, chip, datum, zahl, kurz, titel, fehlerkarte, ton } from './ui.js';
import * as daten from './daten.js';
import * as nutzer from './nutzer.js';

const WURZEL = 'daten/lernen/';
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

const lektionen = () => hole('lektionen.json');
const quizzes = () => hole('quizzes.json');
const karten = () => hole('karten.json');
const fallstudien = () => hole('fallstudien.json');
const cheatsheets = () => hole('cheatsheets.json');

export async function zeigen(wurzel, was = '', id = '') {
  try {
    switch (was) {
      case '': return uebersicht(wurzel);
      case 'lektion': return lektion(wurzel, id);
      case 'quiz': return quiz(wurzel, id);
      case 'karten': return kartenlauf(wurzel);
      case 'fall': return fallstudie(wurzel, id);
      case 'blatt': return cheatsheet(wurzel, id);
      case 'bestaetigung': return bestaetigung(wurzel);
      default: leere(wurzel).append(fehlerkarte(`Den Lernbereich „${was}" gibt es nicht.`));
    }
  } catch (e) {
    console.error(e);
    leere(wurzel).append(fehlerkarte('Der Lernbereich ließ sich nicht aufbauen.', e.message));
  }
}

// ------------------------------------------------------------------ Übersicht

async function uebersicht(wurzel) {
  titel('Lernbereich');
  const [lek, qz, ka, fa, cs] = await Promise.all([lektionen(), quizzes(), karten(), fallstudien(), cheatsheets()]);
  const stand = nutzer.lernstand();
  leere(wurzel);

  const gelesen = lek.lektionen.filter((l) => stand.lektionen[l.id]).length;
  const bestanden = qz.quizzes.filter((q) => (stand.quizze[q.id]?.beste ?? 0) >= 0.7).length;
  const faellig = faelligeKarten(ka.karten, stand).length;

  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('h1', 'Lernbereich'),
      el('p.unterzeile', 'Lektionen, Quizzes, eine Fallstudie und Karteikarten – alle Inhalte sind an den Originalfundstellen verankert. Der Fortschritt bleibt auf diesem Gerät.'))),
    el('div.hinweis.eigen',
      el('div', el('strong', 'Eigene Lerninhalte, KI-erstellt und fachlich noch nicht abgenommen. '),
        'Sie ersetzen den Originaltext nicht – jede Erläuterung verlinkt die Fundstelle, an der sich die Antwort überprüfen lässt.')),
    el('div.gitter.drei', { style: 'margin:18px 0' },
      kennzahl(`${gelesen}/${lek.lektionen.length}`, 'Lektionen gelesen'),
      kennzahl(`${bestanden}/${qz.quizzes.length}`, 'Quizzes bestanden'),
      kennzahl(String(faellig), 'Karten heute fällig')),
  );

  // Lernpfad
  for (const pfad of lek.lernpfade || []) {
    wurzel.append(
      el('h2', { style: 'margin-top:8px' }, pfad.name),
      el('p.unterzeile', pfad.beschreibung),
      el('div.karte.lernpfad', pfad.schritte.map((s, i) => {
        const fertig = schrittFertig(s, stand);
        return el('a.pfadschritt', {
          href: schrittZiel(s), dataset: { fertig: fertig ? 'ja' : 'nein' },
        },
          el('span.pfadnummer', fertig ? '✓' : String(i + 1)),
          el('span', schrittName(s, lek, qz, fa)),
          el('span.chip', schrittArt(s)));
      })),
    );
  }

  wurzel.append(
    el('h2', { style: 'margin-top:22px' }, 'Lektionen'),
    el('div.gitter.zwei', lek.lektionen.map((l) => el('a.karte', { href: `#/lernen/lektion/${l.id}`, style: 'text-decoration:none;color:inherit' },
      el('div', { style: 'display:flex;gap:6px;flex-wrap:wrap;margin-bottom:6px' },
        chip(l.stufe), chip(`${l.dauer} Min.`), stand.lektionen[l.id] ? chip('gelesen', 'original') : null),
      el('h3', { style: 'margin:0 0 4px' }, l.titel),
      el('p', { style: 'margin:0;font-size:.88rem;color:var(--color-text-muted)' }, l.einleitung)))),

    el('h2', { style: 'margin-top:22px' }, 'Quizzes'),
    el('div.gitter.zwei', qz.quizzes.map((q) => {
      const e = stand.quizze[q.id];
      return el('a.karte', { href: `#/lernen/quiz/${q.id}`, style: 'text-decoration:none;color:inherit' },
        el('div', { style: 'display:flex;gap:6px;flex-wrap:wrap;margin-bottom:6px' },
          chip(q.stufe), chip(`${q.fragen.length} Fragen`),
          e ? chip(`bestes Ergebnis ${Math.round(e.beste * 100)} %`, e.beste >= 0.7 ? 'original' : 'zusammenfassung') : null),
        el('h3', { style: 'margin:0 0 4px' }, q.titel),
        el('p', { style: 'margin:0;font-size:.88rem;color:var(--color-text-muted)' },
          `Fragearten: ${[...new Set(q.fragen.map((f) => qz.arten[f.art] || f.art))].join(' · ')}`));
    })),

    el('h2', { style: 'margin-top:22px' }, 'Üben und nachschlagen'),
    el('div.gitter.drei',
      el('a.karte', { href: '#/lernen/karten', style: 'text-decoration:none;color:inherit' },
        el('h3', { style: 'margin:0 0 4px' }, 'Karteikarten'),
        el('p', { style: 'margin:0;font-size:.88rem;color:var(--color-text-muted)' },
          `${ka.karten.length} Karten mit verteilter Wiederholung – ${faellig} heute fällig.`)),
      ...fa.fallstudien.map((f) => el('a.karte', { href: `#/lernen/fall/${f.id}`, style: 'text-decoration:none;color:inherit' },
        el('h3', { style: 'margin:0 0 4px' }, f.titel),
        el('p', { style: 'margin:0;font-size:.88rem;color:var(--color-text-muted)' },
          `Fallstudie mit Entscheidungsbaum, ${f.dauer} Minuten.`),
        stand.faelle[f.id] ? chip('abgeschlossen', 'original') : null)),
      ...cs.cheatsheets.map((c) => el('a.karte', { href: `#/lernen/blatt/${c.id}`, style: 'text-decoration:none;color:inherit' },
        el('h3', { style: 'margin:0 0 4px' }, c.titel),
        el('p', { style: 'margin:0;font-size:.88rem;color:var(--color-text-muted)' }, c.untertitel)))),

    el('div.karte', { style: 'margin-top:22px' },
      el('h2', 'Teilnahmebestätigung'),
      el('p', 'Wenn Sie Lektionen und Quizzes absolviert haben, lässt sich daraus eine Bestätigung zum Ausdrucken oder Speichern als PDF erzeugen.'),
      el('a.knopf.haupt', { href: '#/lernen/bestaetigung' }, 'Bestätigung ansehen')),
  );
}

function kennzahl(wert, beschriftung) {
  return el('div.karte', el('div.kennzahl-titel', beschriftung), el('div.kennzahl', wert));
}

function schrittZiel(s) {
  return { lektion: `#/lernen/lektion/${s.id}`, quiz: `#/lernen/quiz/${s.id}`, fallstudie: `#/lernen/fall/${s.id}` }[s.art] || '#/lernen';
}

function schrittArt(s) {
  return { lektion: 'Lektion', quiz: 'Quiz', fallstudie: 'Fallstudie' }[s.art] || s.art;
}

function schrittName(s, lek, qz, fa) {
  if (s.art === 'lektion') return (lek.lektionen.find((l) => l.id === s.id) || {}).titel || s.id;
  if (s.art === 'quiz') return (qz.quizzes.find((q) => q.id === s.id) || {}).titel || s.id;
  return (fa.fallstudien.find((f) => f.id === s.id) || {}).titel || s.id;
}

function schrittFertig(s, stand) {
  if (s.art === 'lektion') return Boolean(stand.lektionen[s.id]);
  if (s.art === 'quiz') return (stand.quizze[s.id]?.beste ?? 0) >= 0.7;
  return Boolean(stand.faelle[s.id]);
}

// -------------------------------------------------------------------- Lektion

async function lektion(wurzel, id) {
  const d = await lektionen();
  const l = d.lektionen.find((x) => x.id === id);
  if (!l) { leere(wurzel).append(fehlerkarte(`Die Lektion „${id}" gibt es nicht.`)); return; }
  titel(l.titel);
  leere(wurzel);

  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('p.brotkrumen', el('a', { href: '#/lernen' }, 'Lernbereich'), ' › ', l.titel),
      el('h1', l.titel),
      el('p.unterzeile', l.einleitung))),
    el('div', { style: 'display:flex;gap:6px;flex-wrap:wrap;margin-bottom:16px' },
      chip(l.stufe), chip(`${l.dauer} Minuten`), ...(l.themen || []).map((t) => el('a.chip', { href: `#/themen/${t}`, style: 'text-decoration:none' }, t))),
  );

  const karten = await fundstellennamen(l.abschnitte.flatMap((a) => a.fundstellen || []));
  for (const a of l.abschnitte) {
    wurzel.append(el('div.karte', { style: 'margin-bottom:14px' },
      el('h2', a.titel),
      el('p', { style: 'margin:0 0 10px' }, a.text),
      (a.fundstellen || []).length
        ? el('div', { style: 'display:flex;flex-wrap:wrap;gap:6px;padding-top:8px;border-top:1px solid var(--color-border)' },
            el('span.punktzahl', { style: 'align-self:center' }, 'Fundstellen:'),
            a.fundstellen.map((f) => el('a.chip', { href: `#/rw/${f.rw}/${f.pfad}`, style: 'text-decoration:none' },
              karten.get(`${f.rw}|${f.pfad}`) || `${f.rw} ${f.pfad}`)))
        : null));
  }

  const gelesen = Boolean(nutzer.lernstand().lektionen[l.id]);
  const knopf = el('button.knopf' + (gelesen ? '' : '.haupt'), { type: 'button' },
    gelesen ? '✓ als gelesen vermerkt' : 'Als gelesen markieren');
  knopf.addEventListener('click', () => {
    const nun = nutzer.lektionUmschalten(l.id);
    knopf.textContent = nun ? '✓ als gelesen vermerkt' : 'Als gelesen markieren';
    knopf.className = 'knopf' + (nun ? '' : ' haupt');
    ton(nun ? 'Lektion als gelesen vermerkt.' : 'Vermerk zurückgenommen.');
  });
  wurzel.append(el('div', { style: 'display:flex;gap:10px;flex-wrap:wrap;margin-top:16px' },
    knopf,
    l.quiz ? el('a.knopf', { href: `#/lernen/quiz/${l.quiz}` }, 'Zum Quiz →') : null));
}

/** Lesbare Bezeichnungen für Fundstellen nachladen (nur Strukturen, kein Text). */
async function fundstellennamen(stellen) {
  const nachRw = new Map();
  for (const s of stellen) {
    if (!nachRw.has(s.rw)) nachRw.set(s.rw, []);
    nachRw.get(s.rw).push(s);
  }
  const aus = new Map();
  const k = await daten.katalog();
  await Promise.all([...nachRw.entries()].map(async ([rw, liste]) => {
    const kurzname = (k.regelwerke.find((r) => r.id === rw) || {}).kurzname || rw;
    const f = await daten.aktuelleFassung(rw).catch(() => null);
    if (!f) { for (const s of liste) aus.set(`${rw}|${s.pfad}`, `${kurzname} ${s.pfad}`); return; }
    const st = await daten.struktur(rw, f.id).catch(() => null);
    if (!st) return;
    const karten = daten.knotenkarten(st.knoten);
    for (const s of liste) {
      const knoten = karten.nachPfad.get(s.pfad);
      if (!knoten) { aus.set(`${rw}|${s.pfad}`, `${kurzname} ${s.pfad}`); continue; }
      const vater = knoten.art === 'absatz' ? karten.eltern.get(knoten.id) : null;
      const bez = vater
        ? `${daten.bezeichnung(vater, 'de').split(' – ')[0]} ${daten.bezeichnung(knoten, 'de')}`
        : daten.bezeichnung(knoten, 'de');
      aus.set(`${rw}|${s.pfad}`, `${kurzname}: ${kurz(bez, 60)}`);
    }
  }));
  return aus;
}

// ----------------------------------------------------------------------- Quiz

async function quiz(wurzel, id) {
  const d = await quizzes();
  const q = d.quizzes.find((x) => x.id === id);
  if (!q) { leere(wurzel).append(fehlerkarte(`Das Quiz „${id}" gibt es nicht.`)); return; }
  titel(q.titel);
  leere(wurzel);

  let nr = 0;
  const antworten = [];
  const namen = await fundstellennamen(q.fragen.map((f) => f.fundstelle).filter(Boolean));

  const fortschritt = el('div.quiz-fortschritt');
  const buehne = el('div');
  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('p.brotkrumen', el('a', { href: '#/lernen' }, 'Lernbereich'), ' › ', q.titel),
      el('h1', q.titel),
      el('p.unterzeile', `${q.fragen.length} Fragen · ${q.stufe}`))),
    fortschritt, buehne,
  );

  function balken() {
    leere(fortschritt).append(
      ...q.fragen.map((_, i) => el('span.quiz-punkt', {
        dataset: { zustand: i < antworten.length ? (antworten[i] ? 'richtig' : 'falsch') : (i === nr ? 'aktuell' : 'offen') },
        title: `Frage ${i + 1}`,
      })),
      el('span.punktzahl', { style: 'margin-left:auto' }, `${Math.min(nr + 1, q.fragen.length)} von ${q.fragen.length}`));
  }

  function frageZeigen() {
    balken();
    const f = q.fragen[nr];
    leere(buehne);
    const feld = el('div.karte.quiz-frage');
    const kopf = el('div',
      el('div', { style: 'display:flex;gap:6px;flex-wrap:wrap;margin-bottom:8px' },
        chip(`Frage ${nr + 1}`), chip(d.arten[f.art] || f.art)),
      el('h2', { style: 'margin:0 0 12px' }, f.frage || f.aussage));
    feld.append(kopf);

    const eingabe = eingabefeld(f);
    feld.append(eingabe.knoten);

    const pruefen = el('button.knopf.haupt', { type: 'button' }, 'Antwort prüfen');
    const rueck = el('div');
    pruefen.addEventListener('click', () => {
      const richtig = eingabe.pruefen();
      antworten[nr] = richtig;
      pruefen.disabled = true;
      eingabe.sperren();
      balken();
      const stelle = f.fundstelle;
      leere(rueck).append(el('div.quiz-rueck', { dataset: { richtig: richtig ? 'ja' : 'nein' } },
        el('p', { style: 'margin:0 0 6px;font-weight:600' }, richtig ? '✓ Richtig' : '✕ Nicht richtig'),
        eingabe.loesungstext ? el('p', { style: 'margin:0 0 6px' }, el('strong', 'Richtig wäre: '), eingabe.loesungstext()) : null,
        el('p', { style: 'margin:0 0 8px' }, f.erlaeuterung),
        stelle ? el('a.chip', { href: `#/rw/${stelle.rw}/${stelle.pfad}`, style: 'text-decoration:none' },
          namen.get(`${stelle.rw}|${stelle.pfad}`) || `${stelle.rw} ${stelle.pfad}`) : null));
      weiter.style.display = '';
      weiter.focus();
    });

    const weiter = el('button.knopf', { type: 'button', style: 'display:none' },
      nr + 1 < q.fragen.length ? 'Nächste Frage →' : 'Ergebnis ansehen');
    weiter.addEventListener('click', () => {
      nr++;
      if (nr < q.fragen.length) frageZeigen();
      else ergebnis();
    });

    feld.append(el('div', { style: 'display:flex;gap:10px;flex-wrap:wrap;margin-top:14px' }, pruefen, weiter), rueck);
    buehne.append(feld);
  }

  function ergebnis() {
    const richtig = antworten.filter(Boolean).length;
    const quote = richtig / q.fragen.length;
    nutzer.quizErgebnis(q.id, richtig, q.fragen.length);
    leere(fortschritt);
    leere(buehne).append(el('div.karte',
      el('h2', quote >= 0.7 ? 'Bestanden' : 'Noch nicht bestanden'),
      el('p.kennzahl', `${richtig} von ${q.fragen.length}`),
      el('p', quote >= 0.7
        ? 'Ab 70 Prozent gilt das Quiz als bestanden. Das Ergebnis ist gespeichert – auf diesem Gerät.'
        : 'Ab 70 Prozent gilt das Quiz als bestanden. Sehen Sie sich die Erläuterungen an und versuchen Sie es erneut.'),
      el('div', { style: 'display:flex;gap:10px;flex-wrap:wrap;margin-top:12px' },
        el('button.knopf.haupt', { type: 'button', onclick: () => { nr = 0; antworten.length = 0; frageZeigen(); } }, 'Noch einmal'),
        q.lektion ? el('a.knopf', { href: `#/lernen/lektion/${q.lektion}` }, 'Zur Lektion') : null,
        el('a.knopf', { href: '#/lernen' }, 'Zur Übersicht'),
        quote >= 0.7 ? el('a.knopf', { href: '#/lernen/bestaetigung' }, 'Teilnahmebestätigung') : null)));
  }

  frageZeigen();
}

/** Baut das Eingabefeld je Frageart und liefert Prüf- und Sperrfunktion. */
function eingabefeld(f) {
  if (f.art === 'mc' || f.art === 'mehrfach') {
    const mehrfach = f.art === 'mehrfach';
    const name = 'frage-' + Math.random().toString(36).slice(2);
    const eingaben = f.optionen.map((text, i) => {
      const feld = el('input', { type: mehrfach ? 'checkbox' : 'radio', name, value: String(i), id: `${name}-${i}` });
      return { feld, knoten: el('label.quiz-option', { for: `${name}-${i}` }, feld, el('span', text)) };
    });
    return {
      knoten: el('div.quiz-optionen', eingaben.map((e) => e.knoten)),
      pruefen: () => {
        const gewaehlt = eingaben.map((e, i) => (e.feld.checked ? i : -1)).filter((i) => i >= 0);
        const soll = mehrfach ? f.loesung : [f.loesung];
        eingaben.forEach((e, i) => {
          if (soll.includes(i)) e.knoten.dataset.los = 'richtig';
          else if (gewaehlt.includes(i)) e.knoten.dataset.los = 'falsch';
        });
        return gewaehlt.length === soll.length && gewaehlt.every((i) => soll.includes(i));
      },
      sperren: () => eingaben.forEach((e) => { e.feld.disabled = true; }),
    };
  }

  if (f.art === 'wahrfalsch') {
    let gewaehlt = null;
    const knoepfe = [['Wahr', true], ['Falsch', false]].map(([text, wert]) => {
      const b = el('button.knopf', { type: 'button' }, text);
      b.addEventListener('click', () => {
        gewaehlt = wert;
        for (const x of knoepfe) x.setAttribute('aria-pressed', 'false');
        b.setAttribute('aria-pressed', 'true');
      });
      return b;
    });
    return {
      knoten: el('div', { style: 'display:flex;gap:10px' }, knoepfe),
      pruefen: () => gewaehlt === f.loesung,
      sperren: () => knoepfe.forEach((b) => { b.disabled = true; }),
      loesungstext: () => (f.loesung ? 'Wahr' : 'Falsch'),
    };
  }

  if (f.art === 'luecke') {
    const feld = el('input', { type: 'text', style: 'width:100%;max-width:340px;padding:10px', 'aria-label': 'Antwort' });
    return {
      knoten: el('div', el('p', { style: 'margin:0 0 10px' }, f.text.replace('___', '  ______  ')), feld),
      pruefen: () => {
        const w = feld.value.trim().toLowerCase();
        return f.loesung.some((l) => w === l.toLowerCase() || (w.length > 3 && l.toLowerCase().startsWith(w)));
      },
      sperren: () => { feld.disabled = true; },
      loesungstext: () => f.loesung[0],
    };
  }

  if (f.art === 'zuordnung') {
    const rechts = f.paare.map((p) => p[1]);
    const gemischt = mischen([...rechts]);
    const felder = f.paare.map(([links], i) => {
      const s = el('select', { 'aria-label': `Zuordnung für ${links}` }, el('option', { value: '' }, 'bitte wählen'),
        gemischt.map((r) => el('option', { value: r }, r)));
      return { links, s, soll: rechts[i] };
    });
    return {
      knoten: el('div.quiz-zuordnung', felder.map((x) => el('div.zuordnungszeile',
        el('span', x.links), el('span', '→'), x.s))),
      pruefen: () => felder.every((x) => x.s.value === x.soll),
      sperren: () => felder.forEach((x) => { x.s.disabled = true; }),
      loesungstext: () => felder.map((x) => `${x.links} → ${x.soll}`).join('; '),
    };
  }

  if (f.art === 'reihenfolge') {
    const reihen = mischen(f.elemente.map((text, i) => ({ text, i })));
    const liste = el('ol.quiz-reihenfolge');
    const zeichnen = () => {
      leere(liste);
      reihen.forEach((e, pos) => {
        liste.append(el('li',
          el('span', e.text),
          el('span', { style: 'margin-left:auto;display:flex;gap:4px' },
            el('button.knopf', {
              type: 'button', 'aria-label': 'nach oben', disabled: pos === 0 ? true : null,
              onclick: () => { [reihen[pos - 1], reihen[pos]] = [reihen[pos], reihen[pos - 1]]; zeichnen(); },
            }, '↑'),
            el('button.knopf', {
              type: 'button', 'aria-label': 'nach unten', disabled: pos === reihen.length - 1 ? true : null,
              onclick: () => { [reihen[pos + 1], reihen[pos]] = [reihen[pos], reihen[pos + 1]]; zeichnen(); },
            }, '↓'))));
      });
    };
    zeichnen();
    return {
      knoten: liste,
      pruefen: () => reihen.every((e, pos) => f.loesung[pos] === e.i),
      sperren: () => liste.querySelectorAll('button').forEach((b) => { b.disabled = true; }),
      loesungstext: () => f.loesung.map((i) => f.elemente[i]).join(' → '),
    };
  }

  return { knoten: el('p', 'Diese Frageart kennt die App noch nicht.'), pruefen: () => false, sperren: () => {} };
}

function mischen(liste) {
  const a = [...liste];
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

// ------------------------------------------------------------------ Fallstudie

async function fallstudie(wurzel, id) {
  const d = await fallstudien();
  const f = d.fallstudien.find((x) => x.id === id);
  if (!f) { leere(wurzel).append(fehlerkarte(`Die Fallstudie „${id}" gibt es nicht.`)); return; }
  titel(f.titel);
  leere(wurzel);

  const gewaehlt = [];
  const buehne = el('div');
  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('p.brotkrumen', el('a', { href: '#/lernen' }, 'Lernbereich'), ' › ', f.titel),
      el('h1', f.titel),
      el('p.unterzeile', `${f.stufe} · etwa ${f.dauer} Minuten`))),
    el('div.karte', { style: 'margin-bottom:14px' }, el('h2', 'Die Lage'), el('p', { style: 'margin:0' }, f.lage)),
    buehne,
  );

  const namen = await fundstellennamen(f.schritte.flatMap((s) => [
    ...(s.optionen || []).map((o) => o.fundstelle).filter(Boolean),
    ...(s.fundstellen || []),
  ]));

  function schritt(sid) {
    const s = f.schritte.find((x) => x.id === sid);
    if (!s) return;
    if (s.abschluss) {
      nutzer.fallAbschluss(f.id, gewaehlt);
      const gut = gewaehlt.filter((b) => b === 'gut').length;
      buehne.append(el('div.karte',
        el('h2', 'Musterlösung'),
        el('p', s.musterloesung),
        el('p', { style: 'margin-top:10px' },
          el('strong', `Ihre Entscheidungen: ${gut} von ${gewaehlt.length} tragfähig.`)),
        el('div', { style: 'display:flex;flex-wrap:wrap;gap:6px;margin:8px 0' },
          (s.fundstellen || []).map((q) => el('a.chip', { href: `#/rw/${q.rw}/${q.pfad}`, style: 'text-decoration:none' },
            namen.get(`${q.rw}|${q.pfad}`) || `${q.rw} ${q.pfad}`))),
        el('div', { style: 'display:flex;gap:10px;flex-wrap:wrap;margin-top:12px' },
          el('button.knopf.haupt', { type: 'button', onclick: () => { gewaehlt.length = 0; leere(buehne); schritt(f.schritte[0].id); } }, 'Noch einmal'),
          el('a.knopf', { href: '#/lernen' }, 'Zur Übersicht'))));
      buehne.lastChild.scrollIntoView({ behavior: 'smooth', block: 'start' });
      return;
    }

    const feld = el('div.karte', { style: 'margin-bottom:14px' }, el('h2', s.frage));
    const rueck = el('div');
    const knoepfe = s.optionen.map((o) => {
      const b = el('button.knopf.fallknopf', { type: 'button' }, o.text);
      b.addEventListener('click', () => {
        gewaehlt.push(o.bewertung);
        for (const x of knoepfe) x.disabled = true;
        b.dataset.bewertung = o.bewertung;
        leere(rueck).append(el('div.quiz-rueck', { dataset: { richtig: o.bewertung === 'gut' ? 'ja' : 'nein' } },
          el('p', { style: 'margin:0 0 6px' }, o.rueckmeldung),
          o.fundstelle ? el('a.chip', { href: `#/rw/${o.fundstelle.rw}/${o.fundstelle.pfad}`, style: 'text-decoration:none' },
            namen.get(`${o.fundstelle.rw}|${o.fundstelle.pfad}`) || `${o.fundstelle.rw} ${o.fundstelle.pfad}`) : null,
          el('div', { style: 'margin-top:10px' },
            el('button.knopf.haupt', { type: 'button', onclick: () => schritt(o.folge) }, 'Weiter →'))));
      });
      return b;
    });
    feld.append(el('div.falloptionen', { style: 'display:flex;flex-direction:column;gap:8px' }, knoepfe), rueck);
    buehne.append(feld);
    feld.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  schritt(f.schritte[0].id);
}

// ---------------------------------------------------------------- Karteikarten

function faelligeKarten(alle, stand) {
  const jetzt = Date.now();
  return alle.filter((k) => {
    const z = stand.karten[k.id];
    if (!z) return true;
    return new Date(z.faellig).getTime() <= jetzt;
  });
}

async function kartenlauf(wurzel) {
  titel('Karteikarten');
  const d = await karten();
  leere(wurzel);
  const stand = nutzer.lernstand();
  const stapel = faelligeKarten(d.karten, stand);

  wurzel.append(el('div.kopfzeile', el('div.wachs',
    el('p.brotkrumen', el('a', { href: '#/lernen' }, 'Lernbereich'), ' › Karteikarten'),
    el('h1', 'Karteikarten'),
    el('p.unterzeile', `${d.karten.length} Karten insgesamt, ${stapel.length} heute fällig. Abstände: ${d.intervalle.join(', ')} Tage.`))));

  if (!stapel.length) {
    wurzel.append(el('div.karte',
      el('h2', 'Für heute erledigt'),
      el('p', 'Alle fälligen Karten sind durchgearbeitet. Die nächsten werden nach Plan wieder vorgelegt.'),
      el('a.knopf', { href: '#/lernen' }, 'Zur Übersicht')));
    return;
  }

  let i = 0;
  const buehne = el('div');
  wurzel.append(buehne);

  function zeigen() {
    if (i >= stapel.length) {
      leere(buehne).append(el('div.karte',
        el('h2', 'Durchgearbeitet'),
        el('p', `${stapel.length} Karten wiederholt. Der nächste Termin steht je Karte fest.`),
        el('a.knopf.haupt', { href: '#/lernen' }, 'Zur Übersicht')));
      return;
    }
    const k = stapel[i];
    leere(buehne);
    const rueckseite = el('div', { style: 'display:none' },
      el('p', { style: 'margin:14px 0 8px' }, k.hinten),
      k.fundstelle ? el('a.chip', { href: `#/rw/${k.fundstelle.rw}/${k.fundstelle.pfad}`, style: 'text-decoration:none' },
        `${k.fundstelle.rw} · ${k.fundstelle.pfad}`) : null,
      el('div', { style: 'display:flex;gap:10px;flex-wrap:wrap;margin-top:14px' },
        el('button.knopf.haupt', { type: 'button', onclick: () => { nutzer.karteAntwort(k.id, true, d.intervalle); i++; zeigen(); } }, '✓ gewusst'),
        el('button.knopf', { type: 'button', onclick: () => { nutzer.karteAntwort(k.id, false, d.intervalle); i++; zeigen(); } }, '✕ noch nicht')));

    const aufdecken = el('button.knopf', { type: 'button' }, 'Antwort zeigen');
    aufdecken.addEventListener('click', () => { rueckseite.style.display = ''; aufdecken.style.display = 'none'; });

    buehne.append(el('div.karte.karteikarte',
      el('p.punktzahl', { style: 'margin:0 0 6px' }, `Karte ${i + 1} von ${stapel.length}`),
      el('h2', { style: 'margin:0 0 10px' }, k.vorne),
      aufdecken, rueckseite));
  }
  zeigen();
}

// ----------------------------------------------------------------- Cheat Sheet

async function cheatsheet(wurzel, id) {
  const d = await cheatsheets();
  const c = d.cheatsheets.find((x) => x.id === id);
  if (!c) { leere(wurzel).append(fehlerkarte(`Das Blatt „${id}" gibt es nicht.`)); return; }
  titel(c.titel);
  leere(wurzel);
  wurzel.append(
    el('div.kopfzeile',
      el('div.wachs',
        el('p.brotkrumen', el('a', { href: '#/lernen' }, 'Lernbereich'), ' › ', c.titel),
        el('h1', c.titel),
        el('p.unterzeile', c.untertitel)),
      el('button.knopf', { type: 'button', onclick: () => print() }, '🖶 Drucken / PDF')),
    el('div.gitter.zwei.blatt', c.bloecke.map((b) => el('div.karte',
      el('h2', b.titel),
      el('ul', { style: 'margin:0;padding-left:1.1em' }, b.punkte.map((p) => el('li', { style: 'margin:6px 0' },
        p.text,
        p.rw ? el('a.punktzahl', { href: `#/rw/${p.rw}${p.pfad ? '/' + p.pfad : ''}`, style: 'margin-left:6px' },
          `${p.rw} ${p.pfad || ''}`) : null)))))),
    el('div.hinweis.eigen', { style: 'margin-top:14px' },
      el('div', 'Eigene Kurzfassung – sie ersetzt den Originaltext nicht. Rechtsverbindlich ist nur die amtlich veröffentlichte Fassung.')),
  );
}

// ---------------------------------------------------------- Teilnahmebestätigung

async function bestaetigung(wurzel) {
  titel('Teilnahmebestätigung');
  const [lek, qz, fa] = await Promise.all([lektionen(), quizzes(), fallstudien()]);
  const stand = nutzer.lernstand();
  leere(wurzel);

  const gelesene = lek.lektionen.filter((l) => stand.lektionen[l.id]);
  const bestandene = qz.quizzes.filter((q) => (stand.quizze[q.id]?.beste ?? 0) >= 0.7);
  const faelle = fa.fallstudien.filter((f) => stand.faelle[f.id]);

  const nameFeld = el('input', {
    type: 'text', placeholder: 'Ihr Name', value: nutzer.lernname(), style: 'padding:9px;min-width:240px',
    'aria-label': 'Name für die Bestätigung',
    oninput: (e) => { nutzer.lernnameSetzen(e.target.value); urkundeName.textContent = e.target.value || '—'; },
  });
  const urkundeName = el('p.urkunde-name', nutzer.lernname() || '—');

  wurzel.append(
    el('div.kopfzeile', el('div.wachs',
      el('p.brotkrumen', el('a', { href: '#/lernen' }, 'Lernbereich'), ' › Teilnahmebestätigung'),
      el('h1', 'Teilnahmebestätigung'),
      el('p.unterzeile', 'Die Bestätigung entsteht aus Ihrem Lernfortschritt auf diesem Gerät. Zum Speichern als PDF den Druckdialog verwenden und „Als PDF sichern" wählen.'))),
    el('div.filterleiste', { class: 'nicht-drucken' }, nameFeld,
      el('button.knopf.haupt', { type: 'button', onclick: () => print() }, '🖶 Drucken / als PDF sichern')),
  );

  if (!gelesene.length && !bestandene.length) {
    wurzel.append(el('div.karte', el('p', { style: 'margin:0' },
      'Noch nichts abgeschlossen. Lesen Sie eine Lektion oder absolvieren Sie ein Quiz – danach steht hier Ihre Bestätigung.')));
    return;
  }

  wurzel.append(el('div.karte.urkunde',
    el('p.urkunde-kopf', 'Teilnahmebestätigung'),
    urkundeName,
    el('p', { style: 'text-align:center;margin:0 0 18px' },
      'hat im Aufsichtskompass die folgenden Lerninhalte zu IT-Compliance und IT-Governance bearbeitet:'),
    gelesene.length ? el('div',
      el('h3', 'Lektionen'),
      el('ul', gelesene.map((l) => el('li', `${l.titel} (${l.stufe}, ${l.dauer} Min.)`)))) : null,
    bestandene.length ? el('div',
      el('h3', 'Quizzes'),
      el('ul', bestandene.map((q) => el('li',
        `${q.titel} – ${Math.round(stand.quizze[q.id].beste * 100)} % richtig, ${stand.quizze[q.id].versuche.length} Versuch(e)`)))) : null,
    faelle.length ? el('div',
      el('h3', 'Fallstudien'),
      el('ul', faelle.map((f) => el('li', f.titel)))) : null,
    el('p.urkunde-fuss',
      `Ausgestellt am ${datum(new Date().toISOString())}. `,
      'Der Aufsichtskompass ist eine private Lernanwendung; diese Bestätigung ist kein Zertifikat und kein Nachweis gegenüber Dritten. ',
      'Die Lerninhalte sind KI-erstellt und fachlich nicht abgenommen.'),
  ));
}
