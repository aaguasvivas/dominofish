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

// ---------------------------------------------------------------- traces

export const maskOf = (tiles) => tiles.reduce((m, t) => m | (1 << tileIndex(t)), 0) >>> 0;
export const tileText = (t) => `${t[0]}-${t[1]}`;

// The canonical state compared after every move (docs/RULES.md, Canonical
// state; docs/FORMATS.md, Golden traces).
export function canon(state) {
  if (state.consecutivePasses !== state.passesSinceLastPlay) {
    throw new Error("Capi's two pass counters differ");
  }
  const phase = { playing: "P", round_over: "R", finished: "F" }[state.phase];
  const hands = SEATS_2V2.map((s) => maskOf(state.hands[s] ?? []).toString(16));
  const ends = state.board.length ? [state.board[0][0], state.board[state.board.length - 1][1]] : ["-", "-"];
  return [
    phase,
    state.scores[0],
    state.scores[1],
    state.currentTurn,
    state.starterThisRound,
    state.lastPlayedBy ?? "-",
    state.passesSinceLastPlay,
    ...hands,
    maskOf(state.board).toString(16),
    ...ends,
    state.boneyard.length,
  ].join(" ");
}

export const intentText = (i) => (i.type === "play" ? `play ${tileText(i.tile)} ${i.end}` : i.type);

// Capi's callout mapped to DominoFish's typed event (docs/RULES.md, L4).
export function eventText(next, result) {
  const p = result.calloutPayload;
  switch (result.callout) {
    case undefined:
      return "-";
    case "domino":
      return `domino ${next.lastPlayedBy} ${p.pipsAwarded}`;
    case "capicua":
      return `capicua ${next.lastPlayedBy} ${p.pipsAwarded + p.capicuaBonus}`;
    case "trancao":
      return `tranque ${p.blockerSeat} ${p.rivalSeat} ${p.blockerPips} ${p.rivalPips} ${p.winnerSeat} ${p.pts}`;
    case "veinticinco":
      return `${p.salida ? "salida" : "corrido"} ${p.winningTeam}`;
    default:
      throw new Error(`unknown callout ${result.callout}`);
  }
}

// Every intent Capi accepts from the seat on turn: each tile in both
// orientations on both ends, then draw and pass.
export function acceptedIntents(capi, state) {
  const seat = state.currentTurn;
  const out = [];
  for (const [a, b] of state.hands[seat]) {
    const orientations = a === b ? [[a, b]] : [[a, b], [b, a]];
    for (const tile of orientations) {
      for (const end of ["left", "right"]) out.push({ type: "play", tile, end });
    }
  }
  out.push({ type: "draw" }, { type: "pass" });
  return out.filter((i) => capi.validateMove(state, seat, i) === null);
}

export const pick = (arr, rng) => arr[Math.floor(rng() * arr.length)];

// Policies. "raw": uniform over every accepted intent, so leads go left and
// right in both orientations. "capi-fuzz": the policy of Capi's fuzz
// (invariants.fuzz.test.ts:57-71, 109-114), which leads only with end right.
export const POLICIES = {
  raw: (capi, state, rng) => pick(acceptedIntents(capi, state), rng),
  "capi-fuzz": (capi, state, rng) => {
    const hand = state.hands[state.currentTurn] ?? [];
    const plays = [];
    if (state.board.length === 0) {
      for (const tile of hand) plays.push({ type: "play", tile, end: "right" });
    } else {
      const left = state.board[0][0];
      const right = state.board[state.board.length - 1][1];
      for (const tile of hand) {
        if (tile[0] === left || tile[1] === left) plays.push({ type: "play", tile, end: "left" });
        if (tile[0] === right || tile[1] === right) plays.push({ type: "play", tile, end: "right" });
      }
    }
    if (plays.length > 0) return pick(plays, rng);
    if (!state.is2v2 && state.boneyard.length > 0) return { type: "draw" };
    return { type: "pass" };
  },
};

// Wraps a deal rng and rebuilds each deal's order from the 27 values Capi's
// shuffle (reducer.ts:51-58) drew, so a trace records the full deal, the
// round-1 automatic tile included.
export function dealRecorder(rng) {
  let values = [];
  return {
    rng: () => {
      const v = rng();
      values.push(v);
      return v;
    },
    takeOrder() {
      if (values.length !== 27) throw new Error(`a deal drew ${values.length} values, want 27`);
      const out = ALL_TILES.map((t) => [...t]);
      let k = 0;
      for (let i = out.length - 1; i > 0; i--) {
        const j = Math.floor(values[k++] * (i + 1));
        [out[i], out[j]] = [out[j], out[i]];
      }
      values = [];
      return out;
    },
  };
}

// Checks a rebuilt deal order against the state Capi dealt from it.
export function checkDeal(state, order, roundOne) {
  const seats = seatsOf(state.is2v2);
  const keys = (ts) => ts.map(tileIndex).join(",");
  seats.forEach((seat, k) => {
    let want = order.slice(7 * k, 7 * k + 7);
    if (roundOne && seat === state.starterThisRound) {
      want = want.filter((t) => tileIndex(t) !== tileIndex(state.board[0]));
    }
    if (keys(want) !== keys(state.hands[seat])) throw new Error(`rebuilt deal differs for seat ${seat}`);
  });
  if (keys(order.slice(7 * seats.length)) !== keys(state.boneyard)) throw new Error("rebuilt boneyard differs");
}

// The rule paths of docs/RULES.md (Rule paths) that one transition reaches.
export function pathsOf(capi, prev, seat, intent, next, result, round) {
  const out = [];
  const is2v2 = prev.is2v2;
  const callout = result.callout;
  const p = result.calloutPayload;
  const ends = (s) => (s.board.length ? [s.board[0][0], s.board[s.board.length - 1][1]] : [-1, -1]);
  const [L, R] = ends(prev);
  const team = (s) => capi.getTeam(s, is2v2);
  const salidaShape =
    is2v2 && prev.board.length === 1 && prev.lastPlayedBy === prev.starterThisRound && prev.passesSinceLastPlay === 1;
  const partner = (s) => SEATS_2V2[(SEATS_2V2.indexOf(s) + 2) % 4];
  if (intent.type === "play") {
    const [a, b] = intent.tile;
    if (prev.board.length === 0) {
      if (intent.end === "left") out.push("lead-left");
      if (a > b) out.push("lead-flipped");
    } else {
      if (a !== b && ((a === L && b === R) || (a === R && b === L)) && L !== R) out.push("play-both-ends");
      if (L === R && intent.end === "right") out.push("play-equal-ends-right");
    }
    if (salidaShape && seat === partner(prev.starterThisRound)) {
      out.push(callout === "veinticinco" ? "salida-paid" : "salida-capped");
    }
    if (callout === "domino" || callout === "capicua") {
      if (callout === "capicua") out.push(a === b ? "capicua-double" : L === R ? "capicua-equal-ends" : "capicua");
      else out.push(a === b ? "domino-double-one-end" : "domino");
      if (capi.isBoardLocked(next)) out.push("domino-on-locking-tile");
    }
    if (callout === "trancao") {
      if (p.blockerPips === p.rivalPips) {
        out.push("tranque-tie");
        if (p.winnerSeat !== p.blockerSeat && p.winnerSeat !== p.rivalSeat) out.push("tranque-tie-opener-not-compared");
      } else {
        out.push(p.winnerSeat === p.blockerSeat ? "tranque-blocker-wins" : "tranque-rival-wins");
      }
      if (!is2v2 && next.boneyard.length > 0) out.push("tranque-with-boneyard");
    }
  }
  if (intent.type === "draw") {
    const gained = next.hands[seat].length - prev.hands[seat].length;
    const fit = capi.hasLegalPlay(next.hands[seat], next.board);
    if (fit) out.push("draw-fit");
    if (gained >= 2) out.push("draw-several");
    if (!fit && next.boneyard.length === 0) out.push("draw-exhausted");
  }
  if (intent.type === "pass") {
    if (!is2v2) out.push("pass-1v1");
    const forcer = prev.lastPlayedBy;
    const cycle = is2v2 && forcer !== null && next.currentTurn === forcer && prev.passesSinceLastPlay + 1 === 3;
    const paid = callout === "veinticinco";
    if (cycle) out.push(paid ? "pase-corrido-paid" : "pase-corrido-capped");
    if (paid) {
      round.corridos += 1;
      if (round.corridos === 2) out.push("pase-corrido-twice");
      if (prev.board.length === 1 && forcer === prev.starterThisRound) out.push("salida-cancelled-then-corrido");
    }
    if (salidaShape && seat === partner(prev.starterThisRound)) out.push("salida-cancelled");
  }
  if (callout === "veinticinco" || (intent.type === "pass" && out.includes("pase-corrido-capped")) || out.includes("salida-capped")) {
    const t = callout === "veinticinco" ? p.winningTeam : intent.type === "pass" ? team(prev.lastPlayedBy) : team(prev.starterThisRound);
    const short = prev.targetScore - prev.scores[t];
    if (callout === "veinticinco" && short === 26) out.push("bonus-26-short");
    if (callout !== "veinticinco" && short === 25) out.push("bonus-25-short");
  }
  if (next.phase !== "playing" && prev.phase === "playing") {
    const t = p.winningTeam;
    if (next.phase === "finished") {
      if (callout === "trancao") out.push("game-won-by-tranque");
      else out.push("game-won-by-domino");
      if (callout === "capicua" && next.scores[t] - p.capicuaBonus < next.targetScore) out.push("game-won-by-capicua-bonus");
      if (next.scores[t] === next.targetScore) out.push("game-ends-exactly-at-target");
    } else if (next.scores[t] === next.targetScore - 1) {
      out.push("round-ends-one-short");
    }
  }
  return out;
}

// The opening path of a round-1 state.
export function openingPath(state) {
  const [a, b] = state.board[0];
  return a === 6 && b === 6 ? "open-double-66" : a === b ? "open-double-lower" : "open-no-double";
}
