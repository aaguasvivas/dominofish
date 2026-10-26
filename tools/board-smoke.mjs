// Web board smoke test (M11-01): loads the wasm module embedded in the built
// page web/dist/index.html, the same bytes the browser runs, and plays full
// games through the board requests (docs/FORMATS.md, Board requests): every
// seat a bot, an analysis every few moves, checks on every reply.
// Usage: node tools/board-smoke.mjs <games per mode>

import { readFileSync } from "node:fs";

const games = Number(process.argv[2] || 2);
const page = readFileSync("web/dist/index.html", "utf8");
const b64 = page.match(/const DOMINOFISH_WASM = "([A-Za-z0-9+/=]+)";/)[1];
const { instance } = await WebAssembly.instantiate(Buffer.from(b64, "base64"), {});
const x = instance.exports;

function call(text) {
  const bytes = new TextEncoder().encode(text);
  const p = x.board_alloc(bytes.length);
  new Uint8Array(x.memory.buffer, p, bytes.length).set(bytes);
  const r = BigInt.asUintN(64, x.board_request(p, bytes.length));
  const rp = Number(r >> 32n), rl = Number(r & 0xffffffffn);
  const out = new TextDecoder().decode(new Uint8Array(x.memory.buffer, rp, rl).slice());
  x.board_free(rp, rl);
  return out;
}
function ok(text) {
  const out = call(text);
  if (!out.startsWith("ok\n")) throw new Error(`${text.split("\n")[0]}: ${out}`);
  return out.slice(3);
}

let moves = 0, rounds = 0, analyses = 0, finished = 0;
for (const mode of ["2v2", "1v1"]) {
  for (let g = 0; g < games; g++) {
    let record = ok(`new ${mode} 100 ${1000 + g}`);
    for (;;) {
      const v = JSON.parse(ok(`view\n${record}`));
      if (v.mode !== mode) throw new Error(`view mode ${v.mode}`);
      const tiles = v.hands.reduce((n, h) => n + h.length, 0) + v.line.length + v.boneyard;
      if (tiles !== 28) throw new Error(`${tiles} tiles in view`);
      if (v.phase === "finished") { finished++; break; }
      if (v.phase === "round_over") {
        if (!v.result || !v.result.kind) throw new Error("round over without a result");
        record = ok(`next ${7 * moves + 1}\n${record}`);
        rounds++;
        continue;
      }
      if (v.legal.length === 0) throw new Error("no legal move in play");
      if (moves % 15 === 0) {
        const a = JSON.parse(ok(`analyze\n${record}`));
        if (a.moves.length !== v.legal.length) throw new Error("analysis does not cover every legal move");
        const best = Math.max(...a.moves.map((m) => (v.turn % 2 === 0 ? m.value : -m.value)));
        if (best !== (v.turn % 2 === 0 ? a.value : -a.value)) throw new Error("the position value is not the best move's value");
        analyses++;
      }
      const mv = ok(`bot rule ${moves}\n${record}`);
      if (!v.legal.some((m) => m.move === mv)) throw new Error(`bot move ${mv} is not legal`);
      record = ok(`play ${mv}\n${record}`);
      moves++;
    }
  }
}
for (const bad of ["", "new 3v3 100 1", "view\nnot a record", "fly\n" + ok("new 2v2 100 1")]) {
  if (!call(bad).startsWith("error ")) throw new Error(`accepted: ${JSON.stringify(bad)}`);
}
console.log(`board-smoke: games ${finished} (${games} per mode) rounds ${rounds + finished} moves ${moves} analyses ${analyses} errors 0`);
