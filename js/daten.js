// Zugriff auf die Daten unter daten/. Alles wird einmal geholt und im Speicher gehalten;
// der Service Worker legt zusätzlich eine Offline-Kopie an.

const WURZEL = 'daten/';
const gecacht = new Map();

function hole(pfad) {
  if (!gecacht.has(pfad)) {
    gecacht.set(pfad, fetch(WURZEL + pfad, { cache: 'no-cache' }).then((a) => {
      if (!a.ok) throw new Error(`${pfad}: HTTP ${a.status}`);
      return a.json();
    }).catch((e) => {
      gecacht.delete(pfad);   // Fehlschlag nicht dauerhaft festhalten
      throw e;
    }));
  }
  return gecacht.get(pfad);
}

export const katalog = () => hole('regelwerke.json');
export const themen = () => hole('themen.json');
export const quellen = () => hole('quellen.json');
export const fussnoten = () => hole('fussnoten.json');
export const aenderungen = () => hole('aenderungen.json').catch(() => ({ ereignisse: [] }));
export const suchindex = () => hole('suchindex.json').catch(() => ({ indizes: [] }));

/** Verzeichnis der Regelwerke mit Volltext (vom Werkzeug geschrieben). */
export const bestand = () => hole('bestand.json').catch(() => ({ bestand: {} }));
export const fassungen = (rw) => hole(`rw/${rw}/fassungen.json`);
export const struktur = (rw, fassung) => hole(`rw/${rw}/${fassung}/struktur.json`);
export const text = (rw, fassung, sprache) => hole(`rw/${rw}/${fassung}/text-${sprache}.json`);
export const index = (datei) => hole(datei);

/** Katalogeintrag zu einer Id. */
export async function regelwerk(id) {
  const k = await katalog();
  return k.regelwerke.find((r) => r.id === id) || null;
}

/**
 * Aktuelle (jüngste) Fassung eines Regelwerks – oder null, wenn keine vorliegt.
 * Läuft über bestand.json, damit die App nicht für jedes Regelwerk ohne Volltext
 * eine Datei anfragt, die es nicht gibt.
 */
export async function aktuelleFassung(rw) {
  const b = await bestand();
  const e = b.bestand && b.bestand[rw];
  return e ? e.fassung : null;
}

/** Sprachen, in denen eine Fassung vorliegt. */
export async function sprachen(rw) {
  const b = await bestand();
  const e = b.bestand && b.bestand[rw];
  return e ? e.sprachen : [];
}

/** Lädt Struktur und Text einer Fassung in der gewünschten Sprache (mit Rückfall auf de). */
export async function fassungLaden(rw, sprache = 'de') {
  const f = await aktuelleFassung(rw);
  if (!f) return null;
  const s = await struktur(rw, f.id);
  const spr = s.sprachen.includes(sprache) ? sprache : s.sprachen[0];
  const t = await text(rw, f.id, spr);
  return { fassung: f, struktur: s, text: t, sprache: spr };
}

/** Flache Karte Pfad -> Knoten und Id -> Knoten über einen Strukturbaum. */
export function knotenkarten(knoten) {
  const nachPfad = new Map();
  const nachId = new Map();
  const eltern = new Map();
  (function gehe(liste, vater) {
    for (const k of liste) {
      nachId.set(k.id, k);
      if (k.pfad) nachPfad.set(k.pfad, k);
      if (vater) eltern.set(k.id, vater);
      if (k.kinder) gehe(k.kinder, k);
    }
  })(knoten, null);
  return { nachPfad, nachId, eltern };
}

/** Bezeichnung eines Knotens, z. B. „Artikel 28 – Allgemeine Prinzipien". */
export function bezeichnung(knoten, sprache = 'de') {
  const w = WORT[knoten.art] || [knoten.art, knoten.art];
  const wort = sprache === 'en' ? w[1] : w[0];
  const t = knoten.titel && (knoten.titel[sprache] || knoten.titel.de);
  if (knoten.nummer && t) return `${wort} ${knoten.nummer} – ${t}`;
  if (knoten.nummer) return `${wort} ${knoten.nummer}`;
  return t || wort;
}

export const WORT = {
  teil: ['Titel', 'Title'],
  kapitel: ['Kapitel', 'Chapter'],
  abschnitt: ['Abschnitt', 'Section'],
  artikel: ['Artikel', 'Article'],
  absatz: ['Absatz', 'paragraph'],
  erwaegungsgrund: ['Erwägungsgrund', 'Recital'],
  bezugsvermerk: ['Bezugsvermerk', 'Citation'],
  anhang: ['Anhang', 'Annex'],
  praeambel: ['Präambel', 'Preamble'],
};
