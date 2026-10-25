# DominoFish rules

DominoFish plays Capi's rules exactly as Capi's engine implements them at commit 1d8868a26ce930f478f98e64060c06b1d1a2c1f4 (owner decision 5). Where Capi's behavior looks wrong, DominoFish matches it and the question goes to the owner.

Cite format: each cite is a file and line range in the pinned packages/engine tree followed by an anchor, a code fragment that appears in that range. Files ending in .test.ts are under packages/engine/__tests__/, the others under packages/engine/src/. scripts/verify/M1-01.sh checks every anchor against the pinned code. A rule is verified only by parity with Capi (M1-04 to M1-07), never by this text.

## Rules

Source: capi-rules-spec[1] (digest 1.2), checked against the code.

### Setup

- R1. Tiles: the 28 tiles of a double-six set in the order a = 0..6, b = a..6, each stored low pip first; DominoFish uses this order as the mask index (index 0 is 0-0, index 27 is 6-6). `reducer.ts:27-35`: `for (let b = a; b <= 6; b++) {`
- R2. Shuffle: Fisher-Yates over that order with an injected rng. DominoFish never reproduces Capi's RNG; parity tests inject Capi's deals. `reducer.ts:51-58`: `const j = Math.floor(rng() * (i + 1));`
- R3. Seats and deal: 2v2 seats n, e, s, w; 1v1 seats n, s. Seven tiles per seat in seat order from the shuffled array; the rest is the boneyard (14 in 1v1, none in 2v2), kept in order. Teams: 2v2 n and s are team 0, e and w team 1; 1v1 n is team 0, s team 1. `types.ts:112-114`: `return is2v2 ? ["n", "e", "s", "w"] : ["n", "s"];` `reducer.ts:128-133`: `const boneyard = shuffled.slice(idx);` `types.ts:116-119`: `if (is2v2) return seat === "n" || seat === "s" ? 0 : 1;`

### Opening

- R4. Round 1 opener: the seat with the highest double, scanning seats in order with a strict > on the double's pip, so in 2v2 the holder of the 6-6. With no double dealt (1v1 only), each seat's best tile is its first tile in hand order with the highest pip sum, and the seats compare with a strict >, so ties go to the earlier seat. The opener's tile leaves the hand and is placed automatically as oriented in the hand, low pip left. `reducer.ts:135-145`: `if (!starterTile || d[0] > starterTile[0]) {` `reducer.ts:98-109`: `if (sum > bestSum) {` `reducer.ts:146-159`: `if (sum > bestSum) {` `reducer.ts:161-163`: `hands[starter] = removeTileFromHand(hands[starter], starterTile);` `reducer.ts:174`: `board: starterTile ? [starterTile] : [],`
- R5. After the automatic tile: the turn goes to the next seat, lastPlayedBy and starterThisRound are the opener, the pass counter is 0. The automatic tile counts as the opener's play for both bonuses. `reducer.ts:176-184`: `currentTurn: getNextSeat(starter, is2v2),` `reducer.ts:184`: `lastPlayedBy: starter,`
- R6. Later rounds: a fresh deal, an empty board, and the previous round's winning seat leads any tile (who went out on a domino or capicua, the tranque's winnerSeat otherwise); lastPlayedBy is null; scores carry. A new round starts only from round_over. `reducer.ts:520-564`: `const starter = getRoundWinningSeat(state);` `reducer.ts:531`: `if (state.phase !== "round_over") return state;` `reducer.ts:482-491`: `if (payload?.winnerSeat) return payload.winnerSeat;` `reducer.ts:562`: `lastPlayedBy: null,`

### Turns and plays

- R7. Turn order: n, e, s, w in 2v2 and n, s in 1v1; "the player to the right" is the next seat. `types.ts:103-110`: `const order: Seat[] = is2v2 ? SEAT_ORDER : ["n", "s"];`
- R8. Moves are accepted only in phase playing and only from the seat on turn. `validate.ts:58-63`: `if (state.currentTurn !== seat) {`
- R9. A play names a tile in hand, in either orientation, and the end left or right. `validate.ts:67-81`: `if (!tileInHand(hand, intent.tile)) {`
- R10. Empty board: any tile, and the end is irrelevant; the tile is placed as oriented in the intent, so the orientation sets which pip shows on the left. `validate.ts:82-84`: `if (state.board.length === 0) {` `reducer.ts:78`: `if (board.length === 0) return [[a, b]];`
- R11. Otherwise the tile must show the chosen end's pip, and its other pip becomes that end. A double d-d fits only an end that shows d. With ends L != R, the tile L-R fits both: on the left the ends become R|R, on the right L|L. With L == R, the two ends give mirror-image positions. `validate.ts:85-91`: `if (intent.end === "left" && !matchesLeft) return "Tile does not match left end";` `reducer.ts:79-87`: `const match = a === leftEnd ? b : a;`
- R12. A player who can play must play: pass and draw are rejected while any tile in hand fits. `validate.ts:101-103`: `if (hasLegalPlay(hand, state.board)) {` `validate.ts:107-110`: `if (hasLegalPlay(hand, state.board)) {`

### Draws and passes

- R13. A draw is legal only in 1v1, only with no legal play, and only with a non-empty boneyard. `validate.ts:94-105`: `return "Draw is not allowed in 2v2 (no boneyard)";`
- R14. One draw pops tiles from the end of the boneyard into the hand until the first tile that fits or an empty boneyard. The turn stays with the drawer, the pass counter does not change, and a draw never scores. `reducer.ts:385-394`: `const drawn = newBoneyard.pop()!;` `reducer.ts:393`: `if (hasLegalPlay(newHand, state.board)) break;` `reducer.ts:396-405`: `boneyard: newBoneyard,`
- R15. After a draw that found a fit, the last tile drawn is the only tile that fits, so the next play is forced except for the end. This follows from R12 to R14 (Inferred); the parity checks test it. `reducer.ts:390-394`: `while (newBoneyard.length > 0) {`
- R16. A pass is legal only with no legal play, and in 1v1 only once the boneyard is empty. It advances the turn and adds one to the pass counter (Capi keeps two identical counters; DominoFish keeps one, L5). `validate.ts:107-115`: `if (!state.is2v2 && state.boneyard.length > 0) {` `reducer.ts:408-414`: `const newPassesSincePlay = state.passesSinceLastPlay + 1;`

### Round end

- R17. Domino: a play that empties the hand ends the round. It is checked before the lock, so going out on the locking tile is a domino. Points: every pip in every hand, the partner's included; boneyard pips never count. `reducer.ts:306-316`: `if (newHand.length === 0) {` `scoring.ts:35-43`: `(sum, seat) => sum + handPips(state.hands[seat] ?? []),`
- R18. Capicua: +25 on a domino whose last tile fits both ends as they were before it. Yes: 3-5 on 3|5, 5-6 on 5|5, 2-2 on 2|2. No: 2-2 on 2|5, an empty board, a tranque. `reducer.ts:236-240`: `if (isCapicua(endsBefore, lastTile)) {` `scoring.ts:91-98`: `return fits(endsBefore.left) && fits(endsBefore.right);`
- R19. Tranque: after a play that does not empty the hand, the round ends if no tile outside the board (in any hand or the boneyard) fits either end. The blocker is the placer and the rival is the next seat, always an opponent. The one with fewer pips in his own hand wins the round for his side; equal pips go to starterThisRound (la mano), even when he is neither. Points: every pip in every hand; boneyard pips never count. `reducer.ts:318-322`: `if (isBoardLocked(placed)) {` `validate.ts:45-51`: `return !hasLegalPlay(outside, state.board);` `scoring.ts:63-82`: `: state.starterThisRound;` `scoring.ts:76`: `pts: teamPips(state, 0) + teamPips(state, 1),`
- R20. In 1v1 a lock can happen while the boneyard still holds tiles. `reducer.ts:320`: `if (isBoardLocked(placed)) {` `reducer.test.ts:314`: `the tile that leaves nothing playable ends the round, even with tiles in the boneyard`

### Scoring and game end

- R21. Only the round-winning side scores the round. Reaching the target finishes the game with that side as winner; otherwise the phase is round_over. The target defaults to 100; Capi offers 100 or 200. `reducer.ts:190-198`: `state.scores[0] + (winningTeam === 0 ? pts : 0),` `reducer.ts:206-218`: `phase: winnerTeam !== null ? "finished" : "round_over",` `reducer.ts:119`: `const { mode, theme, is2v2, targetScore = 100, rng = secureRandom } = params;`
- R22. Capicua is round score: it is not capped and can win the game. `reducer.test.ts:1537`: `CAPICÚA at game-winning DOMINÓ awards bonus AND finishes game`

### Mid-round bonuses (2v2 only)

- R23. Pase corrido: a pass that is the third since the last play, with lastPlayedBy not null and the next seat equal to lastPlayedBy, gives lastPlayedBy's team +25; the round continues and lastPlayedBy plays next; it can repeat within a round. In 1v1 the threshold is infinite. `reducer.ts:425-429`: `nextTurn === state.lastPlayedBy &&` `reducer.ts:413`: `const veinticincoThreshold = state.is2v2 ? 3 : Infinity;` `reducer.ts:445`: `currentTurn: nextTurn, // = lastPlayedBy, who must now play`
- R24. Pase de salida: a play that does not end the round, by the opener's partner, when the board held only the opening tile, lastPlayedBy was starterThisRound, and the pass counter was 1, gives the opener's team +25. It applies every round, the automatic 6-6 included. The partner's pass cancels it, and the next pass (by the seat before the opener, the third since the opening tile) is a pase corrido that pays 25 once. Capi's comment says "fourth pass" but the code pays on the third. `reducer.ts:340-347`: `seat === getNextSeat(getNextSeat(opener, true), true)` `reducer.ts:345`: `state.passesSinceLastPlay === 1 &&` `reducer.ts:423`: `by the partner's pass, the fourth pass is this pase corrido, so an`
- R25. Cap: either mid-round bonus pays only if scores[team] + 25 < target (strict), so a bonus never reaches the target; otherwise the move is plain, with no event. `reducer.ts:349`: `if (state.scores[openerTeam] + SALIDA_BONUS < state.targetScore) {` `reducer.ts:431`: `if (state.scores[winningTeam] + VEINTICINCO_BONUS < state.targetScore) {` `scoring.ts:100-104`: `export const SALIDA_BONUS = 25;`
- R26. Bonuses and later round points bank in the same score column. `reducer.test.ts:2021`: `max stack: VEINTICINCO + DOMINÓ + CAPICÚA all bank into the same column`
- R27. The next round's leader is the individual round winner (R6), not the team. `reducer.ts:485-487`: `return state.lastPlayedBy ?? state.starterThisRound;`
- R28. Forfeit by silence (120,000 ms in live mode, never in turn_based) is an app rule and is not modeled. `forfeit.ts:9`: `export const CLAIM_AFTER_MS = 120_000;` `forfeit.ts:42-78`: `if (state.mode === "turn_based") {`

R15, R17, R19, R20, and R22 are the rules most likely to be implemented wrong (rules lane).

## Legacy branches not reproduced

Source: capi-rules-spec[4] (digest 1.6). None is reachable from a fresh deal.

- L1. A lock resolved on a draw or a pass, with blocker lastPlayedBy or the previous seat. A live board is never locked, because R19 ends the round at the locking tile. `reducer.ts:377-383`: `if ((intent.type === "draw" || intent.type === "pass") && isBoardLocked(state)) {`
- L2. creditRound's branches for the other side already at the target and for two sides past it. Bonuses stop below the target (R25), so only the round winner can reach it. `reducer.ts:200-213`: `if (scores[winningTeam] >= t && scores[other] >= t) {`
- L3. getRoundWinningSeat's fallbacks for tranque payloads without winnerSeat or comparison fields, and the domino fallback to starterThisRound. `reducer.ts:489-515`: `if (payload?.blockerSeat && payload.rivalSeat) {` `reducer.ts:486`: `return state.lastPlayedBy ?? state.starterThisRound;`
- L4. The pase de salida sent as a "veinticinco" callout with salida set, for old clients. DominoFish emits typed events (L4 is mapped only in the parity harness). `types.ts:27-28`: `// clients that predate it still show a +25 banner for the right side.` `reducer.ts:363`: `lastCallout: "veinticinco",`
- L5. Two pass counters that always hold the same value; DominoFish keeps one. `reducer.ts:177-178`: `consecutivePasses: 0,` `reducer.ts:409-410`: `const newConsecutive = state.consecutivePasses + 1;`
- L6. scoreDomino's unused winningTeam parameter. `scoring.ts:30-35`: `export function scoreDomino(state: GameState, winningTeam: 0 | 1): number {`
- L7. The salida guards for saved states that live play cannot reach. DominoFish keeps the same guards as plain conditions (rules.rs, place) and adds none for unreachable states; the property and parity tests cover the reachable ones. `rules.salida-and-tie.test.ts:250-261`: `Not reachable in live play`
- L8. App and session fields in the rules state (mode, theme, players, rematchGameId, lastMoveAt, forfeit, the callout banners). `types.ts:59-87`: `rematchGameId?: string;`
- L9. Wire validation inside the rules path (tile shape, bogus ends). DominoFish validates at the API boundary. `validate.ts:13-20`: `function isTile(value: unknown): value is Tile {` `validate.ts:77-81`: `if (intent.end !== "left" && intent.end !== "right") {`

## Moves

DominoFish's move type is Play { tile, end }, Draw, or Pass.

- On a non-empty board, Play { tile, end } joins the tile to that end (R11).
- On an empty board, end sets the orientation (R10): Left places the tile low pip left, as it sits in a dealt or drawn hand (every hand tile is stored low pip first, R1); Right places it flipped, high pip left.

Canonical moves are one representative of each set of intents whose results are equal or mirror images (the end-mirror is an exact symmetry: every rule reads the two ends symmetrically):
- Empty board: one move per tile in hand, Play { tile, Left }.
- Ends L != R: Play { tile, Left } for each tile showing L, and Play { tile, Right } for each tile showing R; the tile L-R gives both.
- Ends L == R: Play { tile, Left } for each tile showing L.
- No play: Draw in 1v1 while the boneyard holds tiles, else Pass.

The generator emits at most 12 distinct plays (digest 1.8). Perft line counts (M1-06) count canonical moves.

### Capi intents to moves

Exact replay (golden traces, the fuzzer): a Capi intent becomes one DominoFish move that reproduces Capi's resulting state exactly, orientation included.
- play [a, b] on an empty board: Play { tile, Left } if a <= b, else Play { tile, Right }.
- play [a, b] with end E on a non-empty board: Play { tile, E } (the intent's orientation has no effect: `reducer.ts:79-87`: `const match = a === leftEnd ? b : a;`).
- draw: Draw. pass: Pass.

Canonical map (legal-set comparison, perft): the replay move, then Play { tile, Right } becomes Play { tile, Left } on an empty board or when L == R. Capi's rulesIntents (`flow.endgame.test.ts:83-95`: `function rulesIntents(state: GameState): MoveIntent[] {`) lists every tile in the in-hand orientation with each end that fits; mapped this way, its set equals DominoFish's canonical set. Capi's own fuzz leads only with end "right" on an empty board (`invariants.fuzz.test.ts:60-62`: `for (const tile of hand) out.push({ type: "play", tile, end: "right" });`), so DominoFish's traces and fuzzer also lead with end "left" and with flipped orientations.

## Canonical state

The state compared with Capi after every move: phase; scores; target; the turn; starterThisRound; lastPlayedBy; the pass counter; each seat's hand as a set; the board's tile set with its left and right ends; the boneyard in order; and, at a round end, the event with its points and the next leader. Capi's board array is determined by the moves and these ends, so it is not compared tile by tile.

## Rule paths

The differential fuzzer (M1-07) counts how often each path occurs and requires every count above zero in the modes listed. The first group are the paths Capi's fuzz counts (`invariants.fuzz.test.ts:421-442`: `const SHARED_PATHS = [`); the second are DominoFish's additions.

| Path | Modes | Meaning |
|---|---|---|
| capicua | 1v1, 2v2 | capicua with a non-double on ends L != R |
| capicua-equal-ends | 1v1, 2v2 | capicua with a non-double on ends L == R |
| capicua-double | 1v1, 2v2 | capicua with a double |
| domino-double-one-end | 1v1, 2v2 | domino with a double that fits one end only |
| tranque-blocker-wins | 1v1, 2v2 | the blocker has fewer pips |
| tranque-rival-wins | 1v1, 2v2 | the rival has fewer pips |
| tranque-tie | 1v1, 2v2 | equal pips; the opener wins |
| tranque-tie-opener-not-compared | 2v2 | equal pips won by an opener who is neither blocker nor rival |
| pase-corrido-paid | 2v2 | R23 pays |
| pase-corrido-capped | 2v2 | R23 shape at the cap, no payment |
| salida-paid | 2v2 | R24 pays |
| salida-capped | 2v2 | R24 shape at the cap, no payment |
| salida-cancelled | 2v2 | the opener's partner passes on the lone opening tile |
| salida-cancelled-then-corrido | 2v2 | that cancel followed by a paid pase corrido |
| open-double-66 | 1v1, 2v2 | round 1 opens with the 6-6 |
| open-double-lower | 1v1 | round 1 opens with a lower double |
| open-no-double | 1v1 | round 1 opens with the highest pip-sum tile |
| lead-left | 1v1, 2v2 | a lead sent with end left |
| lead-flipped | 1v1, 2v2 | a lead sent high pip first |
| play-both-ends | 1v1, 2v2 | a tile L-R played on ends L != R |
| play-equal-ends-right | 1v1, 2v2 | a play on the right end with L == R |
| draw-fit | 1v1 | a draw that ends on a fitting tile |
| draw-several | 1v1 | a draw of two or more tiles |
| draw-exhausted | 1v1 | a draw that empties the boneyard with no fit |
| pass-1v1 | 1v1 | a pass with an empty boneyard |
| domino | 1v1, 2v2 | a plain domino |
| domino-on-locking-tile | 1v1, 2v2 | going out with a tile that also locks the board |
| tranque-with-boneyard | 1v1 | a lock while the boneyard holds tiles |
| pase-corrido-twice | 2v2 | two paid pases corridos in one round |
| bonus-26-short | 2v2 | a bonus paid with the side 26 below the target |
| bonus-25-short | 2v2 | a bonus refused with the side 25 below the target |
| game-won-by-domino | 1v1, 2v2 | a domino or capicua finishes the game |
| game-won-by-capicua-bonus | 1v1, 2v2 | the game finishes only because of the capicua bonus |
| game-won-by-tranque | 1v1, 2v2 | a tranque finishes the game |
| round-ends-one-short | 1v1, 2v2 | a round leaves the winner exactly 1 below the target |
| game-ends-exactly-at-target | 1v1, 2v2 | a game finishes with the winner exactly at the target |

## Edge cases

Source: capi-rules-spec[2] (digest 1.5) and the published deals (digest 3.6). Each scenario in fixtures/edges.txt is a round from Capi's engine (tools/capi-oracle/edges.mjs): a fixed deal with a script, a seeded deal found by searching for the case, or a found round replayed from other start scores. crates/dominofish-core/tests/edges.rs replays every scenario against Capi's events and states and holds the named test for each case. A Capi test that builds a state no fresh deal reaches is matched by the closest reachable case, with the reason in the note.

| Capi test | DominoFish tests | Note |
|---|---|---|
| `reducer.test.ts:11`: `starter has 6 tiles, other(s) have 7 in 1v1` | start_1v1_starter_has_6 | |
| `reducer.test.ts:37`: `boneyard has correct remainder` | start_1v1_boneyard_14 | |
| `reducer.test.ts:664`: `has empty boneyard in 2v2` | start_2v2_no_boneyard | |
| `reducer.test.ts:680`: `follows N` | turn_order_n_e_s_w | |
| `reducer.test.ts:1357`: `draw stops at first playable tile; turn does not advance` | draw_stops_at_first_fit | also R15: only the last tile drawn fits |
| `reducer.test.ts:1387`: `draw exhausts boneyard without finding a match; subsequent pass is allowed` | draw_exhausts_then_pass | |
| `reducer.test.ts:1427`: `drawn tiles count against you when the table locks right after` | drawn_tiles_count_in_tranque | |
| `validate.test.ts:118`: `rejects draw in 2v2 mode` | reject_draw_in_2v2 | |
| `validate.test.ts:70`: `rejects pass when boneyard has tiles (1v1)` | reject_pass_with_boneyard | |
| `reducer.test.ts:314`: `even with tiles in the boneyard` | lock_with_boneyard_tiles | |
| `reducer.test.ts:362`: `no lock while the boneyard still holds a tile that fits` | no_lock_while_boneyard_fits | |
| `validate.test.ts:155`: `a fitting tile in the boneyard keeps the board open` | boneyard_tile_keeps_board_open | |
| `validate.test.ts:160`: `an empty table is never locked` | empty_table_never_locked | |
| `reducer.test.ts:734`: `ends the round on the locking tile: no passes and no pase corrido first` | tranque_on_locking_tile | |
| `reducer.test.ts:760`: `the blocker's own hand decides, even when his side holds more pips` | tranque_blocker_hand_decides | |
| `reducer.test.ts:813`: `the rival is always the next seat: W blocks, N compares` | tranque_w_blocks_n_compares | |
| `rules.salida-and-tie.test.ts:364`: `opener N, blocker S, rival W on equal pips` | tranque_tie_opener_n_blocker_s_rival_w | |
| `rules.salida-and-tie.test.ts:393`: `opener W, blocker N, rival E on equal pips` | tranque_tie_opener_w_blocker_n_rival_e | |
| `rules.salida-and-tie.test.ts:409`: `1v1: a tie goes to whichever of the two opened the round` | tranque_tie_1v1_opener | |
| `reducer.test.ts:854`: `locking the board with the last tile is a domin` | going_out_on_locking_tile_is_domino | Capi's state (two board tiles) is unreachable. From a fresh deal a lock needs equal ends (digest 1.4), so the last tile fits both old ends and the ending is a capicua; the reachable case tests that going out on the locking tile is never a tranque. |
| `reducer.test.ts:870`: `locking the board with the double 6-6 on ends 6 and 6 as the last tile` | capicua_6_6_on_6_6_locks | |
| `reducer.test.ts:605`: `a tile that fits both ends but is not the last one` | tranque_both_ends_tile_not_last | |
| `reducer.test.ts:540`: `3-5 on ends 3 and 5` | capicua_3_5_on_3_5_left, capicua_3_5_on_3_5_right | |
| `reducer.test.ts:557`: `5-6 on ends 5 and 5` | capicua_5_6_on_5_5 | |
| `reducer.test.ts:563`: `the double 2-2 on ends 2 and 2` | capicua_2_2_on_2_2 | |
| `reducer.test.ts:579`: `the double 2-2 on ends 2 and 5 is a plain` | domino_2_2_on_2_5 | |
| `reducer.test.ts:586`: `the double 0-0 on ends 0 and 0` | capicua_0_0_on_0_0 | |
| `scoring.test.ts:213`: `4-4 on ends 2 and 2` | reject_4_4_on_2_2 | The tile cannot be played there, so the reachable case is the rejection. |
| `scoring.test.ts:224`: `false on an empty table` | no_capicua_on_empty_table | No round ends on a lead (the leader holds 7 tiles). |
| `reducer.test.ts:967`: `does NOT trigger VEINTICINCO after only 2 passes in 2v2` | no_corrido_after_two_passes | |
| `reducer.test.ts:1898`: `same player can earn VEINTICINCO twice in one round` | corrido_twice_same_player | |
| `reducer.test.ts:1879`: `after a pase corrido the forcer always has a play` | corrido_forcer_must_play | |
| `reducer.test.ts:2021`: `all bank into the same column` | corrido_domino_capicua_same_column | |
| `scoring.attribution.test.ts:106`: `no pase corrido heads-up` | no_corrido_heads_up | |
| `rules.hardening.test.ts:93`: `seeded games produce domino, capicua, trancao and (in 2v2) veinticinco` | seeded_games_produce_every_ending | Replays the golden traces of both modes at targets 100 and 200; 1v1 pays no bonus. |
| `rules.salida-and-tie.test.ts:80`: `round 1: pays 25 to the opener's side on the auto-placed 6-6` | salida_round1_auto_6_6 | |
| `rules.salida-and-tie.test.ts:128`: `a later round: pays 25 after a pass on a free opening tile` | salida_later_round | |
| `rules.salida-and-tie.test.ts:162`: `is cancelled when the partner passes too` | salida_cancelled_fourth_seat_play_pays_nothing | |
| `rules.salida-and-tie.test.ts:182`: `the pase corrido pays 25 once, not 50` | salida_cancelled_then_corrido_once | |
| `rules.salida-and-tie.test.ts:263`: `a later pass in the round never counts` | salida_later_pass_never_counts | |
| `rules.salida-and-tie.test.ts:238`: `heads-up there is none` | salida_none_heads_up | Capi's state (an empty boneyard on the opening tile) is unreachable in 1v1; the reachable case has the rival draw and checks that nothing pays. |
| `reducer.test.ts:2112`: `74 + 25 = 99` | corrido_paid_at_74 | |
| `reducer.test.ts:2120`: `75 + 25 = 100` | corrido_plain_at_75 | |
| `reducer.test.ts:2135`: `80 + 25 = 105` | corrido_plain_at_80 | |
| `rules.salida-and-tie.test.ts:206`: `plain play at 75 of 100, paid at 74` | salida_paid_at_74, salida_plain_at_75 | |
| `rules.hardening.test.ts:127`: `a pase corrido is paid only while it leaves the side below the target` | corrido_cap_hardening | |
| `flow.endgame.test.ts:548`: `one point short is round_over, the exact total is finished` | edge_1v1_domino_one_short, edge_1v1_domino_exact, edge_1v1_capicua_one_short, edge_1v1_capicua_exact, edge_1v1_tranque_blocker_one_short, edge_1v1_tranque_blocker_exact, edge_1v1_tranque_rival_one_short, edge_1v1_tranque_rival_exact, edge_1v1_tranque_tie_one_short, edge_1v1_tranque_tie_exact, edge_2v2_domino_one_short, edge_2v2_domino_exact, edge_2v2_capicua_one_short, edge_2v2_capicua_exact, edge_2v2_tranque_blocker_one_short, edge_2v2_tranque_blocker_exact, edge_2v2_tranque_rival_one_short, edge_2v2_tranque_rival_exact, edge_2v2_tranque_tie_one_short, edge_2v2_tranque_tie_exact | the five ending kinds of flow.endgame.test.ts:547 in both modes, the other side at target - 1 |
| `flow.endgame.test.ts:574`: `pays at 26 short, a plain move at 25 short, and never ends the game` | corrido_paid_26_short_of_100, corrido_plain_25_short_of_100, corrido_plain_1_short_of_100, corrido_paid_26_short_of_200, corrido_plain_25_short_of_200, corrido_plain_1_short_of_200, salida_paid_26_short_of_100, salida_plain_25_short_of_100, salida_plain_1_short_of_100, salida_paid_26_short_of_200, salida_plain_25_short_of_200, salida_plain_1_short_of_200 | at targets 100 and 200, the other side at target - 1; paid at 26 short, plain at 25 and at 1 short |
| `reducer.test.ts:1537`: `awards bonus AND finishes game` | capicua_wins_game | |
| `rules.hardening.test.ts:59`: `a tile in hand with a bogus end is rejected` | bogus_end_cannot_be_expressed | Wire input; End has only left and right, and input parsing (M2) validates at the boundary. |
| `rules.hardening.test.ts:81`: `non-canonical tile shapes are rejected` | non_canonical_tile_shapes_are_rejected | The tile type cannot hold a pip outside 0 to 6. |
| `reducer.ts:135-145`: `if (!starterTile || d[0] > starterTile[0]) {` | open_1v1_lower_double | hard constraint 2 |
| `reducer.ts:146-159`: `if (sum > bestSum) {` | open_1v1_no_double, open_1v1_no_double_tie_to_earlier_seat | hard constraint 2 |
| `reducer.ts:98-109`: `if (sum > bestSum) {` | open_1v1_no_double_first_in_hand_order | hard constraint 2 |
| Parra and Trucco 2025, immortal game (digest 3.6) | parra_trucco_immortal_game | Capi gives 198 to n and s (salida 25, two pases corridos 50, domino 123); the paper's 111 uses other scoring. |
| Parra and Trucco 2025, minimal tranque A (digest 3.6) | parra_trucco_minimal_tranque | Capi gives 126 to e and w (21 against 39). |
| Parra and Trucco 2025, the "107" deal (digest 3.6) | parra_trucco_107 | Capi gives 151 to n and s (salida 25, tranque 126, 5 against 50). |

## Perft

lines(position, d) is 1 when d is 0 or the round is over at the position; otherwise it is the sum, over the canonical moves (Moves, above), of lines(child, d - 1). The complete-line count is lines(position, infinity). Divide lists lines(child, d - 1) for each canonical root move. On Capi's side, tools/capi-oracle/perft.mjs collapses Capi's accepted intents with the canonical map and plays each canonical move with the intent that gives DominoFish's exact state (a lead in the in-hand orientation with end left; end left when both ends show one pip), so both sides count the same tree.
