// Domino tiles as SVG. A half is a unit square; pips sit on the usual 3 by 3
// grid. The vertical layout has the six in two columns; a horizontal tile
// swaps x and y, so the six lies in two rows.
const PIPS = {
  0: [],
  1: [[0.5, 0.5]],
  2: [[0.27, 0.27], [0.73, 0.73]],
  3: [[0.27, 0.27], [0.5, 0.5], [0.73, 0.73]],
  4: [[0.27, 0.27], [0.73, 0.27], [0.27, 0.73], [0.73, 0.73]],
  5: [[0.27, 0.27], [0.73, 0.27], [0.5, 0.5], [0.27, 0.73], [0.73, 0.73]],
  6: [[0.28, 0.23], [0.72, 0.23], [0.28, 0.5], [0.72, 0.5], [0.28, 0.77], [0.72, 0.77]],
};

// A tile [a|b] with a at the left (horizontal) or top (vertical). u is the
// side of one half in px. With back = true it is drawn face down.
function tileSVG(a, b, { vertical = false, u = 24, back = false, cls = "" } = {}) {
  const w = vertical ? u : 2 * u;
  const h = vertical ? 2 * u : u;
  const r = Math.max(3, u * 0.16);
  let body;
  if (back) {
    body = `<rect x="1" y="1" width="${w - 2}" height="${h - 2}" rx="${r}" fill="#16325a" stroke="#0b1f3a" stroke-width="1.5"/>` +
      `<rect x="${u * 0.18}" y="${u * 0.18}" width="${w - u * 0.36}" height="${h - u * 0.36}" rx="${r * 0.6}" fill="none" stroke="rgba(255,255,255,0.22)" stroke-width="1.2"/>`;
  } else {
    const pr = Math.max(1.6, u * 0.095);
    let pips = "";
    const half = (n, ox, oy) => {
      for (const [x, y] of PIPS[n]) {
        const px = vertical ? x : y;
        const py = vertical ? y : 1 - x;
        pips += `<circle class="pip" cx="${(ox + px * u).toFixed(2)}" cy="${(oy + py * u).toFixed(2)}" r="${pr.toFixed(2)}"/>`;
      }
    };
    half(a, 0, 0);
    if (vertical) half(b, 0, u); else half(b, u, 0);
    const bar = vertical
      ? `<line class="bar" x1="${u * 0.18}" y1="${u}" x2="${u * 0.82}" y2="${u}" stroke-width="1.3"/>`
      : `<line class="bar" x1="${u}" y1="${u * 0.18}" x2="${u}" y2="${u * 0.82}" stroke-width="1.3"/>`;
    body = `<rect class="face" x="0.75" y="0.75" width="${w - 1.5}" height="${h - 1.5}" rx="${r}" stroke-width="1.5"/>${bar}${pips}`;
  }
  return `<svg class="tile ${cls}" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" aria-hidden="true">${body}</svg>`;
}

// "a-b" to [a, b].
function parseTile(text) {
  const [a, b] = text.split("-").map(Number);
  return [a, b];
}

// A tile chip for text: the tile drawn small, horizontal.
function chip(text, u = 12) {
  const [a, b] = parseTile(text);
  return `<span class="chip" title="${a}-${b}">${tileSVG(a, b, { u })}</span>`;
}
