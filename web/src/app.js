// The DominoFish Board: you play seat n (bottom); bots play the other seats.
// The page keeps the game record (docs/FORMATS.md, Game records) and asks
// the engine for every view, move, hint, and analysis.

const HUMAN = 0;
const BOTS = ["random", "heaviest", "accompanied", "rule"];
const BOT_DELAY_MS = 550;

const $ = (id) => document.getElementById(id);
const store = {
  get(key, fallback) {
    try {
      const v = localStorage.getItem("dominofish." + key);
      return v === null ? fallback : JSON.parse(v);
    } catch { return fallback; }
  },
  set(key, value) {
    try { localStorage.setItem("dominofish." + key, JSON.stringify(value)); } catch { /* private mode */ }
  },
};

const app = {
  lang: store.get("lang", (navigator.language || "en").toLowerCase().startsWith("es") ? "es" : "en"),
  settings: store.get("settings", { mode: "2v2", target: 100, opponents: "rule", partner: "rule", seed: null }),
  analysisOn: store.get("analysis", false),
  reveal: store.get("reveal", false),
  record: null,
  view: null,
  seenHistory: 0,
  selected: null,
  hint: null,
  busy: false,
  epoch: 0,
  analysisId: 0,
};

function t(key, vars = {}) {
  const s = (STRINGS[app.lang] && STRINGS[app.lang][key]) ?? STRINGS.en[key] ?? key;
  return s.replace(/\{(\w+)\}/g, (_, k) => (k in vars ? vars[k] : `{${k}}`));
}

function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const isTwo = () => app.view && app.view.mode === "2v2";
const team = (seat) => seat % 2;

function seatName(seat) {
  if (seat === HUMAN) return t("you");
  if (!isTwo()) return t("rival");
  return [t("you"), t("east"), t("partnerSeat"), t("west")][seat];
}

function teamName(tm) {
  if (!isTwo()) return tm === 0 ? t("you") : t("rival");
  return tm === 0 ? t("us") : t("them");
}

function botFor(seat) {
  return isTwo() && seat === 2 ? app.settings.partner : app.settings.opponents;
}

function gameSeed() {
  return app.settings.seed ?? 1;
}

// A bot's RNG seed: fixed by the deal, the round, and the ply, so a replay
// of the same game makes the same choices.
function botSeed() {
  return (gameSeed() * 7919 + app.view.round * 1009 + app.view.history.length) >>> 0;
}

// ---------- Text ----------

function applyI18n() {
  document.documentElement.lang = app.lang;
  document.querySelectorAll("[data-i18n]").forEach((el) => { el.textContent = t(el.dataset.i18n); });
  document.querySelectorAll("[data-i18n-label]").forEach((el) => { el.setAttribute("aria-label", t(el.dataset.i18nLabel)); });
  $("lang-label").textContent = app.lang === "en" ? "ES" : "EN";
  $("lang").setAttribute("aria-label", t("language"));
  $("lang").title = t("language");
  $("theme").title = t("theme");
  $("footer-text").textContent = t("footer", { v: DOMINOFISH_VERSION });
  for (const id of ["opponents", "partner"]) {
    const sel = $(id);
    const value = sel.value || (id === "partner" ? app.settings.partner : app.settings.opponents);
    sel.innerHTML = BOTS.map((b) => `<option value="${b}">${esc(t("bot_" + b))}</option>`).join("");
    sel.value = value;
  }
}

function moveText(item) {
  const m = item.move.split(" ");
  if (m[0] === "play") {
    const lead = app.view && item.end === "lead";
    const key = lead ? "playsLead" : m[2] === "left" ? "playsLeft" : "playsRight";
    return t(key, { tile: chip(m[1]) });
  }
  if (m[0] === "draw") {
    if (item.drawn && (item.seat === HUMAN || app.reveal)) return t("drewTiles", { tiles: item.drawn.map((x) => chip(x)).join(" ") });
    return t("drew", { n: item.drawn ? item.drawn.length : 1 });
  }
  return t("passed");
}

function eventText(ev) {
  return t("toast", { event: t("ev_" + ev.kind), team: teamName(ev.team), points: ev.points });
}

// ---------- Rendering ----------

function renderScore() {
  const v = app.view;
  const pct = (s) => Math.min(100, (100 * s) / v.target);
  const teamHtml = (tm, side) => `
    <div class="team ${tm === 0 ? "us" : "them"} ${side}">
      <div class="team-line"><span class="team-score">${v.scores[tm]}</span><span class="team-name">${esc(teamName(tm))}</span></div>
      <div class="meter"><span style="width:${pct(v.scores[tm])}%"></span></div>
    </div>`;
  $("scorebar").innerHTML = teamHtml(0, "") +
    `<div class="round-chip"><strong>${esc(t("round", { n: v.round }))}</strong>${esc(t("to", { n: v.target }))}</div>` +
    teamHtml(1, "right");
}

function plate(seat) {
  const v = app.view;
  const turn = v.phase === "playing" && v.turn === seat;
  const thinking = turn && seat !== HUMAN ? `<span class="thinking" aria-hidden="true"><i></i><i></i><i></i></span>` : "";
  const count = v.hands[seat].length;
  return `<span class="plate ${team(seat) === 0 ? "us" : "them"} ${turn ? "turn" : ""}"><span class="dot"></span>${esc(seatName(seat))} <span class="count">· ${count}</span>${thinking}</span>`;
}

function renderSeats() {
  const v = app.view;
  const spots = isTwo() ? { 1: "seat-right", 2: "seat-top", 3: "seat-left" } : { 1: "seat-top" };
  for (const id of ["seat-top", "seat-left", "seat-right"]) $(id).innerHTML = "";
  for (const [seat, id] of Object.entries(spots)) {
    const s = Number(seat);
    const side = id !== "seat-top";
    let tiles;
    if (app.reveal || v.phase !== "playing") {
      tiles = `<div class="mini-hand">${v.hands[s].map((x) => { const [a, b] = parseTile(x); return tileSVG(a, b, { u: 13, vertical: !side }); }).join("")}</div>`;
    } else {
      const n = v.hands[s].length;
      tiles = `<div class="backs">${Array.from({ length: n }, () => tileSVG(0, 0, { u: 10, vertical: !side, back: true })).join("")}</div>`;
    }
    $(id).innerHTML = plate(s) + tiles;
  }
  $("seat-bottom").innerHTML = plate(HUMAN);
}

function legalFor(tile) {
  return app.view.legal.filter((m) => m.tile === tile);
}

function renderBoard() {
  const v = app.view;
  const board = $("board");
  if (v.line.length === 0) {
    board.innerHTML = `<div class="empty-board">${esc(t("emptyBoard", { seat: seatName(v.turn) }))}</div>` + boneyardChip();
    return;
  }
  const width = board.clientWidth || 600;
  const units = v.line.reduce((s, [a, b]) => s + (a === b ? 1 : 2), 0);
  const u = Math.max(12, Math.min(24, Math.floor((width * 2 - 120) / (units + v.line.length * 0.2) / 1.0)));
  const humanTurn = v.phase === "playing" && v.turn === HUMAN;
  const sel = app.selected ? legalFor(app.selected) : [];
  const can = (end) => humanTurn && sel.some((m) => m.end === end);
  const hinted = (end) => app.hint && app.hint.end === end;
  const endChip = (end, pip) =>
    `<button type="button" class="end-chip ${can(end) ? "can" : ""} ${hinted(end) ? "hintmark" : ""}" data-end="${end}" ${can(end) ? "" : "disabled"} aria-label="${esc(t(end))} ${pip}">${pip}</button>`;
  // Only the tile just placed drops in; a re-render (a bot's turn, a
  // resize) leaves the others still.
  const key = v.line.map((x) => x.join("")).join(" ");
  const prev = app.lineKey || "";
  let fresh = -1;
  if (key !== prev && v.line.length > 0) {
    if (prev && key.endsWith(prev)) fresh = 0;
    else if (prev && key.startsWith(prev)) fresh = v.line.length - 1;
    else if (v.line.length === 1) fresh = 0;
  }
  app.lineKey = key;
  // Each end chip stays on the row of the tile it belongs to.
  const tiles = v.line.map(([a, b], i) => tileSVG(a, b, { u, vertical: a === b, cls: i === fresh ? "fresh" : "" }));
  const left = endChip("left", v.ends[0]);
  const right = endChip("right", v.ends[1]);
  let row;
  if (tiles.length === 1) {
    row = `<span class="end-group">${left}${tiles[0]}${right}</span>`;
  } else {
    const middle = tiles.slice(1, -1).join("");
    row = `<span class="end-group">${left}${tiles[0]}</span>${middle}<span class="end-group">${tiles[tiles.length - 1]}${right}</span>`;
  }
  board.innerHTML = `<div class="line">${row}</div>` + boneyardChip();
  board.querySelectorAll(".end-chip").forEach((el) => el.addEventListener("click", () => chooseEnd(el.dataset.end)));
}

function boneyardChip() {
  return app.view.mode === "1v1" ? `<span class="boneyard">${esc(t("boneyard", { n: app.view.boneyard }))}</span>` : "";
}

function renderHand() {
  const v = app.view;
  const humanTurn = v.phase === "playing" && v.turn === HUMAN && !app.busy;
  const small = window.innerWidth < 640 || window.innerHeight < 520;
  const u = small ? (v.hands[HUMAN].length > 9 ? 19 : window.innerHeight < 520 ? 20 : 23) : 28;
  $("hand").innerHTML = v.hands[HUMAN].map((x) => {
    const [a, b] = parseTile(x);
    const playable = humanTurn && legalFor(x).length > 0;
    const cls = [humanTurn && !playable ? "dim" : "", app.selected === x ? "sel" : "", app.hint && app.hint.tile === x ? "hinted" : ""].join(" ");
    return `<button type="button" class="tile-btn ${cls}" data-tile="${x}" ${playable ? "" : "disabled"} aria-label="${a}-${b}">${tileSVG(a, b, { u, vertical: true })}</button>`;
  }).join("");
  $("hand").querySelectorAll(".tile-btn").forEach((el) => el.addEventListener("click", () => pickTile(el.dataset.tile)));
  const actions = [];
  if (humanTurn && v.legal.length === 1 && v.legal[0].move === "draw") actions.push(`<button type="button" class="btn btn-act" data-move="draw">${esc(t("draw"))}</button>`);
  if (humanTurn && v.legal.length === 1 && v.legal[0].move === "pass") actions.push(`<button type="button" class="btn btn-act" data-move="pass">${esc(t("pass"))}</button>`);
  if (v.phase === "round_over" && !$("round-dialog").open) actions.push(`<button type="button" class="btn btn-primary" data-next="1">${esc(t("nextRound"))}</button>`);
  if (v.phase === "finished" && !$("round-dialog").open) actions.push(`<button type="button" class="btn btn-primary" data-new="1">${esc(t("playAgain"))}</button>`);
  $("hand-actions").innerHTML = actions.join("");
  $("hand-actions").querySelectorAll("[data-move]").forEach((el) => el.addEventListener("click", () => humanMove(el.dataset.move)));
  $("hand-actions").querySelectorAll("[data-next]").forEach((el) => el.addEventListener("click", nextRound));
  $("hand-actions").querySelectorAll("[data-new]").forEach((el) => el.addEventListener("click", openNewGame));
}

function renderStatus() {
  const v = app.view;
  let s = "";
  if (v.phase === "playing") {
    if (v.turn !== HUMAN) s = t("thinking", { seat: seatName(v.turn) });
    else if (v.legal.length === 1 && v.legal[0].move === "draw") s = t("mustDraw");
    else if (v.legal.length === 1 && v.legal[0].move === "pass") s = t("mustPass");
    else if (app.selected && legalFor(app.selected).length > 1) s = t("chooseEnd");
    else if (v.line.length === 0) s = t("yourLead");
    else s = t("yourTurn");
  }
  $("status").textContent = s;
}

function renderMoves() {
  const v = app.view;
  const items = [];
  if (v.opening) items.push(`<li><span class="who">${esc(seatName(v.opening.seat))}</span><span class="what">${t("opensShort", { tile: chip(v.opening.tile) })}</span></li>`);
  for (const h of v.history) {
    const tag = h.event ? `<span class="tag">${esc(t("ev_" + h.event.kind))} +${h.event.points}</span>` : "";
    items.push(`<li><span class="who">${esc(seatName(h.seat))}</span><span class="what">${moveText(h)} ${tag}</span></li>`);
  }
  $("moves").innerHTML = items.length ? items.join("") : `<li class="muted">${esc(t("noMoves"))}</li>`;
  $("moves").scrollTop = $("moves").scrollHeight;
  $("undo").disabled = app.busy || !v.history.some((h) => h.seat === HUMAN);
}

function render() {
  if (!app.view) return;
  renderScore();
  renderSeats();
  renderBoard();
  renderHand();
  renderStatus();
  renderMoves();
  $("reveal").checked = app.reveal;
  $("analysis-on").checked = app.analysisOn;
  $("hint").disabled = !(app.view.phase === "playing" && app.view.turn === HUMAN) || app.busy;
}

function toast(html) {
  const el = document.createElement("div");
  el.className = "toast";
  el.innerHTML = html;
  $("toasts").appendChild(el);
  setTimeout(() => el.remove(), 2600);
}

// ---------- Analysis ----------

async function renderAnalysis() {
  const box = $("analysis");
  const v = app.view;
  if (!app.analysisOn) { box.innerHTML = `<p class="muted small">${esc(t("analysisOff"))}</p>`; return; }
  if (v.phase !== "playing") { box.innerHTML = `<p class="muted small">${esc(t("analysisOver"))}</p>`; return; }
  const id = ++app.analysisId;
  box.innerHTML = `<div class="skeleton" aria-hidden="true"></div>`;
  const res = await app.engine.request("analyze", app.record);
  if (id !== app.analysisId) return;
  if (!res.ok) { box.innerHTML = `<p class="muted small">${esc(res.error)}</p>`; return; }
  const a = JSON.parse(res.body);
  const mover = v.turn;
  const better = (x, y) => (team(mover) === 0 ? x > y : x < y);
  let best = a.moves[0];
  for (const m of a.moves) if (better(m.value, best.value)) best = m;
  const sorted = [...a.moves].sort((x, y) => (team(mover) === 0 ? y.value - x.value : x.value - y.value));
  const fmt = (x) => (x > 0 ? `+${x}` : `${x}`);
  const fill = 50 + (Math.max(-60, Math.min(60, a.value)) / 120) * 100;
  const desc = (m) => {
    const parts = m.move.split(" ");
    if (parts[0] === "play") return `${chip(parts[1], 13)} <span>${esc(t(m.end))}</span>`;
    return esc(parts[0] === "draw" ? t("draw") : t("pass"));
  };
  const list = sorted.map((m) => `
    <li class="mv-item ${m.move === best.move ? "best" : ""}">
      <span class="mv-desc">${desc(m)}${m.move === best.move ? `<span class="tag">${esc(t("best"))}</span>` : ""}</span>
      <span></span>
      <span class="val ${m.value > 0 ? "pos" : m.value < 0 ? "neg" : ""}">${fmt(m.value)}</span>
    </li>`).join("");
  const pv = a.line.slice(0, 12).map((s) => `<span class="step" title="${esc(seatName(s.seat))}" aria-label="${esc(seatName(s.seat))}"><span class="dot ${team(s.seat) === 0 ? "us" : "them"}"></span>${s.tile ? chip(s.tile, 11) : esc(s.move === "draw" ? t("draw") : t("pass"))}</span>`).join("");
  box.innerHTML = `
    <div class="evalbar" role="img" aria-label="${esc(teamName(0))} ${fmt(a.value)}"><div class="fill" style="width:${fill}%"></div><div class="label">${fmt(a.value)}</div></div>
    <p class="small muted">${esc(t("analysisFor", { seat: seatName(mover) }))}</p>
    <ul class="mv-list">${list}</ul>
    <p class="pv-title">${esc(t("lineTitle"))}</p>
    <div class="pv">${pv}</div>
    ${a.capped ? `<p class="small muted">${esc(t("capped"))}</p>` : ""}`;
}

// ---------- Game flow ----------

async function refresh() {
  const res = await app.engine.request("view", app.record);
  if (!res.ok) throw new Error(res.error);
  const prev = app.view;
  app.view = JSON.parse(res.body);
  const sameRound = prev && prev.round === app.view.round;
  if (sameRound && app.view.history.length > app.seenHistory) {
    for (const h of app.view.history.slice(app.seenHistory)) if (h.event) toast(esc(eventText(h.event)));
  }
  app.seenHistory = app.view.history.length;
  store.set("record", app.record);
  render();
  renderAnalysis();
  if (app.view.phase !== "playing") showRoundEnd();
  else if (app.view.turn !== HUMAN) botTurn();
}

async function botTurn() {
  if (app.busy) return;
  app.busy = true;
  const epoch = app.epoch;
  render();
  await sleep(BOT_DELAY_MS);
  if (epoch !== app.epoch) return;
  const v = app.view;
  const res = await app.engine.request(`bot ${botFor(v.turn)} ${botSeed()}`, app.record);
  if (epoch !== app.epoch) return;
  app.busy = false;
  if (!res.ok) { $("status").textContent = res.error; return; }
  await applyMove(res.body);
}

async function applyMove(move) {
  const res = await app.engine.request(`play ${move}`, app.record);
  if (!res.ok) { $("status").textContent = res.error; return; }
  app.record = res.body;
  app.selected = null;
  app.hint = null;
  $("hint-out").textContent = "";
  await refresh();
}

function humanMove(move) {
  if (app.busy || app.view.turn !== HUMAN) return;
  applyMove(move);
}

function pickTile(tile) {
  const options = legalFor(tile);
  if (options.length === 0) return;
  if (options.length === 1) { humanMove(options[0].move); return; }
  app.selected = app.selected === tile ? null : tile;
  render();
}

function chooseEnd(end) {
  if (!app.selected) return;
  const m = legalFor(app.selected).find((x) => x.end === end);
  if (m) humanMove(m.move);
}

async function hint() {
  const v = app.view;
  if (v.phase !== "playing" || v.turn !== HUMAN) { $("hint-out").textContent = t("hintWait"); return; }
  const res = await app.engine.request(`bot rule ${botSeed()}`, app.record);
  if (!res.ok) { $("hint-out").textContent = res.error; return; }
  const m = v.legal.find((x) => x.move === res.body) || { move: res.body };
  app.hint = m;
  if (m.tile) {
    $("hint-out").innerHTML = m.end === "lead" ? t("hintLead", { tile: chip(m.tile, 14) }) : t("hintPlay", { tile: chip(m.tile, 14), end: esc(t(m.end)) });
  } else {
    $("hint-out").textContent = m.move === "draw" ? t("hintDraw") : t("hintPass");
  }
  render();
}

function showRoundEnd() {
  const v = app.view;
  const r = v.result;
  const dlg = $("round-dialog");
  if (!r) return;
  const finished = v.phase === "finished";
  const winner = r.team;
  $("round-title").textContent = finished
    ? t(isTwo() ? "gameTitle" : "gameTitle1", { team: teamName(winner), a: Math.max(...v.scores), b: Math.min(...v.scores) })
    : t("roundTitle", { team: teamName(winner), points: r.points });
  let detail = "";
  if (r.kind === "tranque") {
    detail = t("detail_tranque", { a: seatName(r.blocker), ap: r.blocker_pips, b: seatName(r.rival), bp: r.rival_pips, winner: seatName(r.seat) });
  } else {
    detail = t("detail_" + r.kind, { seat: seatName(r.seat) });
  }
  $("round-detail").textContent = detail;
  $("round-hands").innerHTML = `<p class="pv-title">${esc(t("remaining"))}</p>` + v.hands.map((h, s) => `
    <div class="rh"><span>${esc(seatName(s))}</span><div class="mini-hand">${h.map((x) => { const [a, b] = parseTile(x); return tileSVG(a, b, { u: 13 }); }).join("") || "<span class='muted'>0</span>"}</div></div>`).join("");
  $("round-next").textContent = finished ? t("playAgain") : t("nextRound");
  if (!dlg.open) dlg.showModal();
  render();
}

async function nextRound() {
  $("round-dialog").close();
  if (app.view.phase === "finished") { openNewGame(); return; }
  const seed = (gameSeed() * 104729 + app.view.round * 7) >>> 0;
  const res = await app.engine.request(`next ${seed}`, app.record);
  if (!res.ok) { $("status").textContent = res.error; return; }
  app.record = res.body;
  app.seenHistory = 0;
  await refresh();
}

async function undo() {
  if (app.busy) return;
  let removed = false;
  while (!removed) {
    const v = app.view;
    if (v.history.length === 0) break;
    const last = v.history[v.history.length - 1];
    const res = await app.engine.request("undo", app.record);
    if (!res.ok) break;
    app.record = res.body;
    const view = await app.engine.request("view", app.record);
    app.view = JSON.parse(view.body);
    removed = last.seat === HUMAN;
  }
  app.selected = null;
  app.hint = null;
  app.seenHistory = app.view.history.length;
  $("hint-out").textContent = "";
  if ($("round-dialog").open) $("round-dialog").close();
  await refresh();
}

async function startGame(settings) {
  app.epoch++;
  app.busy = false;
  app.settings = settings;
  store.set("settings", settings);
  const res = await app.engine.request(`new ${settings.mode} ${settings.target} ${settings.seed}`);
  if (!res.ok) { $("status").textContent = res.error; return; }
  app.record = res.body;
  app.view = null;
  app.seenHistory = 0;
  app.selected = null;
  app.hint = null;
  $("hint-out").textContent = "";
  if ($("round-dialog").open) $("round-dialog").close();
  await refresh();
}

function randomSeed() {
  const a = new Uint32Array(1);
  crypto.getRandomValues(a);
  return (a[0] % 900000000) + 1;
}

function openNewGame() {
  if ($("round-dialog").open) $("round-dialog").close();
  const f = $("new-form");
  f.mode.value = app.settings.mode;
  f.target.value = String(app.settings.target);
  $("opponents").value = app.settings.opponents;
  $("partner").value = app.settings.partner;
  $("seed").value = String(randomSeed());
  $("partner-field").hidden = f.mode.value !== "2v2";
  $("new-dialog").showModal();
}

// ---------- Setup ----------

function setTheme(theme) {
  if (theme) document.documentElement.dataset.theme = theme;
  else delete document.documentElement.dataset.theme;
}

async function main() {
  setTheme(store.get("theme", null));
  applyI18n();
  $("new-game").addEventListener("click", openNewGame);
  $("lang").addEventListener("click", () => {
    app.lang = app.lang === "en" ? "es" : "en";
    store.set("lang", app.lang);
    applyI18n();
    render();
    renderAnalysis();
    if ($("round-dialog").open) showRoundEnd();
  });
  $("theme").addEventListener("click", () => {
    const dark = getComputedStyle(document.documentElement).colorScheme === "dark";
    const next = dark ? "light" : "dark";
    setTheme(next);
    store.set("theme", next);
  });
  $("hint").addEventListener("click", hint);
  $("undo").addEventListener("click", undo);
  $("reveal").addEventListener("change", (e) => { app.reveal = e.target.checked; store.set("reveal", app.reveal); render(); });
  $("analysis-on").addEventListener("change", (e) => { app.analysisOn = e.target.checked; store.set("analysis", app.analysisOn); renderAnalysis(); });
  $("round-next").addEventListener("click", nextRound);
  $("round-review").addEventListener("click", () => { $("round-dialog").close(); render(); });
  $("seed-random").addEventListener("click", () => { $("seed").value = String(randomSeed()); });
  const form = $("new-form");
  form.querySelectorAll("input[name=mode]").forEach((el) => el.addEventListener("change", () => { $("partner-field").hidden = form.mode.value !== "2v2"; }));
  $("new-dialog").addEventListener("close", () => {
    if ($("new-dialog").returnValue !== "start") return;
    const seed = Number.parseInt($("seed").value, 10);
    startGame({
      mode: form.mode.value,
      target: Number(form.target.value),
      opponents: $("opponents").value,
      partner: $("partner").value,
      seed: Number.isFinite(seed) && seed > 0 ? seed : randomSeed(),
    });
  });
  let resizeTimer = 0;
  window.addEventListener("resize", () => { clearTimeout(resizeTimer); resizeTimer = setTimeout(render, 120); });

  app.engine = new Engine(DOMINOFISH_WASM);
  try {
    await app.engine.start();
  } catch (err) {
    $("status").textContent = t("engineError");
    return;
  }
  const saved = store.get("record", null);
  if (saved && app.settings.seed) {
    app.record = saved;
    try { await refresh(); return; } catch { /* a stale or damaged record: start over */ }
  }
  await startGame({ ...app.settings, seed: randomSeed() });
}

main();
