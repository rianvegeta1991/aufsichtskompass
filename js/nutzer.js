// Alles, was einem Nutzer gehört, bleibt auf dem Gerät: Erscheinungsbild, Sprache,
// Relevanz-Einstufung je Regelwerk, Lesezeichen, Notizen. Kein Konto, keine Übertragung.

const SCHLUESSEL = 'aufsichtskompass-nutzer';

const STANDARD = {
  thema: 'system',        // system | hell | dunkel
  sprache: 'de',          // Anzeigesprache der Originaltexte
  relevanz: {},           // regelwerkId -> relevant | referenz | nichtrelevant (sticht den Katalog)
  lesezeichen: [],        // { rw, pfad, bez, zeit }
  notizen: {},            // "rw#fundstellenId" -> Text
  begriffe: false,        // Begriffsbestimmungen im Text auszeichnen
  gelesen: {},            // Meldungs-Id -> true
  newsGemerkt: [],        // { id, titel, url, zeit }
  rueckmeldungen: {},     // Meldungs-Id -> { wert, begriffe, zeit }
};

let zustand = laden();

function laden() {
  try {
    const roh = localStorage.getItem(SCHLUESSEL);
    if (!roh) return structuredClone(STANDARD);
    return { ...structuredClone(STANDARD), ...JSON.parse(roh) };
  } catch {
    return structuredClone(STANDARD);
  }
}

function sichern() {
  try {
    localStorage.setItem(SCHLUESSEL, JSON.stringify(zustand));
  } catch {
    /* privates Fenster oder voller Speicher: die App läuft ohne Gedächtnis weiter */
  }
}

export function alles() { return zustand; }

export function hol(feld) { return zustand[feld]; }

export function setz(feld, wert) {
  zustand[feld] = wert;
  sichern();
}

// ------------------------------------------------------------- Erscheinungsbild

export function thema() { return zustand.thema; }

/** Setzt das Erscheinungsbild und gibt das nun wirksame zurück. */
export function themaSetzen(wert) {
  zustand.thema = wert;
  sichern();
  themaAnwenden();
  return wirksamesThema();
}

export function wirksamesThema() {
  if (zustand.thema !== 'system') return zustand.thema;
  return matchMedia('(prefers-color-scheme: dark)').matches ? 'dunkel' : 'hell';
}

export function themaAnwenden() {
  const w = zustand.thema;
  if (w === 'system') document.documentElement.removeAttribute('data-thema');
  else document.documentElement.setAttribute('data-thema', w);
}

// -------------------------------------------------------------------- Relevanz

/** Relevanz eines Regelwerks: eigene Einstufung sticht den Katalogwert. */
export function relevanz(rw) {
  return zustand.relevanz[rw.id] || rw.relevanz || 'relevant';
}

export function relevanzSetzen(id, wert) {
  if (!wert) delete zustand.relevanz[id];
  else zustand.relevanz[id] = wert;
  sichern();
}

export function relevanzEigen(id) { return Boolean(zustand.relevanz[id]); }

// ----------------------------------------------------------------- Lesezeichen

export function lesezeichen() { return zustand.lesezeichen; }

export function istGemerkt(rw, pfad) {
  return zustand.lesezeichen.some((l) => l.rw === rw && l.pfad === pfad);
}

export function merken(rw, pfad, bez) {
  if (istGemerkt(rw, pfad)) {
    zustand.lesezeichen = zustand.lesezeichen.filter((l) => !(l.rw === rw && l.pfad === pfad));
    sichern();
    return false;
  }
  zustand.lesezeichen.unshift({ rw, pfad, bez, zeit: new Date().toISOString() });
  zustand.lesezeichen = zustand.lesezeichen.slice(0, 300);
  sichern();
  return true;
}

// --------------------------------------------------------------------- Notizen

export function notiz(rw, id) { return zustand.notizen[`${rw}#${id}`] || ''; }

export function notizSetzen(rw, id, text) {
  const k = `${rw}#${id}`;
  if (text && text.trim()) zustand.notizen[k] = text;
  else delete zustand.notizen[k];
  sichern();
}

export function notizenZu(rw) {
  return Object.entries(zustand.notizen)
    .filter(([k]) => k.startsWith(rw + '#'))
    .map(([k, t]) => ({ id: k.slice(rw.length + 1), text: t }));
}

export function notizenAnzahl() { return Object.keys(zustand.notizen).length; }

// -------------------------------------------------------------------- News

export function istGelesen(id) { return zustand.gelesen[id] === true; }

export function gelesenUmschalten(id) {
  if (zustand.gelesen[id]) delete zustand.gelesen[id];
  else zustand.gelesen[id] = true;
  sichern();
  return istGelesen(id);
}

export function newsGemerkt() { return zustand.newsGemerkt; }

export function istGemerktNews(id) {
  return zustand.newsGemerkt.some((m) => m.id === id);
}

export function newsMerken(id, titel, url) {
  if (istGemerktNews(id)) {
    zustand.newsGemerkt = zustand.newsGemerkt.filter((m) => m.id !== id);
    sichern();
    return false;
  }
  zustand.newsGemerkt.unshift({ id, titel, url, zeit: new Date().toISOString() });
  zustand.newsGemerkt = zustand.newsGemerkt.slice(0, 200);
  sichern();
  return true;
}

export function rueckmeldung(id) {
  const r = zustand.rueckmeldungen[id];
  return r ? r.wert : '';
}

/**
 * Rückmeldung zur Relevanz. Sie wirkt zunächst nur hier: Die Gewichte stehen in
 * daten/taxonomie.json und werden redaktionell gepflegt. Unter „Lesezeichen &
 * Notizen" stehen die Begriffe der abgelehnten Meldungen gesammelt – daraus lässt
 * sich die Taxonomie gezielt nachschärfen.
 */
export function rueckmeldungSetzen(id, wert, begriffe = []) {
  if (!wert) delete zustand.rueckmeldungen[id];
  else zustand.rueckmeldungen[id] = { wert, begriffe, zeit: new Date().toISOString() };
  sichern();
}

export function rueckmeldungen() { return zustand.rueckmeldungen; }
