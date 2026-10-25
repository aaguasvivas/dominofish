// Lockstep differential fuzzer, Capi side (M1-07). Plays seeded random rounds
// through Capi's engine and streams, per move, Capi's canonical legal-move
// set before the move and a digest of the canonical state and event after it
// (docs/FORMATS.md, Differential fuzzer). `dominofish capi-fuzz` spawns this
// script, replays every move in Rust, and compares.
// Usage: node tools/capi-oracle/fuzz.mjs <1v1|2v2> <rounds> <seed>

import { loadCapi, mulberry32, ALL_TILES, tileIndex, fixedDealRng, dealFixedRound, seatsOf, pathsOf, openingPath } from "./lib.mjs";

const [modeArg, roundsArg, seedArg] = process.argv.slice(2);
const is2v2 = modeArg === "2v2";
const rounds = Number(roundsArg);
const seed = Number(seedArg) >>> 0;
const capi = await loadCapi();
const seats = seatsOf(is2v2);
const seatIndex = (s) => (s === null ? 4 : seats.indexOf(s));

// The digest both sides compute: word-wise FNV-1a over the canonical state
// and the event, then a final avalanche.
function digest(words) {
  let h = 0x811c9dc5;
  for (const w of words) h = Math.imul(h ^ (w >>> 0), 0x01000193) >>> 0;
  h ^= h >>> 16;
  h = Math.imul(h, 0x85ebca6b) >>> 0;
  h ^= h >>> 13;
  return h >>> 0;
}
const maskOf = (tiles) => tiles.reduce((m, t) => m | (1 << tileIndex(t)), 0) >>> 0;
function stateWords(s) {
  const phase = { playing: 0, round_over: 1, finished: 2 }[s.phase];
  const hands = [...seats.map((x) => maskOf(s.hands[x])), 0, 0].slice(0, 4);
  const empty = s.board.length === 0;
  return [
    phase,
    s.scores[0],
    s.scores[1],
    seatIndex(s.currentTurn),
    seatIndex(s.starterThisRound),
    seatIndex(s.lastPlayedBy),
    s.passesSinceLastPlay,
    ...hands,
    maskOf(s.board),
    empty ? 7 : s.board[0][0],
    empty ? 7 : s.board[s.board.length - 1][1],
    s.boneyard.length,
  ];
}
function eventWords(next, r) {
  const p = r.calloutPayload;
  switch (r.callout) {
    case undefined:
      return [0];
    case "domino":
      return [1, seatIndex(next.lastPlayedBy), p.pipsAwarded];
    case "capicua":
      return [2, seatIndex(next.lastPlayedBy), p.pipsAwarded + p.capicuaBonus];
    case "trancao":
      return [3, seatIndex(p.blockerSeat), seatIndex(p.rivalSeat), p.blockerPips, p.rivalPips, seatIndex(p.winnerSeat), p.pts];
    case "veinticinco":
      return [p.salida ? 5 : 4, p.winningTeam];
  }
  throw new Error(`unknown callout ${r.callout}`);
}

// Every accepted intent, and the canonical set as two 32-bit words: bit
// 2 * tile + end (0 left, 1 right) for plays, 56 for draw, 57 for pass.
function accepted(s) {
  const seat = s.currentTurn;
  const out = [];
  for (const [a, b] of s.hands[seat]) {
    for (const tile of a === b ? [[a, b]] : [[a, b], [b, a]]) {
      for (const end of ["left", "right"]) out.push({ type: "play", tile, end });
    }
  }
  out.push({ type: "draw" }, { type: "pass" });
  return out.filter((i) => capi.validateMove(s, seat, i) === null);
}
function canonicalSet(s, intents) {
  let lo = 0;
  let hi = 0;
  const equal = s.board.length === 0 || s.board[0][0] === s.board[s.board.length - 1][1];
  for (const i of intents) {
    const bit = i.type === "draw" ? 56 : i.type === "pass" ? 57 : 2 * tileIndex(i.tile) + (equal || i.end === "left" ? 0 : 1);
    if (bit < 32) lo |= 1 << bit;
    else hi |= 1 << (bit - 32);
  }
  return [lo >>> 0, hi >>> 0];
}

const pick = (arr, rng) => arr[Math.floor(rng() * arr.length)];
const pips = (t) => t[0] + t[1];
const fitsBoard = (board, t) =>
  board.length === 0 || [board[0][0], board[board.length - 1][1]].some((e) => t[0] === e || t[1] === e);
// Oracle-side policies (they may read every hand): uniform over accepted
// intents; heaviest or lightest tile; and a blocker that leaves the next seat
// the fewest plays, which drives passes, bonuses, and locks.
const POLICY = {
  raw: (s, ok, rng) => pick(ok, rng),
  heavy: (s, ok, rng) => best(ok, (i) => pips(i.tile), rng),
  light: (s, ok, rng) => best(ok, (i) => -pips(i.tile), rng),
  blocker: (s, ok, rng) =>
    best(ok, (i) => {
      const board = capi.placeTileOnBoard(s.board, i.tile, i.end);
      const next = capi.getNextSeat(s.currentTurn, is2v2);
      return -s.hands[next].filter((t) => fitsBoard(board, t)).length;
    }, rng),
};
function best(ok, score, rng) {
  const plays = ok.filter((i) => i.type === "play");
  if (plays.length === 0) return ok[0];
  const top = Math.max(...plays.map(score));
  return pick(plays.filter((i) => score(i) === top), rng);
}

function shuffled(rng) {
  const order = ALL_TILES.map((t) => [...t]);
  for (let i = order.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1));
    [order[i], order[j]] = [order[j], order[i]];
  }
  return order;
}

const counts = new Map();
let out = [];
let moves = 0;
const flush = async () => {
  if (out.length === 0) return;
  const text = out.join("\n") + "\n";
  out = [];
  if (!process.stdout.write(text)) await new Promise((r) => process.stdout.once("drain", r));
};

for (let r = 0; r < rounds; r++) {
  const rng = mulberry32((seed ^ Math.imul(r + 1, 0x9e3779b9)) >>> 0);
  const target = rng() < 0.5 ? 100 : 200;
  const policy = pick(["raw", "raw", "heavy", "light", "blocker"], rng);
  const round1 = rng() < 0.25;
  let order = shuffled(rng);
  let state;
  let leader = pick(seats, rng);
  let script = null;
  let scores = [0, 0];
  if (round1) {
    state = capi.createInitialState({ mode: "live", theme: "patio", is2v2, targetScore: target, rng: fixedDealRng(order) });
  } else {
    // Start scores biased to the bonus cap and the target edges.
    const kind = rng();
    if (kind >= 0.4) scores = [Math.floor(rng() * target), Math.floor(rng() * target)];
    if (kind >= 0.7) scores[rng() < 0.5 ? 0 : 1] = target - pick([1, 2, 3, 10, 24, 25, 26, 27], rng);
    if (is2v2 && rng() < 0.002) {
      // The leader holds a whole suit and leads its double, so nobody can
      // follow: the salida is cancelled and the pase corrido pays once.
      const k = Math.floor(rng() * 7);
      const suit = ALL_TILES.filter((t) => t[0] === k || t[1] === k);
      const rest = order.filter((t) => t[0] !== k && t[1] !== k);
      const at = 7 * seats.indexOf(leader);
      order = [...rest.slice(0, at), ...suit, ...rest.slice(at)];
      script = { type: "play", tile: [k, k], end: rng() < 0.5 ? "left" : "right" };
    }
    state = dealFixedRound(capi, { is2v2, targetScore: target, scores, leader, order });
  }
  out.push(`R ${r} ${round1 ? "r1" : "L"} ${seatIndex(state.starterThisRound)} ${scores[0]} ${scores[1]} ${target} ${order.map(tileIndex).join(",")}`);
  if (round1) counts.set(openingPath(state), (counts.get(openingPath(state)) ?? 0) + 1);
  const round = { corridos: 0 };
  for (let step = 0; state.phase === "playing"; step++) {
    if (step > 400) throw new Error(`round ${r} did not end`);
    const ok = accepted(state);
    const [lo, hi] = canonicalSet(state, ok);
    const intent = step === 0 && script ? script : POLICY[policy](state, ok, rng);
    const seat = state.currentTurn;
    const res = capi.applyMove(state, seat, intent);
    if (!res.success) throw new Error(`round ${r}: Capi rejected its own accepted intent: ${res.error}`);
    for (const p of pathsOf(capi, state, seat, intent, res.newState, res, round)) counts.set(p, (counts.get(p) ?? 0) + 1);
    state = res.newState;
    const code = intent.type === "play" ? `${intent.tile[0]}${intent.tile[1]}${intent.end[0]}` : intent.type[0];
    out.push(`${code} ${lo.toString(16)} ${hi.toString(16)} ${digest([...stateWords(state), ...eventWords(state, res)]).toString(16)}`);
    moves++;
  }
  if (out.length > 4000) await flush();
}
for (const [path, n] of [...counts].sort()) out.push(`P ${path} ${n}`);
out.push(`END ${rounds} ${moves}`);
await flush();
