// Edge-case scenarios (M1-05): one named round per edge case of
// capi-rules-spec[2] and per published deal (digest 3.6), every value
// produced by Capi's engine. A scenario is a fixed deal with a script, a
// round found by searching seeded deals for a predicate, or a found round
// replayed with other start scores. Writes fixtures/edges.txt
// (docs/FORMATS.md, Edge scenarios).
// Usage: node tools/capi-oracle/edges.mjs [out-file]

import { writeFileSync } from "node:fs";
import {
  PIN,
  loadCapi,
  mulberry32,
  ALL_TILES,
  tileIndex,
  tileText,
  canon,
  intentText,
  eventText,
  POLICIES,
  fixedDealRng,
  dealFixedRound,
  seatsOf,
} from "./lib.mjs";

const outFile = process.argv[2] ?? "fixtures/edges.txt";
const capi = await loadCapi();

const T = (s) => {
  const [a, b] = s.split("-").map(Number);
  return [Math.min(a, b), Math.max(a, b)];
};
const key = tileIndex;
const pipsOf = (hand) => hand.reduce((n, t) => n + t[0] + t[1], 0);
const ends = (s) => (s.board.length ? [s.board[0][0], s.board[s.board.length - 1][1]] : [-1, -1]);
const has = (t, p) => t[0] === p || t[1] === p;
const partner = (s) => ["n", "e", "s", "w"][(["n", "e", "s", "w"].indexOf(s) + 2) % 4];

function shuffledOrder(seed) {
  const rng = mulberry32(seed);
  const order = ALL_TILES.map((t) => [...t]);
  for (let i = order.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1));
    [order[i], order[j]] = [order[j], order[i]];
  }
  return order;
}

// A full 28-tile order: the given tiles at the start of each listed seat's
// hand (or of the boneyard), the rest from a seeded shuffle filling the gaps.
function orderWith(is2v2, parts, seed) {
  const slots = seatsOf(is2v2).length;
  const fixed = new Map();
  for (const [where, tiles] of Object.entries(parts)) {
    const base = where === "boneyard" ? 7 * slots : 7 * seatsOf(is2v2).indexOf(where);
    tiles.forEach((t, k) => fixed.set(base + k, T(t)));
  }
  const used = new Set([...fixed.values()].map(key));
  const rest = shuffledOrder(seed).filter((t) => !used.has(key(t)));
  return Array.from({ length: 28 }, (_, i) => fixed.get(i) ?? rest.shift());
}

// Resolves a script step ("pass", "draw", "6-6", "0-4 right") at a state.
function resolve(state, step) {
  if (step === "pass" || step === "draw") return { type: step };
  const [tileText_, endText] = step.split(" ");
  // The tile keeps the orientation written in the step, which sets the
  // board on a lead (docs/RULES.md R10).
  const tile = tileText_.split("-").map(Number);
  if (!state.hands[state.currentTurn].some((t) => key(t) === key(tile))) {
    throw new Error(`script: ${tileText_} not in ${state.currentTurn}'s hand`);
  }
  if (endText) return { type: "play", tile, end: endText };
  const ok = ["left", "right"].filter((end) => capi.validateMove(state, state.currentTurn, { type: "play", tile, end }) === null);
  const [l, r] = ends(state);
  if (ok.length === 2 && state.board.length > 0 && l !== r) throw new Error(`script: ${tileText_} fits both ends ${l}|${r}; name the end`);
  if (ok.length === 0) throw new Error(`script: ${tileText_} does not fit`);
  return { type: "play", tile, end: ok[0] };
}

// Plays one round in Capi. Returns the trace lines and the transitions.
function playRound({ is2v2, target, round1, leader, scores, order, script, seed }) {
  let state = round1
    ? capi.createInitialState({ mode: "live", theme: "patio", is2v2, targetScore: target, rng: fixedDealRng(order) })
    : dealFixedRound(capi, { is2v2, targetScore: target, scores, leader, order });
  const lines = [`G ${is2v2 ? "2v2" : "1v1"} ${target} ${seed} ${script ? "script" : "raw"}`];
  if (!round1) lines.push(`L ${leader} ${state.scores[0]} ${state.scores[1]}`);
  lines.push(`D ${order.map(key).join(",")}`);
  if (round1) lines.push(`O ${state.starterThisRound} ${tileText(state.board[0])}`);
  lines.push(`S ${canon(state)}`);
  const init = state;
  const trs = [];
  const choice = mulberry32(seed ^ 0x9e3779b9);
  const intents = [];
  for (let k = 0; state.phase === "playing"; k++) {
    if (k > 400) throw new Error("round did not end");
    const intent = script && k < script.length ? resolve(state, script[k]) : POLICIES.raw(capi, state, choice);
    const seat = state.currentTurn;
    const result = capi.applyMove(state, seat, intent);
    if (!result.success) throw new Error(`Capi rejected ${intentText(intent)}: ${result.error}`);
    trs.push({ prev: state, seat, intent, next: result.newState, result });
    intents.push(intent.type === "play" ? `${tileText(intent.tile)} ${intent.end}` : intent.type);
    state = result.newState;
    lines.push(`M ${seat} ${intentText(intent)} | ${eventText(state, result)} | ${canon(state)}`);
  }
  return { lines, init, trs, intents };
}

// Rejection checks: intents Capi refuses at a state, written as X lines
// before the move made there.
function withRejections(run, at, intents) {
  const tr = run.trs[at];
  const out = [];
  for (const i of intents) {
    if (capi.validateMove(tr.prev, tr.seat, i) === null) throw new Error(`expected Capi to reject ${intentText(i)}`);
    out.push(`X ${tr.seat} ${intentText(i)} | rejected`);
  }
  const headerLen = run.lines.findIndex((l) => l.startsWith("M "));
  run.lines.splice(headerLen + at, 0, ...out);
  return run;
}

const callout = (tr) => tr.result.callout;
const pay = (tr) => tr.result.calloutPayload;
const rounds = [];

// Finds the first seed from `base` whose round satisfies `test`.
function find(spec, test, base, tries = 300000) {
  for (let seed = base; seed < base + tries; seed++) {
    const leaders = seatsOf(spec.is2v2);
    const leader = spec.leader ?? leaders[seed % leaders.length];
    const order = spec.order ?? shuffledOrder(seed);
    const run = playRound({ ...spec, leader, order, seed, scores: spec.scores ?? [0, 0] });
    const at = test(run);
    if (at !== false && at !== undefined) return { run, at, seed, leader, order };
  }
  throw new Error(`no seed in ${base}..${base + tries} satisfies the predicate`);
}
const firstIndex = (run, f) => {
  const i = run.trs.findIndex((tr, k) => f(tr, k, run));
  return i < 0 ? false : i;
};

function add(name, cite, run) {
  rounds.push([`C ${name} ${cite}`, ...run.lines, "Z"]);
}

const two = { is2v2: true, target: 100 };
const one = { is2v2: false, target: 100 };

// ------------------------------------------------------------- opening
add("start-1v1-starter-has-6", "reducer.test.ts:11", find({ ...one, round1: true }, (r) => 0, 1000).run);
add("start-1v1-boneyard-14", "reducer.test.ts:37", find({ ...one, round1: true }, (r) => 0, 1100).run);
add("start-2v2-no-boneyard", "reducer.test.ts:664", find({ ...two, round1: true }, (r) => 0, 1200).run);
add(
  "turn-order-n-e-s-w",
  "reducer.test.ts:680",
  find({ ...two, leader: "n" }, (r) => r.trs.slice(0, 4).every((tr, k) => tr.intent.type === "play" && tr.seat === "nesw"[k]) && 0, 1300).run
);
add(
  "open-1v1-lower-double",
  "reducer.ts:135-145",
  find({ ...one, round1: true }, (r) => (r.init.board[0][0] === r.init.board[0][1] && r.init.board[0][0] < 6 ? 0 : false), 1400).run
);
const noDouble = ["0-1", "0-2", "1-2", "0-3", "1-3", "2-3", "0-4", "1-4", "2-4", "3-4", "0-5", "1-5", "2-5", "3-5"];
add(
  "open-1v1-no-double",
  "reducer.ts:146-159",
  playRound({ ...one, round1: true, order: orderWith(false, { n: noDouble.slice(0, 7), s: noDouble.slice(7) }, 1500), seed: 1500 })
);
add(
  "open-1v1-no-double-tie-to-earlier-seat",
  "reducer.ts:146-159",
  playRound({
    ...one,
    round1: true,
    order: orderWith(false, { n: ["0-1", "0-2", "1-2", "3-6", "0-3", "1-3", "2-3"], s: ["0-4", "1-4", "2-4", "4-5", "0-5", "1-5", "2-5"] }, 1600),
    seed: 1600,
  })
);
add(
  "open-1v1-no-double-first-in-hand-order",
  "reducer.ts:98-109",
  playRound({
    ...one,
    round1: true,
    order: orderWith(false, { n: ["0-1", "4-5", "1-2", "3-6", "0-3", "1-3", "2-3"], s: ["0-4", "1-4", "2-4", "0-5", "1-5", "2-5", "3-4"] }, 1700),
    seed: 1700,
  })
);

// ---------------------------------------------------------------- draw
add(
  "draw-stops-at-first-fit",
  "reducer.test.ts:1357",
  find(one, (r) => firstIndex(r, (tr) => tr.intent.type === "draw" && tr.next.hands[tr.seat].length - tr.prev.hands[tr.seat].length >= 2 && capi.hasLegalPlay(tr.next.hands[tr.seat], tr.next.board)) !== false && 0, 2000).run
);
add(
  "draw-exhausts-then-pass",
  "reducer.test.ts:1387",
  find(one, (r) => firstIndex(r, (tr, k) => tr.intent.type === "draw" && tr.next.boneyard.length === 0 && !capi.hasLegalPlay(tr.next.hands[tr.seat], tr.next.board) && r.trs[k + 1]?.intent.type === "pass" && r.trs[k + 1].seat === tr.seat) !== false && 0, 2100).run
);
add(
  "drawn-tiles-count-in-tranque",
  "reducer.test.ts:1427",
  find(one, (r) => firstIndex(r, (tr, k) => tr.intent.type === "draw" && tr.next.hands[tr.seat].length - tr.prev.hands[tr.seat].length >= 2 && r.trs[k + 1] && callout(r.trs[k + 1]) === "trancao" && r.trs[k + 1].seat === tr.seat) !== false && 0, 2200).run
);
{
  const f = find(two, (r) => firstIndex(r, (tr) => tr.intent.type === "pass"), 2300);
  add("reject-draw-in-2v2", "validate.test.ts:118", withRejections(f.run, f.at, [{ type: "draw" }]));
}
{
  const f = find(one, (r) => firstIndex(r, (tr) => tr.intent.type === "draw"), 2400);
  add("reject-pass-with-boneyard", "validate.test.ts:70", withRejections(f.run, f.at, [{ type: "pass" }]));
}

// ---------------------------------------------------------------- lock
add("lock-with-boneyard-tiles", "reducer.test.ts:314", find(one, (r) => firstIndex(r, (tr) => callout(tr) === "trancao" && tr.next.boneyard.length > 0) !== false && 0, 3000).run);
const boneyardKeepsOpen = (r) =>
  firstIndex(r, (tr) => {
    if (tr.intent.type !== "play" || tr.next.phase !== "playing") return false;
    const [l, rr] = ends(tr.next);
    const fits = (t) => has(t, l) || has(t, rr);
    return !["n", "s"].some((s) => tr.next.hands[s].some(fits)) && tr.next.boneyard.some(fits);
  }) !== false && 0;
add("no-lock-while-boneyard-fits", "reducer.test.ts:362", find(one, boneyardKeepsOpen, 3100).run);
add("boneyard-tile-keeps-board-open", "validate.test.ts:155", find(one, boneyardKeepsOpen, 3200).run);
add("empty-table-never-locked", "validate.test.ts:160", find(two, (r) => 0, 3300).run);

// ------------------------------------------------------------- tranque
add("tranque-on-locking-tile", "reducer.test.ts:734", find(two, (r) => firstIndex(r, (tr) => callout(tr) === "trancao") !== false && 0, 4000).run);
add(
  "tranque-blocker-hand-decides",
  "reducer.test.ts:760",
  find(two, (r) => firstIndex(r, (tr) => {
    if (callout(tr) !== "trancao" || pay(tr).winnerSeat !== tr.seat) return false;
    const mine = capi.getTeam(tr.seat, true);
    return capi.teamPips(tr.next, mine) > capi.teamPips(tr.next, 1 - mine);
  }) !== false && 0, 4100).run
);
add("tranque-w-blocks-n-compares", "reducer.test.ts:813", find(two, (r) => firstIndex(r, (tr) => callout(tr) === "trancao" && tr.seat === "w") !== false && 0, 4200).run);
const tie = (opener, blocker) => (r) =>
  firstIndex(r, (tr) => callout(tr) === "trancao" && pay(tr).blockerPips === pay(tr).rivalPips && tr.prev.starterThisRound === opener && tr.seat === blocker) !== false && 0;
add("tranque-tie-opener-n-blocker-s-rival-w", "rules.salida-and-tie.test.ts:364", find({ ...two, leader: "n" }, tie("n", "s"), 4300).run);
add("tranque-tie-opener-w-blocker-n-rival-e", "rules.salida-and-tie.test.ts:393", find({ ...two, leader: "w" }, tie("w", "n"), 4400).run);
add("tranque-tie-1v1-opener", "rules.salida-and-tie.test.ts:409", find(one, (r) => firstIndex(r, (tr) => callout(tr) === "trancao" && pay(tr).blockerPips === pay(tr).rivalPips) !== false && 0, 4500).run);

// ------------------------------------------------------- domino vs lock
// Capi's test builds an unreachable state for a plain domino on a locking
// tile. From a fresh deal a lock needs equal ends, so the last tile fits both
// old ends and the ending is a capicua (docs/RULES.md, Edge cases).
add("going-out-on-locking-tile-is-domino", "reducer.test.ts:854", find(two, (r) => firstIndex(r, (tr) => (callout(tr) === "domino" || callout(tr) === "capicua") && capi.isBoardLocked(tr.next)) !== false && 0, 5000).run);
add(
  "capicua-6-6-on-6-6-locks",
  "reducer.test.ts:870",
  find(two, (r) => firstIndex(r, (tr) => callout(tr) === "capicua" && key(tr.intent.tile) === 27) !== false && 0, 5100).run
);
add(
  "tranque-both-ends-tile-not-last",
  "reducer.test.ts:605",
  find(two, (r) => firstIndex(r, (tr) => {
    if (callout(tr) !== "trancao") return false;
    const [l, rr] = ends(tr.prev);
    return has(tr.intent.tile, l) && has(tr.intent.tile, rr);
  }) !== false && 0, 5200).run
);

// ------------------------------------------------------------- capicua
const capicuaWith = (tile, endsWanted, end) => (r) =>
  firstIndex(r, (tr) => {
    if (tr.intent.type !== "play" || key(tr.intent.tile) !== key(T(tile))) return false;
    if (callout(tr) !== "domino" && callout(tr) !== "capicua") return false;
    const [l, rr] = ends(tr.prev);
    return [l, rr].sort().join() === endsWanted.slice().sort().join() && (!end || tr.intent.end === end);
  }) !== false && 0;
add("capicua-3-5-on-3-5-left", "reducer.test.ts:540", find(two, capicuaWith("3-5", [3, 5], "left"), 6000).run);
add("capicua-3-5-on-3-5-right", "reducer.test.ts:540", find(two, capicuaWith("3-5", [3, 5], "right"), 6100).run);
add("capicua-5-6-on-5-5", "reducer.test.ts:557", find(two, capicuaWith("5-6", [5, 5]), 6200).run);
add("capicua-2-2-on-2-2", "reducer.test.ts:563", find(two, capicuaWith("2-2", [2, 2]), 6300).run);
add("domino-2-2-on-2-5", "reducer.test.ts:579", find(two, capicuaWith("2-2", [2, 5]), 6400).run);
add("capicua-0-0-on-0-0", "reducer.test.ts:586", find(two, capicuaWith("0-0", [0, 0]), 6500).run);
{
  const f = find(two, (r) => firstIndex(r, (tr) => {
    const [l, rr] = ends(tr.prev);
    return l === 2 && rr === 2 && tr.prev.hands[tr.seat].some((t) => key(t) === key(T("4-4")));
  }), 6600);
  const tile = f.run.trs[f.at].prev.hands[f.run.trs[f.at].seat].find((t) => key(t) === key(T("4-4")));
  add("reject-4-4-on-2-2", "scoring.test.ts:213", withRejections(f.run, f.at, [{ type: "play", tile, end: "left" }, { type: "play", tile, end: "right" }]));
}
add("no-capicua-on-empty-table", "scoring.test.ts:224", find(one, (r) => 0, 6700).run);

// --------------------------------------------------------- pase corrido
add(
  "no-corrido-after-two-passes",
  "reducer.test.ts:967",
  find(two, (r) => firstIndex(r, (tr, k) => tr.intent.type === "play" && r.trs[k + 1]?.intent.type === "pass" && r.trs[k + 2]?.intent.type === "pass" && r.trs[k + 3]?.intent.type === "play") !== false && 0, 7000).run
);
add(
  "corrido-forcer-must-play",
  "reducer.test.ts:1879",
  find(two, (r) => firstIndex(r, (tr, k) => callout(tr) === "veinticinco" && !pay(tr).salida && r.trs[k + 1]?.intent.type === "play") !== false && 0, 7100).run
);
add(
  "corrido-twice-same-player",
  "reducer.test.ts:1898",
  find(two, (r) => {
    const forcers = r.trs.filter((tr) => callout(tr) === "veinticinco" && !pay(tr).salida).map((tr) => tr.prev.lastPlayedBy);
    return forcers.some((f, i) => forcers.indexOf(f) !== i) && 0;
  }, 7200).run
);
add(
  "corrido-domino-capicua-same-column",
  "reducer.test.ts:2021",
  find(two, (r) => {
    const last = r.trs[r.trs.length - 1];
    if (callout(last) !== "capicua") return false;
    const t = pay(last).winningTeam;
    return r.trs.some((tr) => callout(tr) === "veinticinco" && pay(tr).winningTeam === t) && 0;
  }, 7300).run
);
add("no-corrido-heads-up", "scoring.attribution.test.ts:106", find(one, (r) => firstIndex(r, (tr) => tr.intent.type === "pass") !== false && 0, 7400).run);

// --------------------------------------------------------------- salida
const salidaShape = (tr) => tr.prev.board.length === 1 && tr.prev.lastPlayedBy === tr.prev.starterThisRound && tr.prev.passesSinceLastPlay === 1 && tr.seat === partner(tr.prev.starterThisRound);
add("salida-round1-auto-6-6", "rules.salida-and-tie.test.ts:80", find({ ...two, round1: true }, (r) => firstIndex(r, (tr) => salidaShape(tr) && tr.intent.type === "play") !== false && 0, 8000).run);
const laterSalida = find(two, (r) => firstIndex(r, (tr) => salidaShape(tr) && tr.intent.type === "play" && callout(tr) === "veinticinco"), 8100);
add("salida-later-round", "rules.salida-and-tie.test.ts:128", laterSalida.run);
add(
  "salida-cancelled-fourth-seat-play-pays-nothing",
  "rules.salida-and-tie.test.ts:162",
  find(two, (r) => firstIndex(r, (tr, k) => salidaShape(tr) && tr.intent.type === "pass" && r.trs[k + 1]?.intent.type === "play") !== false && 0, 8200).run
);
add(
  "salida-cancelled-then-corrido-once",
  "rules.salida-and-tie.test.ts:182",
  playRound({ ...two, leader: "n", order: orderWith(true, { n: ["0-0", "0-1", "0-2", "0-3", "0-4", "0-5", "0-6"] }, 8300), script: ["0-0"], seed: 8300 })
);
add(
  "salida-later-pass-never-counts",
  "rules.salida-and-tie.test.ts:263",
  find(two, (r) => firstIndex(r, (tr) => tr.prev.board.length > 1 && tr.prev.lastPlayedBy === tr.prev.starterThisRound && tr.prev.passesSinceLastPlay === 1 && tr.seat === partner(tr.prev.starterThisRound) && tr.intent.type === "play") !== false && 0, 8400).run
);
add("salida-none-heads-up", "rules.salida-and-tie.test.ts:238", find(one, (r) => (r.trs[0]?.intent.type === "play" && r.trs[1]?.intent.type === "draw" ? 0 : false), 8500).run);

// Replays a found round from other start scores, as in Capi's tests that
// set the scores before the move.
const rescore = (f, scores, target = 100) =>
  playRound({ is2v2: f.run.init.is2v2, target, leader: f.leader, scores, order: f.order, script: f.run.intents, seed: f.seed });
const teamScores = (team, score) => (team === 0 ? [score, 0] : [0, score]);
{
  const salidaTeam = capi.getTeam(laterSalida.leader, true);
  add("salida-paid-at-74", "rules.salida-and-tie.test.ts:206", rescore(laterSalida, teamScores(salidaTeam, 74)));
  add("salida-plain-at-75", "rules.salida-and-tie.test.ts:206", rescore(laterSalida, teamScores(salidaTeam, 75)));
  add("salida-paid-26-short-of-200", "flow.endgame.test.ts:574", rescore(laterSalida, teamScores(salidaTeam, 174), 200));
  add("salida-plain-25-short-of-200", "flow.endgame.test.ts:574", rescore(laterSalida, teamScores(salidaTeam, 175), 200));
}

// ------------------------------------------------------------------ cap
{
  // A round whose only bonus is one pase corrido.
  const f = find(two, (r) => {
    const bonuses = r.trs.filter((tr) => callout(tr) === "veinticinco");
    return bonuses.length === 1 && !pay(bonuses[0]).salida ? 0 : false;
  }, 9000);
  const t = pay(f.run.trs.find((tr) => callout(tr) === "veinticinco")).winningTeam;
  add("corrido-paid-at-74", "reducer.test.ts:2112", rescore(f, teamScores(t, 74)));
  add("corrido-plain-at-75", "reducer.test.ts:2120", rescore(f, teamScores(t, 75)));
  add("corrido-plain-at-80", "reducer.test.ts:2135", rescore(f, teamScores(t, 80)));
  add("corrido-cap-hardening", "rules.hardening.test.ts:127", rescore(f, teamScores(t, 76)));
  add("corrido-paid-26-short-of-200", "flow.endgame.test.ts:574", rescore(f, teamScores(t, 174), 200));
  add("corrido-plain-25-short-of-200", "flow.endgame.test.ts:574", rescore(f, teamScores(t, 175), 200));
}

// --------------------------------------------------------- target edges
for (const [mode, spec] of [["1v1", one], ["2v2", two]]) {
  for (const kind of ["domino", "capicua", "trancao"]) {
    const f = find(spec, (r) => {
      const last = r.trs[r.trs.length - 1];
      return callout(last) === kind && !r.trs.some((tr) => callout(tr) === "veinticinco") && pay(last).pipsAwarded !== 0 ? 0 : false;
    }, 9100 + (mode === "2v2" ? 500 : 0) + ["domino", "capicua", "trancao"].indexOf(kind) * 100);
    const last = f.run.trs[f.run.trs.length - 1];
    const points = kind === "trancao" ? pay(last).pts : pay(last).pipsAwarded + (pay(last).capicuaBonus ?? 0);
    const t = pay(last).winningTeam;
    const name = kind === "trancao" ? "tranque" : kind;
    add(`edge-${mode}-${name}-one-short`, "flow.endgame.test.ts:548", rescore(f, teamScores(t, 100 - 1 - points)));
    add(`edge-${mode}-${name}-exact`, "flow.endgame.test.ts:548", rescore(f, teamScores(t, 100 - points)));
    if (kind === "capicua" && mode === "2v2") {
      add("capicua-wins-game", "reducer.test.ts:1537", rescore(f, teamScores(t, 100 - points + 1)));
    }
  }
}

// ------------------------------------------------------ published deals
// Parra and Trucco 2025 (digest 3.6): P1 to P4 are n, e, s, w; P1 leads a
// later round. Ends are named where a tile fits both.
const parra = (n, e, s, w, script, seed) =>
  playRound({ ...two, leader: "n", order: [n, e, s, w].flat().map(T), script, seed });
add(
  "parra-trucco-immortal-game",
  "digest 3.6",
  parra(
    ["1-1", "4-0", "3-1", "0-5", "0-0", "0-2", "0-6"],
    ["5-5", "6-5", "5-4", "5-3", "6-3", "4-3", "3-3"],
    ["1-0", "0-3", "5-1", "2-5", "2-3", "2-2", "1-2"],
    ["1-4", "2-4", "4-4", "4-6", "2-6", "1-6", "6-6"],
    ["1-1", "pass", "1-0 left", "1-4", "4-0 right", "pass", "0-3", "pass", "3-1", "pass", "5-1", "pass", "0-5 left", "pass", "pass", "pass", "0-0", "pass", "pass", "pass", "0-2", "pass", "2-5", "pass", "0-6"],
    9900
  )
);
add(
  "parra-trucco-minimal-tranque",
  "digest 3.6",
  parra(
    ["0-0", "0-3", "5-6", "6-6", "5-5", "4-4", "3-3"],
    ["0-1", "3-4", "6-0", "2-2", "1-1", "4-1", "6-4"],
    ["1-2", "4-0", "6-3", "5-4", "6-2", "3-2", "5-3"],
    ["2-0", "0-5", "6-1", "2-5", "5-1", "3-1", "4-2"],
    ["0-0", "0-1 right", "1-2", "2-0 right", "0-3", "3-4", "4-0 left", "0-5", "5-6", "6-0 left"],
    9901
  )
);
add(
  "parra-trucco-107",
  "digest 3.6",
  parra(
    ["0-0", "1-4", "0-5", "3-0", "2-0", "1-1", "1-2"],
    ["1-5", "1-6", "2-5", "2-6", "5-5", "5-6", "6-6"],
    ["0-1", "0-4", "0-6", "2-3", "1-3", "2-2", "2-4"],
    ["3-3", "3-4", "3-5", "3-6", "4-4", "4-5", "4-6"],
    ["0-0", "pass", "0-1 right", "pass", "1-4", "pass", "0-4 right", "pass", "0-5", "5-6", "6-0 left", "pass", "0-3", "pass", "3-2", "pass", "0-2 left"],
    9902
  )
);

const header = [
  `# DominoFish edge scenarios from Capi's engine at ${PIN}.`,
  `# Generated by tools/capi-oracle/edges.mjs; never edit by hand. Format: docs/FORMATS.md.`,
];
writeFileSync(outFile, [...header, ...rounds.flat()].join("\n") + "\n");
console.log(`edges: ${rounds.length} scenarios written to ${outFile}`);
