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
  lernen: {               // Lernfortschritt - ausschliesslich auf diesem Geraet
    name: '',
    lektionen: {},        // id -> ISO-Zeit
    quizze: {},           // id -> { versuche: [...], beste: 0..1 }
    karten: {},           // id -> { stufe, faellig }
    faelle: {},           // id -> { zeit, bewertungen }
  },
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

// ------------------------------------------------------------------ Lernen

export function lernstand() {
  if (!zustand.lernen) zustand.lernen = structuredClone(STANDARD.lernen);
  return zustand.lernen;
}

export function lernname() { return lernstand().name || ''; }

export function lernnameSetzen(n) {
  lernstand().name = n;
  sichern();
}

export function lektionUmschalten(id) {
  const l = lernstand();
  if (l.lektionen[id]) delete l.lektionen[id];
  else l.lektionen[id] = new Date().toISOString();
  sichern();
  return Boolean(l.lektionen[id]);
}

export function quizErgebnis(id, richtig, gesamt) {
  const l = lernstand();
  const quote = gesamt ? richtig / gesamt : 0;
  const e = l.quizze[id] || { versuche: [], beste: 0 };
  e.versuche.unshift({ zeit: new Date().toISOString(), richtig, gesamt });
  e.versuche = e.versuche.slice(0, 20);
  e.beste = Math.max(e.beste || 0, quote);
  l.quizze[id] = e;
  sichern();
  return e;
}

/**
 * Verteilte Wiederholung: Wer die Karte weiß, rückt eine Stufe vor, wer sie nicht
 * weiß, faengt wieder vorn an. Die Abstaende kommen aus der Datendatei.
 */
export function karteAntwort(id, gewusst, intervalle = [1, 3, 7, 16, 35]) {
  const l = lernstand();
  const alt = l.karten[id] || { stufe: -1 };
  const stufe = gewusst ? Math.min(alt.stufe + 1, intervalle.length - 1) : 0;
  const tage = intervalle[stufe] || 1;
  l.karten[id] = {
    stufe,
    faellig: new Date(Date.now() + tage * 86400000).toISOString(),
    zuletzt: new Date().toISOString(),
  };
  sichern();
  return l.karten[id];
}

export function fallAbschluss(id, bewertungen) {
  const l = lernstand();
  l.faelle[id] = { zeit: new Date().toISOString(), bewertungen };
  sichern();
}
