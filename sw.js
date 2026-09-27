// Service Worker: macht die App offlinefähig.
//
// Beim Ausliefern einer neuen Version die CACHE-Nummer hochzählen - dann holt der
// Worker alles frisch und wirft die alten Caches weg. Absichtlich wird jede Datei
// EINZELN gecacht (cache.add statt addAll): sonst legt eine einzige fehlende Datei
// den ganzen Offline-Betrieb lahm.

const CACHE = 'aufsichtskompass-v1';

const GERUEST = [
  './',
  'index.html',
  'manifest.webmanifest',
  'icon.svg',
  'js/app.js',
  'js/ui.js',
  'js/daten.js',
  'js/nutzer.js',
  'js/bibliothek.js',
  'js/viewer.js',
  'js/suche.js',
  'daten/regelwerke.json',
  'daten/themen.json',
  'daten/quellen.json',
  'daten/fussnoten.json',
  'daten/suchindex.json',
  'daten/bestand.json',
];

self.addEventListener('install', (e) => {
  e.waitUntil((async () => {
    const c = await caches.open(CACHE);
    await Promise.all(GERUEST.map((d) => c.add(new Request(d, { cache: 'reload' })).catch(() => {})));
    self.skipWaiting();
  })());
});

self.addEventListener('activate', (e) => {
  e.waitUntil((async () => {
    for (const name of await caches.keys()) {
      if (name !== CACHE) await caches.delete(name);
    }
    await self.clients.claim();
  })());
});

// Beim Entwickeln nichts abfangen: sonst liefert der Worker die Seite von vorhin aus
// und man prueft eine Fassung, die es im Ordner gar nicht mehr gibt.
const ENTWICKLUNG = ['localhost', '127.0.0.1', '::1'].includes(location.hostname);

self.addEventListener('fetch', (e) => {
  const anfrage = e.request;
  if (ENTWICKLUNG) return;
  if (anfrage.method !== 'GET' || new URL(anfrage.url).origin !== location.origin) return;

  // Inhalte: erst Netz (damit eine neue Fassung sofort sichtbar ist), dann Kopie.
  if (anfrage.url.includes('/daten/')) {
    e.respondWith((async () => {
      try {
        const antwort = await fetch(anfrage);
        if (antwort.ok) {
          const c = await caches.open(CACHE);
          c.put(anfrage, antwort.clone());
        }
        return antwort;
      } catch (fehler) {
        const kopie = await caches.match(anfrage);
        if (kopie) return kopie;
        throw fehler;
      }
    })());
    return;
  }

  // Geruest: Kopie sofort ausliefern, im Hintergrund erneuern (stale-while-revalidate).
  // So ist die App sofort da, holt sich eine neue Fassung aber spaetestens beim
  // naechsten Aufruf - auch dann, wenn jemand vergisst, CACHE hochzuzaehlen.
  e.respondWith((async () => {
    const kopie = await caches.match(anfrage, { ignoreSearch: true });
    const frisch = fetch(anfrage).then(async (antwort) => {
      if (antwort.ok) {
        const c = await caches.open(CACHE);
        await c.put(anfrage, antwort.clone());
      }
      return antwort;
    });
    if (kopie) {
      frisch.catch(() => {});   // offline ist kein Fehler
      return kopie;
    }
    return frisch;
  })());
});
