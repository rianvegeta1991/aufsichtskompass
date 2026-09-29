// Datenschicht der Interdependenz-Analyse: fachliche Beziehungen, belegte Verweise,
// Themenzuordnung, Meldepflichten und Rollen.
//
// Zwei Sorten Beziehungen, bewusst getrennt gehalten:
//   * `beziehungen.json` - redaktionell, mit Begründung, Konfidenz und Prüfstatus.
//   * `verweise.json`    - vom Werkzeug aus dem Wortlaut gewonnen, mit Beleg.
// Die Verweisdatei ist groß (rund 800 KB), deshalb wird sie erst geladen, wenn sie
// wirklich gebraucht wird, und danach im Speicher gehalten.

import * as daten from './daten.js';

const WURZEL = 'daten/';
const gecacht = new Map();

function hole(name) {
  if (!gecacht.has(name)) {
    gecacht.set(name, fetch(WURZEL + name).then((a) => {
      if (!a.ok) throw new Error(`${name}: HTTP ${a.status}`);
      return a.json();
    }).catch((e) => {
      gecacht.delete(name);
      throw e;
    }));
  }
  return gecacht.get(name);
}

export const beziehungen = () => hole('beziehungen.json').catch(() => ({ beziehungen: [], typen: {} }));
export const verweise = () => hole('verweise.json').catch(() => ({ verweise: [], paare: {} }));
export const themenzuordnung = () => hole('themenzuordnung.json').catch(() => ({ zuordnungen: [] }));
export const meldepflichten = () => hole('meldepflichten.json').catch(() => null);
export const rollen = () => hole('rollen.json').catch(() => null);

export const TYP_NAME = {
  entspricht: 'entspricht',
  konkretisiert: 'konkretisiert',
  ersetzt: 'ersetzt',
  verweist: 'verweist auf',
  ueberschneidet: 'überschneidet teilweise',
  'geht-weiter': 'geht weiter als',
  spannungsfeld: 'Spannungsfeld',
  'lex-specialis': 'lex specialis',
};

/** Kurzzeichen für die Matrixzellen – Farbe allein soll nie die Aussage tragen. */
export const TYP_ZEICHEN = {
  entspricht: '=',
  konkretisiert: '⊂',
  ersetzt: '↦',
  verweist: '→',
  ueberschneidet: '∩',
  'geht-weiter': '>',
  spannungsfeld: '≠',
  'lex-specialis': '§',
};

/**
 * Alle Beziehungen, die eine Fundstelle berühren – in beide Richtungen.
 * `pfad` darf fehlen: dann zählt alles, was am Regelwerk hängt.
 */
export function zuFundstelle(liste, rw, pfad) {
  const passt = (s) => s.rw === rw && (!s.pfad || !pfad || s.pfad === pfad || pfad.startsWith(s.pfad + '/'));
  return liste.filter((b) => passt(b.von) || passt(b.nach)).map((b) => ({
    ...b,
    richtung: passt(b.von) ? 'von' : 'nach',
    gegenueber: passt(b.von) ? b.nach : b.von,
  }));
}

/** Zählt Beziehungen je Regelwerkspaar; `ungerichtet` legt A→B und B→A zusammen. */
export function paare(liste, ungerichtet = true) {
  const karte = new Map();
  for (const b of liste) {
    const a = b.von.rw;
    const z = b.nach.rw;
    if (a === z) continue;
    const schluessel = ungerichtet ? [a, z].sort().join('|') : `${a}|${z}`;
    const e = karte.get(schluessel) || { anzahl: 0, typen: new Set(), eintraege: [] };
    e.anzahl++;
    e.typen.add(b.typ);
    e.eintraege.push(b);
    karte.set(schluessel, e);
  }
  return karte;
}

/** Fundstellen und Regelwerke zu einem Thema. */
export async function zuThema(themaId) {
  const [z, k] = await Promise.all([themenzuordnung(), daten.katalog()]);
  const stellen = (z.zuordnungen || []).filter((e) => (e.themen || []).includes(themaId));
  const regelwerke = k.regelwerke.filter((r) => (r.themen || []).includes(themaId));
  return { stellen, regelwerke };
}

/** Lädt zu einer Liste von Fundstellen die Bezeichnungen aus den Strukturen nach. */
export async function bezeichnungen(stellen) {
  const nachRw = new Map();
  for (const s of stellen) {
    if (!nachRw.has(s.rw)) nachRw.set(s.rw, []);
    nachRw.get(s.rw).push(s);
  }
  const aus = new Map();
  // Nur die Struktur laden, nicht den Text: für die Bezeichnung reicht der Baum,
  // und der Text ist um ein Vielfaches größer (VAG allein rund 2 MB).
  await Promise.all([...nachRw.entries()].map(async ([rw, liste]) => {
    const f = await daten.aktuelleFassung(rw);
    if (!f) return;
    const s = await daten.struktur(rw, f.id).catch(() => null);
    if (!s) return;
    const karten = daten.knotenkarten(s.knoten);
    for (const x of liste) {
      const k = karten.nachPfad.get(x.pfad);
      if (!k) continue;
      // Ein Absatz für sich genommen heißt nur "Absatz 3" - erst mit seinem
      // Artikel bzw. Paragrafen wird daraus eine brauchbare Bezeichnung.
      const vater = k.art === 'absatz' ? karten.eltern.get(k.id) : null;
      const text = vater
        ? `${daten.bezeichnung(vater, 'de')}, ${daten.bezeichnung(k, 'de')}`
        : daten.bezeichnung(k, 'de');
      aus.set(`${rw}|${x.pfad}`, text);
    }
  }));
  return aus;
}
