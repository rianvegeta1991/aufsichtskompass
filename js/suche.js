// Volltextsuche im Browser. Der Index wird vom Werkzeug (Rust) vorgebaut, hier läuft
// nur die Bewertung: BM25 über die Stämme.
//
// ACHTUNG – Gleichlauf: stamm() unten muss Wort für Wort dasselbe Ergebnis liefern wie
// `stamm()` in werkzeug/src/suche.rs. Ändert sich eine Regel dort, muss sie hier mit
// geändert werden, sonst findet die Suche indexierte Wörter nicht mehr. Die Testfälle
// des Werkzeugs (`cargo test`) und proben() hier unten halten beide Seiten zusammen.

import * as daten from './daten.js';
import * as nutzer from './nutzer.js';

const STOPP = new Set([
  'der', 'die', 'das', 'des', 'dem', 'den', 'ein', 'eine', 'einer', 'eines', 'einem', 'einen',
  'und', 'oder', 'aber', 'auch', 'als', 'am', 'an', 'auf', 'aus', 'bei', 'bis', 'durch', 'für',
  'fur', 'gegen', 'im', 'in', 'ist', 'sind', 'mit', 'nach', 'nicht', 'von', 'vor', 'zu', 'zum',
  'zur', 'über', 'uber', 'unter', 'sowie', 'dass', 'dieser', 'diese', 'dieses', 'werden', 'wird',
  'kann', 'können', 'konnen', 'muss', 'müssen', 'mussen', 'soll', 'sollen', 'haben', 'hat',
  'ihre', 'ihrer', 'ihren', 'ihr', 'sich', 'sie', 'es', 'er', 'wenn', 'wie', 'so', 'einschließlich',
  'the', 'and', 'or', 'of', 'to', 'in', 'for', 'on', 'with', 'by', 'as', 'at', 'from', 'that',
  'this', 'these', 'those', 'be', 'is', 'are', 'was', 'were', 'shall', 'may', 'which', 'such',
  'any', 'all', 'their', 'its', 'not', 'other', 'where', 'when',
]);

const FALTUNG = { 'ä': 'a', 'ö': 'o', 'ü': 'u', 'á': 'a', 'à': 'a', 'â': 'a', 'é': 'e', 'è': 'e', 'ê': 'e', 'í': 'i', 'ì': 'i', 'ó': 'o', 'ò': 'o', 'ô': 'o', 'ú': 'u', 'ù': 'u', 'û': 'u', 'ç': 'c' };
const ENDUNGEN = ['ern', 'est', 'end', 'ing', 'em', 'er', 'en', 'es', 'et', 'ed', 'e', 's', 'n'];

/** Wort auf den Suchstamm bringen; null = nicht indexiert. */
export function stamm(wort) {
  let s = String(wort).toLowerCase().replace(/[äöüáàâéèêíìóòôúùûç]/g, (c) => FALTUNG[c]).replace(/ß/g, 'ss');
  if (s.length < 2 || STOPP.has(s)) return null;
  s = kuerze(s);
  if (s.length < 2 || STOPP.has(s)) return null;
  return s;
}

function kuerze(w) {
  if (/^\d+$/.test(w)) return w;
  let s = w;
  if (s.length > 7 && s.endsWith('ungen')) s = s.slice(0, -5);
  else if (s.length > 5 && s.endsWith('ung')) s = s.slice(0, -3);
  else if (s.length > 5 && s.endsWith('ies')) return s.slice(0, -3) + 'y';
  for (let runde = 0; runde < 2; runde++) {
    for (const suf of ENDUNGEN) {
      if (s.length >= suf.length + 4 && s.endsWith(suf)) { s = s.slice(0, -suf.length); break; }
    }
  }
  if (s.length > 4 && s[s.length - 1] === s[s.length - 2] && 'nmtlrsfp'.includes(s[s.length - 1])) {
    s = s.slice(0, -1);
  }
  return s;
}

export function worte(text) {
  const aus = [];
  for (const roh of String(text).split(/[^\p{L}\p{N}]+/u)) {
    if (!roh) continue;
    const s = stamm(roh);
    if (s) aus.push(s);
  }
  return aus;
}

/** Dieselben Paare prüft `cargo test` im Werkzeug. Aufruf: suche.proben() in der Konsole. */
export function proben() {
  const paare = [
    ['Informationsregister', 'informationsregist'], ['Informationsregisters', 'informationsregist'],
    ['Meldungen', 'meld'], ['Meldung', 'meld'], ['Auslagerung', 'auslag'], ['Auslagerungen', 'auslag'],
    ['IKT', 'ikt'], ['Drittparteienrisiko', 'drittparteienrisiko'], ['2022', '2022'],
    ['Resilienz', 'resilienz'], ['Finanzunternehmen', 'finanzunternehm'],
    ['requirements', 'requirement'], ['policies', 'policy'],
  ];
  const fehler = paare.filter(([a, b]) => stamm(a) !== b).map(([a, b]) => `${a}: ${stamm(a)} statt ${b}`);
  if (stamm('und') !== null) fehler.push('und müsste Stoppwort sein');
  console[fehler.length ? 'error' : 'log'](fehler.length ? fehler : 'Stammbildung stimmt mit dem Werkzeug überein.');
  return fehler.length === 0;
}

// ------------------------------------------------------------------- Indexladen

let geladen = null;

/** Lädt alle Indexdateien der gewünschten Sprache (einmalig). */
export async function laden(sprache = 'de') {
  if (geladen && geladen.sprache === sprache) return geladen.indizes;
  const verz = await daten.suchindex();
  const passend = (verz.indizes || []).filter((i) => i.sprache === sprache);
  const indizes = await Promise.all(passend.map(async (i) => {
    const d = await daten.index(i.datei);
    return { ...d, datei: i.datei };
  }));
  geladen = { sprache, indizes };
  return indizes;
}

/**
 * Sucht über alle geladenen Indizes.
 * @returns {Promise<{treffer:Array, begriffe:Array, durchsucht:number, dokumente:number}>}
 */
export async function suchen(frage, optionen = {}) {
  const sprache = optionen.sprache || 'de';
  const nurRw = optionen.regelwerk || null;
  const begriffe = [...new Set(worte(frage))];
  if (!begriffe.length) return { treffer: [], begriffe: [], durchsucht: 0, dokumente: 0 };

  const indizes = (await laden(sprache)).filter((i) => !nurRw || i.regelwerk === nurRw);
  const katalog = await daten.katalog();
  const nachId = new Map(katalog.regelwerke.map((r) => [r.id, r]));

  // Dokumenthäufigkeit über alle Regelwerke zusammen – so sind die Punktzahlen vergleichbar.
  let gesamtDoks = 0;
  const df = new Map();
  for (const idx of indizes) {
    gesamtDoks += idx.dokumente.length;
    for (const b of begriffe) {
      const p = idx.worte[b];
      if (p) df.set(b, (df.get(b) || 0) + p.length);
    }
  }
  const idf = new Map();
  for (const b of begriffe) {
    const d = df.get(b) || 0;
    idf.set(b, Math.log(1 + (gesamtDoks - d + 0.5) / (d + 0.5)));
  }

  const k1 = 1.2, bb = 0.75;
  const treffer = [];
  for (const idx of indizes) {
    const rw = nachId.get(idx.regelwerk);
    if (rw && nutzer.relevanz(rw) === 'nichtrelevant') continue;
    const punkte = new Map();   // dokNr -> {p, treffer:Set}
    for (const b of begriffe) {
      const postings = idx.worte[b];
      if (!postings) continue;
      const gewicht = idf.get(b);
      for (const [nr, tf] of postings) {
        const dok = idx.dokumente[nr];
        if (!dok) continue;
        const norm = tf * (k1 + 1) / (tf + k1 * (1 - bb + bb * (dok.l / (idx.avgdl || 1))));
        const e = punkte.get(nr) || { p: 0, gefunden: new Set() };
        e.p += gewicht * norm;
        e.gefunden.add(b);
        punkte.set(nr, e);
      }
    }
    for (const [nr, e] of punkte) {
      const dok = idx.dokumente[nr];
      // Abdeckung: wer mehr Suchbegriffe enthält, steht weiter oben.
      const abdeckung = e.gefunden.size / begriffe.length;
      treffer.push({
        regelwerk: idx.regelwerk,
        kurzname: rw ? rw.kurzname : idx.regelwerk,
        modus: rw ? rw.modus : 'original',
        id: dok.id,
        pfad: dok.pfad,
        bezeichnung: dok.b,
        ausschnitt: dok.t,
        punkte: e.p * (0.45 + 0.55 * abdeckung) * (abdeckung === 1 ? 1.25 : 1),
        gefunden: [...e.gefunden],
      });
    }
  }
  treffer.sort((a, b) => b.punkte - a.punkte);
  return { treffer, begriffe, durchsucht: indizes.length, dokumente: gesamtDoks };
}

/** Markiert die Suchbegriffe im Ausschnitt (stammbasiert, deshalb auch bei Beugung). */
export function hervorheben(text, begriffe) {
  const menge = new Set(begriffe);
  const stuecke = String(text).split(/(\p{L}[\p{L}\p{N}]*)/u);
  return stuecke.map((s, i) => {
    if (i % 2 === 0) return document.createTextNode(s);
    const st = stamm(s);
    if (st && menge.has(st)) {
      const m = document.createElement('mark');
      m.textContent = s;
      return m;
    }
    return document.createTextNode(s);
  });
}
