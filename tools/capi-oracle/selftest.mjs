// Self-test of the oracle tooling (M1-03): the bundle loads with the needed
// exports; fixedDealRng makes Capi deal any wanted order in both modes; and
// the copied mulberry32 reproduces the whole-suit deals of Capi's fuzz
// (invariants.fuzz.test.ts:456-460). Prints one summary line; exit 0 on pass.

import { loadCapi, mulberry32, ALL_TILES, tileIndex, dealFixedRound, seatsOf } from "./lib.mjs";

const capi = await loadCapi();
const errors = [];
for (const name of ["createInitialState", "applyMove", "startNewRound", "validateMove", "getTeam"]) {
  if (typeof capi[name] !== "function") errors.push(`bundle lacks ${name}`);
}

const key = (t) => tileIndex(t);
let deals = 0;
for (const is2v2 of [true, false]) {
  for (let k = 0; k < 500; k++) {
    // A wanted order from a seeded Fisher-Yates of our own.
    const rng = mulberry32(700000 + k + (is2v2 ? 0 : 5000));
    const order = ALL_TILES.map((t) => [...t]);
    for (let i = order.length - 1; i > 0; i--) {
      const j = Math.floor(rng() * (i + 1));
      [order[i], order[j]] = [order[j], order[i]];
    }
    const leader = seatsOf(is2v2)[k % (is2v2 ? 4 : 2)];
    const state = dealFixedRound(capi, { is2v2, targetScore: 100, leader, order });
    const got = [...seatsOf(is2v2).flatMap((s) => state.hands[s]), ...state.boneyard].map(key);
    if (got.join(",") !== order.map(key).join(",")) {
      errors.push(`${is2v2 ? "2v2" : "1v1"} fixed deal ${k} differs`);
    }
    if (state.currentTurn !== leader || state.board.length !== 0) errors.push(`fixed deal ${k}: wrong leader or board`);
    deals++;
  }
}

let wholeSuit = 0;
for (const seed of [59313, 83322, 282144, 645301]) {
  const state = capi.createInitialState({ mode: "live", theme: "colmado", is2v2: true, targetScore: 100, rng: mulberry32(seed) });
  const sixes = state.hands[state.starterThisRound].filter((t) => t[0] === 6 || t[1] === 6).length;
  if (sixes === 6 && state.board.length === 1 && key(state.board[0]) === 27) wholeSuit++;
  else errors.push(`seed ${seed}: the 6-6 holder keeps ${sixes} sixes, want 6`);
}

for (const e of errors.slice(0, 10)) console.log("ERROR " + e);
console.log(
  `selftest: ${errors.length ? "FAIL" : "PASS"} bundle exports ok; fixedDealRng reproduced ${deals - errors.filter((e) => e.includes("fixed deal")).length} of ${deals} deals (both modes); whole-suit seeds ${wholeSuit} of 4`
);
process.exit(errors.length ? 1 : 0);
