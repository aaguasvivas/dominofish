//! M1-05: the edge cases of capi-rules-spec[2] and the three Parra and
//! Trucco deals as named tests. Each scenario in fixtures/edges.txt comes from
//! Capi's engine (tools/capi-oracle/edges.mjs); the replay compares every
//! event and state with Capi's, and each test then checks what its case is
//! about. docs/RULES.md (Edge cases) maps every Capi test cite to a test here.
//! The numbers stay in the fixture: no expected value is typed by hand.

mod common;

use std::sync::OnceLock;

use common::{Played, Step};
use dominofish_core::rules::{End, Event, Mode, Move, Phase, State, team};
use dominofish_core::tile::{ALL, SUIT, Tile, mask_pips};

fn scenarios() -> &'static Vec<Played> {
    static ALL_PLAYED: OnceLock<Vec<Played>> = OnceLock::new();
    ALL_PLAYED.get_or_init(|| common::replay(&common::fixture("edges.txt")).games)
}

/// The scenario, replayed with 0 mismatches against Capi.
fn run(name: &str) -> &'static Played {
    let p = scenarios()
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("no scenario {name} in fixtures/edges.txt"));
    assert_eq!(p.mismatches, 0, "{name} differs from Capi");
    assert!(!p.steps.is_empty() || p.init.is_some());
    p
}

fn init(p: &Played) -> State {
    p.init.expect("scenario has a deal")
}

fn last(p: &Played) -> &Step {
    p.steps.last().expect("scenario has moves")
}

fn events(p: &Played) -> Vec<Event> {
    p.steps.iter().filter_map(|s| s.event).collect()
}

fn step(p: &Played, f: impl Fn(&Step) -> bool) -> &Step {
    p.steps.iter().find(|s| f(s)).expect("the scenario's step")
}

fn index(p: &Played, f: impl Fn(&Step) -> bool) -> usize {
    p.steps.iter().position(f).expect("the scenario's step")
}

fn tile(a: u8, b: u8) -> Tile {
    Tile::new(a, b)
}

fn played_tile(s: &Step) -> Option<Tile> {
    match s.mv {
        Move::Play { tile, .. } => Some(tile),
        _ => None,
    }
}

fn locked(s: &State) -> bool {
    s.board != 0 && ALL & !s.board & (SUIT[s.left as usize] | SUIT[s.right as usize]) == 0
}

fn fits(s: &State, mask: u32) -> bool {
    mask & (SUIT[s.left as usize] | SUIT[s.right as usize]) != 0
}

fn gained(s: &Step) -> u32 {
    let seat = s.before.turn as usize;
    (s.after.hands[seat] & !s.before.hands[seat]).count_ones()
}

fn team_pips(s: &State, t: usize) -> u32 {
    (0..s.mode.seats())
        .filter(|&x| team(x) == t)
        .map(|x| mask_pips(s.hands[x as usize]))
        .sum()
}

fn salida_shape(s: &State) -> bool {
    s.mode == Mode::TwoVsTwo
        && s.board.count_ones() == 1
        && s.last_played_by == s.starter
        && s.passes == 1
}

#[test]
fn edge_scenarios_replay_with_zero_mismatches() {
    let report = common::replay(&common::fixture("edges.txt"));
    for e in report.errors.iter().take(10) {
        println!("{e}");
    }
    println!(
        "edges: scenarios {} moves {} mismatches {}",
        report.games.len(),
        report.moves,
        report.mismatches
    );
    assert_eq!(report.mismatches, 0);
}

// ----------------------------------------------------------------- opening

#[test]
fn start_1v1_starter_has_6() {
    let s = init(run("start-1v1-starter-has-6"));
    let other = s.mode.next(s.starter);
    assert_eq!(s.hands[s.starter as usize].count_ones(), 6);
    assert_eq!(s.hands[other as usize].count_ones(), 7);
}

#[test]
fn start_1v1_boneyard_14() {
    assert_eq!(init(run("start-1v1-boneyard-14")).boneyard().len(), 14);
}

#[test]
fn start_2v2_no_boneyard() {
    let s = init(run("start-2v2-no-boneyard"));
    assert!(s.boneyard().is_empty());
    let held: u32 = s.hands.iter().map(|h| h.count_ones()).sum();
    assert_eq!(held + s.board.count_ones(), 28);
}

#[test]
fn turn_order_n_e_s_w() {
    let p = run("turn-order-n-e-s-w");
    let seats: Vec<u8> = p.steps[..4].iter().map(|s| s.before.turn).collect();
    assert_eq!(seats, [0, 1, 2, 3]);
    assert!(
        p.steps[..4]
            .iter()
            .all(|s| matches!(s.mv, Move::Play { .. }))
    );
}

#[test]
fn open_1v1_lower_double() {
    let (_, t) = run("open-1v1-lower-double").opening.unwrap();
    assert!(t.is_double() && t.low() < 6);
}

#[test]
fn open_1v1_no_double() {
    let p = run("open-1v1-no-double");
    let (seat, t) = p.opening.unwrap();
    assert!(!t.is_double());
    let s = init(p);
    let dealt = s.hands[0] | s.hands[1] | t.bit();
    assert!(dominofish_core::tile::tiles(dealt).all(|x| x.pips() <= t.pips()));
    assert_eq!(s.starter, seat);
}

#[test]
fn open_1v1_no_double_tie_to_earlier_seat() {
    let (seat, t) = run("open-1v1-no-double-tie-to-earlier-seat")
        .opening
        .unwrap();
    assert_eq!((seat, t), (0, tile(3, 6)));
}

#[test]
fn open_1v1_no_double_first_in_hand_order() {
    let (seat, t) = run("open-1v1-no-double-first-in-hand-order")
        .opening
        .unwrap();
    assert_eq!((seat, t), (0, tile(4, 5)));
}

// -------------------------------------------------------------------- draw

#[test]
fn draw_stops_at_first_fit() {
    let p = run("draw-stops-at-first-fit");
    let s = step(p, |s| s.mv == Move::Draw && gained(s) >= 2);
    let seat = s.before.turn as usize;
    assert_eq!(s.after.turn, s.before.turn);
    let new = s.after.hands[seat] & !s.before.hands[seat];
    let fitting = new & (SUIT[s.after.left as usize] | SUIT[s.after.right as usize]);
    assert_eq!(fitting.count_ones(), 1, "only the last tile drawn fits");
    assert_eq!(
        s.after.hands[seat] & (SUIT[s.after.left as usize] | SUIT[s.after.right as usize]),
        fitting
    );
}

#[test]
fn draw_exhausts_then_pass() {
    let p = run("draw-exhausts-then-pass");
    let i = index(p, |s| s.mv == Move::Draw && s.after.boneyard().is_empty());
    let s = &p.steps[i];
    assert!(!fits(&s.after, s.after.hands[s.before.turn as usize]));
    assert_eq!(p.steps[i + 1].mv, Move::Pass);
    assert_eq!(p.steps[i + 1].before.turn, s.before.turn);
}

#[test]
fn drawn_tiles_count_in_tranque() {
    let p = run("drawn-tiles-count-in-tranque");
    let i = (0..p.steps.len() - 1)
        .find(|&k| {
            p.steps[k].mv == Move::Draw
                && gained(&p.steps[k]) >= 2
                && matches!(p.steps[k + 1].event, Some(Event::Tranque { .. }))
        })
        .expect("a draw of two or more tiles, then the locking play");
    let next = &p.steps[i + 1];
    let Some(Event::Tranque {
        blocker,
        blocker_pips,
        ..
    }) = next.event
    else {
        panic!("the play after the draw locks the board");
    };
    assert_eq!(blocker, p.steps[i].before.turn);
    assert_eq!(
        u32::from(blocker_pips),
        mask_pips(next.after.hands[blocker as usize])
    );
}

#[test]
fn reject_draw_in_2v2() {
    let p = run("reject-draw-in-2v2");
    assert!(
        p.rejected
            .iter()
            .any(|(s, mv)| *mv == Move::Draw && !s.is_legal(Move::Draw))
    );
}

#[test]
fn reject_pass_with_boneyard() {
    let p = run("reject-pass-with-boneyard");
    assert!(
        p.rejected
            .iter()
            .any(|(s, mv)| *mv == Move::Pass && !s.boneyard().is_empty())
    );
}

// -------------------------------------------------------------------- lock

#[test]
fn lock_with_boneyard_tiles() {
    let s = last(run("lock-with-boneyard-tiles"));
    assert!(matches!(s.event, Some(Event::Tranque { .. })));
    assert!(!s.after.boneyard().is_empty());
}

fn boneyard_keeps_board_open(name: &str) {
    let p = run(name);
    let s = step(p, |s| {
        let pile = s.after.boneyard().iter().fold(0, |m, t| m | t.bit());
        matches!(s.mv, Move::Play { .. })
            && s.after.phase == Phase::Playing
            && !fits(&s.after, s.after.hands[0] | s.after.hands[1])
            && fits(&s.after, pile)
    });
    assert!(!locked(&s.after));
}

#[test]
fn no_lock_while_boneyard_fits() {
    boneyard_keeps_board_open("no-lock-while-boneyard-fits");
}

#[test]
fn boneyard_tile_keeps_board_open() {
    boneyard_keeps_board_open("boneyard-tile-keeps-board-open");
}

#[test]
fn empty_table_never_locked() {
    let s = init(run("empty-table-never-locked"));
    assert_eq!((s.board, s.phase), (0, Phase::Playing));
}

// ----------------------------------------------------------------- tranque

#[test]
fn tranque_on_locking_tile() {
    let p = run("tranque-on-locking-tile");
    let s = last(p);
    assert!(matches!(s.event, Some(Event::Tranque { .. })));
    assert!(matches!(s.mv, Move::Play { .. }) && locked(&s.after));
}

#[test]
fn tranque_blocker_hand_decides() {
    let s = last(run("tranque-blocker-hand-decides"));
    let Some(Event::Tranque {
        blocker, winner, ..
    }) = s.event
    else {
        panic!("tranque");
    };
    assert_eq!(winner, blocker);
    assert!(team_pips(&s.after, team(blocker)) > team_pips(&s.after, 1 - team(blocker)));
}

#[test]
fn tranque_w_blocks_n_compares() {
    let s = last(run("tranque-w-blocks-n-compares"));
    assert!(matches!(
        s.event,
        Some(Event::Tranque {
            blocker: 3,
            rival: 0,
            ..
        })
    ));
}

fn tie(name: &str) -> (u8, u8, u8, u8) {
    let s = last(run(name));
    let Some(Event::Tranque {
        blocker,
        rival,
        blocker_pips,
        rival_pips,
        winner,
        ..
    }) = s.event
    else {
        panic!("tranque");
    };
    assert_eq!(blocker_pips, rival_pips);
    (s.before.starter, blocker, rival, winner)
}

#[test]
fn tranque_tie_opener_n_blocker_s_rival_w() {
    assert_eq!(tie("tranque-tie-opener-n-blocker-s-rival-w"), (0, 2, 3, 0));
}

#[test]
fn tranque_tie_opener_w_blocker_n_rival_e() {
    assert_eq!(tie("tranque-tie-opener-w-blocker-n-rival-e"), (3, 0, 1, 3));
}

#[test]
fn tranque_tie_1v1_opener() {
    let (opener, _, _, winner) = tie("tranque-tie-1v1-opener");
    assert_eq!(winner, opener);
}

// --------------------------------------------------------- domino vs lock

#[test]
fn going_out_on_locking_tile_is_domino() {
    // From a fresh deal a lock needs equal ends, so the last tile fit both
    // old ends: the ending is a capicua, never a tranque.
    let s = last(run("going-out-on-locking-tile-is-domino"));
    assert!(matches!(s.event, Some(Event::Capicua { .. })));
    assert!(locked(&s.after));
}

#[test]
fn capicua_6_6_on_6_6_locks() {
    let s = last(run("capicua-6-6-on-6-6-locks"));
    assert!(matches!(s.event, Some(Event::Capicua { .. })));
    assert_eq!(played_tile(s), Some(tile(6, 6)));
    assert!(locked(&s.after));
}

#[test]
fn tranque_both_ends_tile_not_last() {
    let s = last(run("tranque-both-ends-tile-not-last"));
    let t = played_tile(s).unwrap();
    assert!(t.has(s.before.left) && t.has(s.before.right));
    assert!(matches!(s.event, Some(Event::Tranque { .. })));
}

// ----------------------------------------------------------------- capicua

fn capicua(name: &str, t: Tile, ends: [u8; 2]) -> &'static Step {
    let s = last(run(name));
    assert_eq!(played_tile(s), Some(t));
    let mut before = [s.before.left, s.before.right];
    before.sort();
    assert_eq!(before, ends);
    s
}

#[test]
fn capicua_3_5_on_3_5_left() {
    let s = capicua("capicua-3-5-on-3-5-left", tile(3, 5), [3, 5]);
    assert!(matches!(s.mv, Move::Play { end: End::Left, .. }));
    assert!(matches!(s.event, Some(Event::Capicua { .. })));
}

#[test]
fn capicua_3_5_on_3_5_right() {
    let s = capicua("capicua-3-5-on-3-5-right", tile(3, 5), [3, 5]);
    assert!(matches!(
        s.mv,
        Move::Play {
            end: End::Right,
            ..
        }
    ));
    assert!(matches!(s.event, Some(Event::Capicua { .. })));
}

#[test]
fn capicua_5_6_on_5_5() {
    let s = capicua("capicua-5-6-on-5-5", tile(5, 6), [5, 5]);
    assert!(matches!(s.event, Some(Event::Capicua { .. })));
}

#[test]
fn capicua_2_2_on_2_2() {
    let s = capicua("capicua-2-2-on-2-2", tile(2, 2), [2, 2]);
    assert!(matches!(s.event, Some(Event::Capicua { .. })));
}

#[test]
fn domino_2_2_on_2_5() {
    let s = capicua("domino-2-2-on-2-5", tile(2, 2), [2, 5]);
    assert!(matches!(s.event, Some(Event::Domino { .. })));
}

#[test]
fn capicua_0_0_on_0_0() {
    let s = capicua("capicua-0-0-on-0-0", tile(0, 0), [0, 0]);
    assert!(matches!(s.event, Some(Event::Capicua { .. })));
}

#[test]
fn reject_4_4_on_2_2() {
    let p = run("reject-4-4-on-2-2");
    assert_eq!(p.rejected.len(), 2);
    for (s, mv) in &p.rejected {
        assert_eq!((s.left, s.right), (2, 2));
        assert_eq!(
            played_tile(&Step {
                before: *s,
                mv: *mv,
                event: None,
                after: *s
            }),
            Some(tile(4, 4))
        );
    }
}

#[test]
fn no_capicua_on_empty_table() {
    // A leader holds 7 tiles, so a lead never ends a round.
    let p = run("no-capicua-on-empty-table");
    let s = &p.steps[0];
    assert_eq!(s.before.board, 0);
    assert_eq!((s.event, s.after.phase), (None, Phase::Playing));
}

// ------------------------------------------------------------ pase corrido

#[test]
fn no_corrido_after_two_passes() {
    let p = run("no-corrido-after-two-passes");
    let i = (0..p.steps.len() - 3)
        .find(|&k| {
            matches!(p.steps[k].mv, Move::Play { .. })
                && p.steps[k + 1].mv == Move::Pass
                && p.steps[k + 2].mv == Move::Pass
                && matches!(p.steps[k + 3].mv, Move::Play { .. })
        })
        .unwrap();
    assert_eq!((p.steps[i + 1].event, p.steps[i + 2].event), (None, None));
}

#[test]
fn corrido_forcer_must_play() {
    let p = run("corrido-forcer-must-play");
    let i = index(p, |s| matches!(s.event, Some(Event::PaseCorrido { .. })));
    let forcer = p.steps[i].before.last_played_by;
    assert_eq!(p.steps[i].after.turn, forcer);
    assert!(matches!(p.steps[i + 1].mv, Move::Play { .. }));
}

#[test]
fn corrido_twice_same_player() {
    let p = run("corrido-twice-same-player");
    let forcers: Vec<u8> = p
        .steps
        .iter()
        .filter(|s| matches!(s.event, Some(Event::PaseCorrido { .. })))
        .map(|s| s.before.last_played_by)
        .collect();
    assert!(
        forcers
            .iter()
            .enumerate()
            .any(|(i, f)| forcers[..i].contains(f))
    );
}

#[test]
fn corrido_domino_capicua_same_column() {
    let p = run("corrido-domino-capicua-same-column");
    let s = last(p);
    let Some(Event::Capicua { seat, points }) = s.event else {
        panic!("capicua");
    };
    let t = team(seat);
    let corridos = events(p)
        .iter()
        .filter(|e| **e == Event::PaseCorrido { team: t })
        .count() as u16;
    assert!(corridos >= 1);
    assert_eq!(
        s.after.scores[t],
        init(p).scores[t] + 25 * corridos + points
    );
}

#[test]
fn no_corrido_heads_up() {
    let p = run("no-corrido-heads-up");
    assert_eq!(init(p).mode, Mode::OneVsOne);
    assert!(
        p.steps
            .iter()
            .any(|s| s.mv == Move::Pass && s.event.is_none())
    );
    assert!(
        !events(p)
            .iter()
            .any(|e| matches!(e, Event::PaseCorrido { .. } | Event::PaseSalida { .. }))
    );
}

#[test]
fn seeded_games_produce_every_ending() {
    // Over every golden trace: 1v1 has dominoes and tranques, some game has a
    // capicua, 2v2 has a pase corrido, and 1v1 never pays a bonus
    // (rules.hardening.test.ts:93-115).
    let mut events = [Vec::new(), Vec::new()];
    for name in ["1v1-100", "1v1-200", "2v2-100", "2v2-200"] {
        let report = common::replay(&common::fixture(&format!("golden/{name}.txt")));
        assert_eq!(report.mismatches, 0, "{name}");
        let k = usize::from(name.starts_with("2v2"));
        events[k].extend(
            report
                .games
                .iter()
                .flat_map(|g| g.steps.iter().filter_map(|s| s.event)),
        );
    }
    let has = |k: usize, f: fn(&Event) -> bool| events[k].iter().any(f);
    assert!(has(0, |e| matches!(e, Event::Domino { .. })));
    assert!(has(0, |e| matches!(e, Event::Tranque { .. })));
    assert!(
        has(0, |e| matches!(e, Event::Capicua { .. }))
            || has(1, |e| matches!(e, Event::Capicua { .. }))
    );
    assert!(has(1, |e| matches!(e, Event::PaseCorrido { .. })));
    assert!(!has(0, |e| matches!(
        e,
        Event::PaseCorrido { .. } | Event::PaseSalida { .. }
    )));
}

// ------------------------------------------------------------------ salida

#[test]
fn salida_round1_auto_6_6() {
    let p = run("salida-round1-auto-6-6");
    assert_eq!(p.opening.unwrap().1, tile(6, 6));
    let s = step(p, |s| {
        salida_shape(&s.before) && matches!(s.mv, Move::Play { .. })
    });
    assert_eq!(
        s.event,
        Some(Event::PaseSalida {
            team: team(s.before.starter)
        })
    );
}

#[test]
fn salida_later_round() {
    let p = run("salida-later-round");
    assert!(p.opening.is_none());
    let s = step(p, |s| {
        salida_shape(&s.before) && matches!(s.mv, Move::Play { .. })
    });
    assert_eq!(
        s.event,
        Some(Event::PaseSalida {
            team: team(s.before.starter)
        })
    );
}

#[test]
fn salida_cancelled_fourth_seat_play_pays_nothing() {
    let p = run("salida-cancelled-fourth-seat-play-pays-nothing");
    let i = index(p, |s| salida_shape(&s.before) && s.mv == Move::Pass);
    assert_eq!(p.steps[i].event, None);
    assert!(matches!(p.steps[i + 1].mv, Move::Play { .. }));
    assert_eq!(p.steps[i + 1].event, None);
}

#[test]
fn salida_cancelled_then_corrido_once() {
    let p = run("salida-cancelled-then-corrido-once");
    let first: Vec<Option<Event>> = p.steps[..4].iter().map(|s| s.event).collect();
    assert_eq!(
        first,
        [None, None, None, Some(Event::PaseCorrido { team: 0 })]
    );
    assert_eq!(p.steps[3].after.scores[0], init(p).scores[0] + 25);
}

#[test]
fn salida_later_pass_never_counts() {
    let p = run("salida-later-pass-never-counts");
    let s = step(p, |s| {
        s.before.board.count_ones() > 1
            && s.before.last_played_by == s.before.starter
            && s.before.passes == 1
            && s.before.turn == s.before.mode.next(s.before.mode.next(s.before.starter))
            && matches!(s.mv, Move::Play { .. })
    });
    assert!(!matches!(s.event, Some(Event::PaseSalida { .. })));
}

#[test]
fn salida_none_heads_up() {
    // Capi's case needs an empty boneyard on the opening tile, which no
    // fresh 1v1 deal reaches; here the rival draws instead and nothing pays.
    let p = run("salida-none-heads-up");
    assert_eq!(p.steps[1].mv, Move::Draw);
    assert!(
        !events(p)
            .iter()
            .any(|e| matches!(e, Event::PaseSalida { .. }))
    );
}

fn bonus_at(name: &str, salida: bool) -> (u16, Option<Event>) {
    let p = run(name);
    let i = if salida {
        index(p, |s| {
            salida_shape(&s.before) && matches!(s.mv, Move::Play { .. })
        })
    } else {
        index(p, |s| {
            s.mv == Move::Pass && s.before.passes == 2 && s.before.mode == Mode::TwoVsTwo
        })
    };
    let s = &p.steps[i];
    let t = team(if salida {
        s.before.starter
    } else {
        s.before.last_played_by
    });
    assert_eq!(
        s.before.scores[t],
        init(p).scores[t],
        "first bonus of the round for that side"
    );
    if s.event.is_some() {
        // A paid bonus adds 25 and never ends the game.
        assert_eq!(s.after.scores[t], s.before.scores[t] + 25);
    } else {
        // A plain move pays nothing.
        assert_eq!(s.after.scores, s.before.scores);
    }
    assert_eq!(s.after.phase, Phase::Playing);
    (s.before.target - s.before.scores[t], s.event)
}

fn salida_paid(name: &str, short: u16) {
    let (got, ev) = bonus_at(name, true);
    assert_eq!(got, short);
    assert!(matches!(ev, Some(Event::PaseSalida { .. })), "{name}");
}

fn salida_plain(name: &str, short: u16) {
    assert_eq!(bonus_at(name, true), (short, None), "{name}");
}

fn corrido_paid(name: &str, short: u16) {
    let (got, ev) = bonus_at(name, false);
    assert_eq!(got, short);
    assert!(matches!(ev, Some(Event::PaseCorrido { .. })), "{name}");
}

fn corrido_plain(name: &str, short: u16) {
    assert_eq!(bonus_at(name, false), (short, None), "{name}");
}

/// The side that did not score sits at target - 1 (flow.endgame.test.ts:556).
fn other_side_one_short(name: &str) {
    let p = run(name);
    let s = init(p);
    assert!(s.scores.contains(&(s.target - 1)), "{name}: {:?}", s.scores);
}

#[test]
fn salida_paid_at_74() {
    salida_paid("salida-paid-at-74", 26);
}

#[test]
fn salida_plain_at_75() {
    salida_plain("salida-plain-at-75", 25);
}

#[test]
fn salida_paid_26_short_of_100() {
    salida_paid("salida-paid-26-short-of-100", 26);
    other_side_one_short("salida-paid-26-short-of-100");
}

#[test]
fn salida_plain_25_short_of_100() {
    salida_plain("salida-plain-25-short-of-100", 25);
}

#[test]
fn salida_plain_1_short_of_100() {
    salida_plain("salida-plain-1-short-of-100", 1);
}

#[test]
fn salida_paid_26_short_of_200() {
    salida_paid("salida-paid-26-short-of-200", 26);
    other_side_one_short("salida-paid-26-short-of-200");
}

#[test]
fn salida_plain_25_short_of_200() {
    salida_plain("salida-plain-25-short-of-200", 25);
}

#[test]
fn salida_plain_1_short_of_200() {
    salida_plain("salida-plain-1-short-of-200", 1);
}

// --------------------------------------------------------------------- cap

#[test]
fn corrido_paid_at_74() {
    corrido_paid("corrido-paid-at-74", 26);
}

#[test]
fn corrido_plain_at_75() {
    corrido_plain("corrido-plain-at-75", 25);
}

#[test]
fn corrido_plain_at_80() {
    corrido_plain("corrido-plain-at-80", 20);
}

#[test]
fn corrido_cap_hardening() {
    corrido_plain("corrido-cap-hardening", 24);
}

#[test]
fn corrido_paid_26_short_of_100() {
    corrido_paid("corrido-paid-26-short-of-100", 26);
    other_side_one_short("corrido-paid-26-short-of-100");
}

#[test]
fn corrido_plain_25_short_of_100() {
    corrido_plain("corrido-plain-25-short-of-100", 25);
}

#[test]
fn corrido_plain_1_short_of_100() {
    corrido_plain("corrido-plain-1-short-of-100", 1);
}

#[test]
fn corrido_paid_26_short_of_200() {
    corrido_paid("corrido-paid-26-short-of-200", 26);
    other_side_one_short("corrido-paid-26-short-of-200");
}

#[test]
fn corrido_plain_25_short_of_200() {
    corrido_plain("corrido-plain-25-short-of-200", 25);
}

#[test]
fn corrido_plain_1_short_of_200() {
    corrido_plain("corrido-plain-1-short-of-200", 1);
}

// ------------------------------------------------------------ target edges

/// The round winner ends `short` below the target in `phase`, and the other
/// side stays at target - 1 (flow.endgame.test.ts:548-571).
fn edge(name: &str, short: u16, phase: Phase, kind: fn(&Event) -> bool) {
    let s = last(run(name));
    let event = s.event.expect("round end");
    assert!(kind(&event), "{name}: {event:?}");
    let winner = match event {
        Event::Domino { seat, .. } | Event::Capicua { seat, .. } => seat,
        Event::Tranque { winner, .. } => winner,
        other => panic!("{name}: round end expected, got {other:?}"),
    };
    let t = team(winner);
    assert_eq!(
        (
            s.before.target - s.after.scores[t].min(s.before.target),
            s.after.phase
        ),
        (short, phase),
        "{name}"
    );
    assert_eq!(
        s.after.scores[1 - t],
        s.before.target - 1,
        "{name}: the other side"
    );
}

fn domino(e: &Event) -> bool {
    matches!(e, Event::Domino { .. })
}

fn capicua_event(e: &Event) -> bool {
    matches!(e, Event::Capicua { .. })
}

fn blocker_wins(e: &Event) -> bool {
    matches!(e, Event::Tranque { blocker, winner, blocker_pips, rival_pips, .. } if blocker == winner && blocker_pips < rival_pips)
}

fn rival_wins(e: &Event) -> bool {
    matches!(e, Event::Tranque { rival, winner, blocker_pips, rival_pips, .. } if rival == winner && rival_pips < blocker_pips)
}

fn tie_to_opener(e: &Event) -> bool {
    matches!(e, Event::Tranque { blocker_pips, rival_pips, .. } if blocker_pips == rival_pips)
}

#[test]
fn edge_1v1_domino_one_short() {
    edge("edge-1v1-domino-one-short", 1, Phase::RoundOver, domino);
}

#[test]
fn edge_1v1_domino_exact() {
    edge("edge-1v1-domino-exact", 0, Phase::Finished, domino);
}

#[test]
fn edge_1v1_capicua_one_short() {
    edge(
        "edge-1v1-capicua-one-short",
        1,
        Phase::RoundOver,
        capicua_event,
    );
}

#[test]
fn edge_1v1_capicua_exact() {
    edge("edge-1v1-capicua-exact", 0, Phase::Finished, capicua_event);
}

#[test]
fn edge_1v1_tranque_blocker_one_short() {
    edge(
        "edge-1v1-tranque-blocker-one-short",
        1,
        Phase::RoundOver,
        blocker_wins,
    );
}

#[test]
fn edge_1v1_tranque_blocker_exact() {
    edge(
        "edge-1v1-tranque-blocker-exact",
        0,
        Phase::Finished,
        blocker_wins,
    );
}

#[test]
fn edge_1v1_tranque_rival_one_short() {
    edge(
        "edge-1v1-tranque-rival-one-short",
        1,
        Phase::RoundOver,
        rival_wins,
    );
}

#[test]
fn edge_1v1_tranque_rival_exact() {
    edge(
        "edge-1v1-tranque-rival-exact",
        0,
        Phase::Finished,
        rival_wins,
    );
}

#[test]
fn edge_1v1_tranque_tie_one_short() {
    edge(
        "edge-1v1-tranque-tie-one-short",
        1,
        Phase::RoundOver,
        tie_to_opener,
    );
}

#[test]
fn edge_1v1_tranque_tie_exact() {
    edge(
        "edge-1v1-tranque-tie-exact",
        0,
        Phase::Finished,
        tie_to_opener,
    );
}

#[test]
fn edge_2v2_domino_one_short() {
    edge("edge-2v2-domino-one-short", 1, Phase::RoundOver, domino);
}

#[test]
fn edge_2v2_domino_exact() {
    edge("edge-2v2-domino-exact", 0, Phase::Finished, domino);
}

#[test]
fn edge_2v2_capicua_one_short() {
    edge(
        "edge-2v2-capicua-one-short",
        1,
        Phase::RoundOver,
        capicua_event,
    );
}

#[test]
fn edge_2v2_capicua_exact() {
    edge("edge-2v2-capicua-exact", 0, Phase::Finished, capicua_event);
}

#[test]
fn edge_2v2_tranque_blocker_one_short() {
    edge(
        "edge-2v2-tranque-blocker-one-short",
        1,
        Phase::RoundOver,
        blocker_wins,
    );
}

#[test]
fn edge_2v2_tranque_blocker_exact() {
    edge(
        "edge-2v2-tranque-blocker-exact",
        0,
        Phase::Finished,
        blocker_wins,
    );
}

#[test]
fn edge_2v2_tranque_rival_one_short() {
    edge(
        "edge-2v2-tranque-rival-one-short",
        1,
        Phase::RoundOver,
        rival_wins,
    );
}

#[test]
fn edge_2v2_tranque_rival_exact() {
    edge(
        "edge-2v2-tranque-rival-exact",
        0,
        Phase::Finished,
        rival_wins,
    );
}

#[test]
fn edge_2v2_tranque_tie_one_short() {
    edge(
        "edge-2v2-tranque-tie-one-short",
        1,
        Phase::RoundOver,
        tie_to_opener,
    );
}

#[test]
fn edge_2v2_tranque_tie_exact() {
    edge(
        "edge-2v2-tranque-tie-exact",
        0,
        Phase::Finished,
        tie_to_opener,
    );
}

#[test]
fn capicua_wins_game() {
    let p = run("capicua-wins-game");
    let s = last(p);
    let Some(Event::Capicua { seat, points }) = s.event else {
        panic!("capicua");
    };
    assert_eq!(s.after.phase, Phase::Finished);
    assert!(
        s.before.scores[team(seat)] + points - 25 < s.before.target,
        "the bonus decides the game"
    );
}

// ------------------------------------------------------------ wire hardening

#[test]
#[should_panic]
fn non_canonical_tile_shapes_are_rejected() {
    // Capi rejects pips outside 0..6 at its wire boundary; DominoFish's tile
    // type cannot hold one.
    let _ = Tile::new(7, 0);
}

#[test]
fn bogus_end_cannot_be_expressed() {
    // Capi rejects ends other than left and right; End has only those two,
    // and a tile not in hand is rejected without changing the state.
    let p = run("reject-4-4-on-2-2");
    let (s, mv) = p.rejected[0];
    assert!(!s.is_legal(mv));
}

// ---------------------------------------------------------- published deals

fn kinds(p: &Played) -> Vec<&'static str> {
    events(p)
        .iter()
        .map(|e| match e {
            Event::PaseSalida { .. } => "salida",
            Event::PaseCorrido { .. } => "corrido",
            Event::Domino { .. } => "domino",
            Event::Capicua { .. } => "capicua",
            Event::Tranque { .. } => "tranque",
        })
        .collect()
}

#[test]
fn parra_trucco_immortal_game() {
    let p = run("parra-trucco-immortal-game");
    assert_eq!(kinds(p), ["salida", "corrido", "corrido", "domino"]);
    assert!(events(p).iter().all(|e| match e {
        Event::PaseSalida { team } | Event::PaseCorrido { team } => *team == 0,
        Event::Domino { seat, .. } => *seat == 0,
        _ => false,
    }));
}

#[test]
fn parra_trucco_minimal_tranque() {
    let p = run("parra-trucco-minimal-tranque");
    assert_eq!(kinds(p), ["tranque"]);
    assert!(matches!(
        last(p).event,
        Some(Event::Tranque {
            blocker: 1,
            rival: 2,
            winner: 1,
            ..
        })
    ));
}

#[test]
fn parra_trucco_107() {
    let p = run("parra-trucco-107");
    assert_eq!(kinds(p), ["salida", "tranque"]);
    assert!(matches!(
        last(p).event,
        Some(Event::Tranque {
            blocker: 0,
            rival: 1,
            winner: 0,
            ..
        })
    ));
}
