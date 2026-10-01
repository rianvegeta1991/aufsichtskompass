// Kleine Bausteine für die Oberfläche. Kein Framework, keine Abhängigkeiten.

/**
 * Adresse für einen Link nach außen – oder `null`.
 *
 * Grund (Phase 8): Die Adressen der Meldungen kommen aus **fremden** Feeds. Stünde
 * dort `javascript:…`, würde ein Klick Code im Kontext der Seite ausführen. Deshalb
 * wird hier nur durchgelassen, was sich als http- oder https-Adresse lesen lässt;
 * das Werkzeug prüft beim Einsammeln dasselbe noch einmal. Kinder von `el` sind
 * immer Textknoten, Attribute setzt `setAttribute` – damit bleibt nur dieser Weg.
 */
export function externURL(u) {
  if (typeof u !== 'string' || !u) return null;
  try {
    const a = new URL(u, location.href);
    return a.protocol === 'https:' || a.protocol === 'http:' ? a.href : null;
  } catch {
    return null;
  }
}

/** Element bauen: el('div.karte', {id:'x'}, 'Text', kindElement, …) */
export function el(spec, attrs, ...kinder) {
  const [tag, ...klassen] = String(spec).split('.');
  const n = document.createElement(tag || 'div');
  if (klassen.length) n.className = klassen.join(' ');
  if (attrs && (attrs.nodeType || typeof attrs === 'string' || Array.isArray(attrs))) {
    kinder.unshift(attrs);
  } else if (attrs) {
    for (const [k, v] of Object.entries(attrs)) {
      if (v === null || v === undefined || v === false) continue;
      if (k === 'text') n.textContent = v;
      else if (k === 'dataset') Object.assign(n.dataset, v);
      else if (k.startsWith('on') && typeof v === 'function') n.addEventListener(k.slice(2), v);
      else n.setAttribute(k, v === true ? '' : v);
    }
  }
  for (const k of kinder.flat()) {
    if (k === null || k === undefined || k === false) continue;
    n.append(k.nodeType ? k : document.createTextNode(String(k)));
  }
  return n;
}

export function leere(knoten) {
  while (knoten.firstChild) knoten.removeChild(knoten.firstChild);
  return knoten;
}

/** Chip mit Beschriftung – Farbe ist nie das einzige Merkmal. */
export function chip(text, klasse, titel) {
  return el('span.chip' + (klasse ? '.' + klasse : ''), { title: titel || null }, text);
}

export const VERBINDLICHKEIT = {
  bindend: ['bindend', 'bindend', 'Unmittelbar rechtsverbindlich'],
  auslegung: ['auslegung', 'Auslegung der Aufsicht', 'Verwaltungspraxis der Aufsicht'],
  leitlinie: ['leitlinie', 'Leitlinie', 'Leitlinie oder Empfehlung'],
  standard: ['', 'freiwilliger Standard', 'Kein Rechtsakt'],
};

export const MODUS = {
  original: ['original', 'Originaltext', 'Der amtliche Wortlaut ist hier zulässig und wird angezeigt.'],
  zusammenfassung: ['zusammenfassung', 'Zusammenfassung', 'Kein zulässiger Volltext – eigene Zusammenfassung.'],
};

export const TIEFE = { voll: 'volle Tiefe', mittel: 'mittlere Tiefe', grob: 'grobe Übersicht' };

/** Datum "2024-10-15" oder ISO-Zeitpunkt deutsch ausgeben. */
export function datum(s) {
  if (!s) return '–';
  const m = String(s).match(/^(\d{4})-(\d{2})-(\d{2})/);
  if (m) return `${m[3]}.${m[2]}.${m[1]}`;
  const d = new Date(s);
  return isNaN(d) ? String(s) : d.toLocaleDateString('de-DE');
}

export function zahl(n) {
  return new Intl.NumberFormat('de-DE').format(n);
}

/**
 * Tabelle, die auf schmalen Schirmen zu gestapelten Karten wird: jede Zelle traegt
 * ihren Spaltennamen in `data-spalte`, das CSS blendet den Kopf aus und stellt den
 * Namen vor den Wert. Besser als Querscrollen bei sieben Spalten auf 375 px.
 */
export function tabelle(spalten, zeilen) {
  return el('table.liste.stapel',
    el('thead', el('tr', spalten.map((s) => el('th', s)))),
    el('tbody', zeilen.map((z) => el('tr',
      z.map((zelle, i) => el('td', { dataset: { spalte: spalten[i] || '' } }, zelle))))));
}

/** Kürzt auf Wortgrenze. */
export function kurz(text, max = 160) {
  const t = String(text || '');
  if (t.length <= max) return t;
  return t.slice(0, t.lastIndexOf(' ', max) > 0 ? t.lastIndexOf(' ', max) : max) + '…';
}

/**
 * Lädt eine Tabelle als CSV herunter. Semikolon und BOM, weil Excel in deutscher
 * Einstellung sonst alles in eine Spalte legt bzw. Umlaute zerlegt.
 */
export function csvLaden(name, spalten, zeilen) {
  const feld = (w) => {
    const t = String(w ?? '').replace(/"/g, '""').replace(/\s*[\r\n]+\s*/g, ' ');
    return /[";]/.test(t) ? `"${t}"` : t;
  };
  // BOM voran und Semikolon als Trenner: sonst zerlegt Excel in deutscher
  // Einstellung die Umlaute und legt alles in eine Spalte.
  const text = '\uFEFF' + [spalten, ...zeilen].map((z) => z.map(feld).join(';')).join('\r\n');
  const url = URL.createObjectURL(new Blob([text], { type: 'text/csv;charset=utf-8' }));
  const a = el('a', { href: url, download: `${name}.csv` });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 2000);
  ton(`${name}.csv gespeichert.`);
}

/** Lädt Text als Datei herunter – dieselbe Mechanik wie `csvLaden`, nur ohne Tabelle. */
export function textLaden(name, inhalt, typ = 'application/json;charset=utf-8') {
  const url = URL.createObjectURL(new Blob([inhalt], { type: typ }));
  const a = el('a', { href: url, download: name });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 2000);
  ton(`${name} gespeichert.`);
}

/** Knopfpaar für den Export einer Ansicht. */
export function exportknoepfe(name, spalten, zeilen) {
  return el('div', { style: 'display:flex;gap:8px;flex-wrap:wrap' },
    el('button.knopf', { type: 'button', onclick: () => csvLaden(name, spalten, zeilen()) }, '↓ CSV'),
    el('button.knopf', { type: 'button', onclick: () => print() }, '🖶 Drucken / PDF'));
}

/** Setzt den Seitentitel und die H1-Zeile. */
export function titel(seite) {
  document.title = seite ? `${seite} – Aufsichtskompass` : 'Aufsichtskompass';
}

export function fehlerkarte(text, detail) {
  return el('div.karte', el('h2', 'Das hat nicht geklappt'), el('p', text),
    detail ? el('p', el('code', String(detail))) : null);
}

/** Kurzer Statuston am unteren Rand (z. B. „Zitat kopiert"). */
let tonKnoten = null;
export function ton(text) {
  if (!tonKnoten) {
    tonKnoten = el('div', {
      role: 'status', 'aria-live': 'polite',
      style: 'position:fixed;left:50%;bottom:22px;transform:translateX(-50%);z-index:60;' +
        'background:var(--color-text);color:var(--color-surface);padding:9px 16px;border-radius:8px;' +
        'box-shadow:var(--schatten-stark);font-size:.88rem;opacity:0;transition:opacity .2s',
    });
    document.body.append(tonKnoten);
  }
  tonKnoten.textContent = text;
  tonKnoten.style.opacity = '1';
  clearTimeout(tonKnoten._t);
  tonKnoten._t = setTimeout(() => { tonKnoten.style.opacity = '0'; }, 2400);
}
