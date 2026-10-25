// Shared helpers for the Capi oracle scripts. No npm dependencies; run with
// /Users/Adelson/.nvm/versions/node/v20.20.0/bin/node after build.sh.

import { readFileSync } from "node:fs";

export const PIN = "1d8868a26ce930f478f98e64060c06b1d1a2c1f4";

// Capi's engine bundled by build.sh from the pinned commit.
export async function loadCapi() {
  const pin = readFileSync(new URL("./build/PIN", import.meta.url), "utf8").trim();
  if (pin !== PIN) throw new Error(`oracle bundle is from ${pin}, want ${PIN}; run tools/capi-oracle/build.sh`);
  return import(new URL("./build/capi-engine.mjs", import.meta.url));
}

// mulberry32, copied from Capi's packages/engine/__tests__/invariants.fuzz.test.ts:17-26
// at the pin with the type annotations removed (docs/FORMATS.md).
export function mulberry32(seed) {
  let a = seed >>> 0;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// The 28 tiles in Capi's order, low pip first (docs/RULES.md R1).
export const ALL_TILES = (() => {
  const out = [];
  for (let a = 0; a <= 6; a++) for (let b = a; b <= 6; b++) out.push([a, b]);
  return out;
})();

export const tileIndex = (t) => {
  const [a, b] = t[0] <= t[1] ? t : [t[1], t[0]];
  return a * 7 - (a * (a - 1)) / 2 + (b - a);
};

// An rng that makes Capi's shuffle (reducer.ts:51-58) produce `order`, a
// list of 28 tiles: for i from 27 down to 1 it returns (j + 0.5) / (i + 1),
// where j is the current index of the tile wanted at position i.
export function fixedDealRng(order) {
  const current = ALL_TILES.map(tileIndex);
  const wanted = order.map(tileIndex);
  let i = 27;
  return () => {
    if (i < 1) throw new Error("fixedDealRng called more than 27 times");
    const j = current.indexOf(wanted[i]);
    [current[i], current[j]] = [current[j], current[i]];
    const r = (j + 0.5) / (i + 1);
    i -= 1;
    return r;
  };
}

export const SEATS_2V2 = ["n", "e", "s", "w"];
export const SEATS_1V1 = ["n", "s"];
export const seatsOf = (is2v2) => (is2v2 ? SEATS_2V2 : SEATS_1V1);

// A round dealt from a fixed tile order with `leader` to lead on an empty
// board: startNewRound on a round_over state whose last callout is a domino
// by the leader (as invariants.fuzz.test.ts:488-499 does).
export function dealFixedRound(capi, { is2v2, targetScore, scores, leader, order }) {
  const base = capi.createInitialState({ mode: "turn_based", theme: "patio", is2v2, targetScore, rng: mulberry32(1) });
  const ended = {
    ...base,
    scores: scores ?? [0, 0],
    phase: "round_over",
    lastCallout: "domino",
    lastCalloutPayload: { winningTeam: capi.getTeam(leader, is2v2), team0Pips: 0, team1Pips: 0 },
    lastPlayedBy: leader,
  };
  return capi.startNewRound(ended, ended.players, fixedDealRng(order));
}

// The 28 tiles of a dealt state in Capi's deal layout: hands in seat order,
// then the boneyard (docs/RULES.md R3). Valid at the deal, before any move.
export function dealOrder(state) {
  const order = [];
  for (const seat of seatsOf(state.is2v2)) order.push(...state.hands[seat]);
  order.push(...state.boneyard);
  return order;
}
