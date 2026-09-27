// Kleine Bausteine für die Oberfläche. Kein Framework, keine Abhängigkeiten.

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
      if (k === 'html') n.innerHTML = v;
      else if (k === 'text') n.textContent = v;
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

/** Kürzt auf Wortgrenze. */
export function kurz(text, max = 160) {
  const t = String(text || '');
  if (t.length <= max) return t;
  return t.slice(0, t.lastIndexOf(' ', max) > 0 ? t.lastIndexOf(' ', max) : max) + '…';
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
