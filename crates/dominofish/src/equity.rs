//! dominofish equity (M6-01; docs/PLAN.md D20): the engine's own rounds and
//! the game equity tables W built from them. Self-play games to 200 between
//! copies of PIMC record every round after the first, re-scored without the
//! bonus cap (`equity::rescore`); W for targets 100 and 200 follows from
//! the recursion of `Equity::build`. Games run on up to `threads` threads,
//! each seeded by its index alone, and the rounds are taken in game order,
//! so the files are the same at any thread count.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use dominofish_core::equity::{Equity, RoundOutcome, rescore};
use dominofish_core::infoset::InfoSet;
use dominofish_core::pimc::{self, Limits};
use dominofish_core::record::{GameRecord, RoundRecord};
use dominofish_core::rng::{Rng, derive};
use dominofish_core::rules::{Deal, Mode, MoveList, Phase, State};

/// The target of the recorded games: many rounds after the first per game.
pub const GAME_TARGET: u16 = 200;
/// The targets W is built for.
pub const TARGETS: [u16; 2] = [100, 200];

/// A recorded round: its game, its round (from 0), its leader's seat, and
/// its outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorded {
    pub game: u64,
    pub round: usize,
    pub leader: u8,
    pub outcome: RoundOutcome,
}

/// The work of the games that gave the rounds: games, moves, searched
/// decisions (a forced move is played without search), and capped solves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Work {
    pub games: u64,
    pub moves: u64,
    pub decisions: u64,
    pub capped: u64,
}

/// Game `g`: round r deals from derive(deal_base + g, r), as the harness
/// deals; seat s searches with seeds from derive(derive(engine_base, g),
/// 100 + s), reseeded each round as an engine is (engine.rs). A world that
/// disagrees with its InfoSet, or none at all, voids the run.
fn play_game(
    mode: Mode,
    g: u64,
    deal_base: u64,
    engine_base: u64,
    limits: &Limits,
) -> Result<(GameRecord, Work), String> {
    let deal_seed = deal_base + g;
    let deal_for = |r: u64| Deal::shuffled(&mut Rng::new(derive(deal_seed, r)));
    let mut record = GameRecord {
        mode,
        target: GAME_TARGET,
        rounds: vec![RoundRecord {
            deal: deal_for(0),
            moves: vec![],
        }],
    };
    let (mut state, _) = State::first_round(mode, GAME_TARGET, &record.rounds[0].deal);
    let mut work = Work {
        games: 1,
        ..Work::default()
    };
    let mut legal = MoveList::new();
    loop {
        let r = record.rounds.len() - 1;
        let mut rngs: Vec<Rng> = (0..mode.seats())
            .map(|s| {
                let seat = derive(derive(engine_base, g), 100 + u64::from(s));
                Rng::new(derive(seat, r as u64 + 1))
            })
            .collect();
        while state.phase == Phase::Playing {
            let seat = state.turn;
            state.legal_moves(&mut legal);
            let mv = if legal.len() == 1 {
                legal.as_slice()[0]
            } else {
                let ply = record.rounds[r].moves.len();
                let info = InfoSet::new(&record, r, ply, seat)
                    .map_err(|e| format!("game {g} round {} ply {ply}: {e:?}", r + 1))?;
                let a = pimc::analyze(&info, limits, rngs[usize::from(seat)].next_u64());
                if a.inconsistent > 0 || a.fallback > 0 {
                    return Err(format!(
                        "game {g} round {} ply {ply}: {} inconsistent worlds, {} fallbacks",
                        r + 1,
                        a.inconsistent,
                        a.fallback
                    ));
                }
                work.decisions += 1;
                work.capped += u64::from(a.capped);
                a.best
            };
            state.play(mv);
            record.rounds[r].moves.push(mv);
            work.moves += 1;
        }
        if state.phase == Phase::Finished {
            return Ok((record, work));
        }
        let deal = deal_for(record.rounds.len() as u64);
        state = state.next_round(&deal);
        record.rounds.push(RoundRecord {
            deal,
            moves: vec![],
        });
    }
}

/// The rounds after the first of game `g`, re-scored.
fn later_rounds(record: &GameRecord, g: u64) -> Vec<Recorded> {
    (1..record.rounds.len())
        .map(|r| {
            let (start, _) = record.round_start(r).expect("a played record replays");
            let round = &record.rounds[r];
            Recorded {
                game: g,
                round: r,
                leader: start.starter,
                outcome: rescore(record.mode, &round.deal, start.starter, &round.moves),
            }
        })
        .collect()
}

/// The first `wanted` rounds after the first of games 0, 1, 2, ... in game
/// order, from games played on `threads` threads, and the work of the games
/// they came from. A game that a thread finishes after the rounds are
/// complete is not used.
pub fn record(
    mode: Mode,
    wanted: usize,
    deal_base: u64,
    engine_base: u64,
    limits: &Limits,
    threads: usize,
) -> Result<(Vec<Recorded>, Work), String> {
    type Done = BTreeMap<u64, Result<(Vec<Recorded>, Work), String>>;
    let next = AtomicU64::new(0);
    let stop = AtomicBool::new(false);
    let done: Mutex<Done> = Mutex::new(BTreeMap::new());
    // Whether games 0, 1, ... in order already hold `wanted` rounds or a
    // failure, which ends the recording.
    let complete = |d: &Done| {
        let mut have = 0;
        for (i, (&g, result)) in d.iter().enumerate() {
            if g != i as u64 {
                return false;
            }
            match result {
                Err(_) => return true,
                Ok((rounds, _)) => have += rounds.len(),
            }
            if have >= wanted {
                return true;
            }
        }
        false
    };
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                while !stop.load(Ordering::SeqCst) {
                    let g = next.fetch_add(1, Ordering::SeqCst);
                    let result = play_game(mode, g, deal_base, engine_base, limits)
                        .map(|(record, work)| (later_rounds(&record, g), work));
                    let mut d = done.lock().expect("the results lock");
                    d.insert(g, result);
                    if complete(&d) {
                        stop.store(true, Ordering::SeqCst);
                    }
                }
            });
        }
    });
    let mut rounds = Vec::new();
    let mut work = Work::default();
    for (_, result) in done.into_inner().expect("the results lock") {
        let (r, w) = result?;
        rounds.extend(r);
        work.games += w.games;
        work.moves += w.moves;
        work.decisions += w.decisions;
        work.capped += w.capped;
        if rounds.len() >= wanted {
            rounds.truncate(wanted);
            return Ok((rounds, work));
        }
    }
    Err(format!("only {} rounds were recorded", rounds.len()))
}

/// The rounds file (docs/FORMATS.md, Equity rounds and tables).
pub fn rounds_text(mode: Mode, settings: &str, rounds: &[Recorded], work: &Work) -> String {
    let mut out = format!(
        "# equity rounds mode={} target={GAME_TARGET} {settings}\n",
        mode.name()
    );
    for r in rounds {
        let o = &r.outcome;
        let bonuses: String = if o.bonuses.is_empty() {
            "-".into()
        } else {
            o.bonuses
                .iter()
                .map(|&l| if l { 'L' } else { 'O' })
                .collect()
        };
        let _ = writeln!(
            out,
            "round {} {} leader {} won {} points {} bonuses {bonuses}",
            r.game,
            r.round + 1,
            r.leader,
            u8::from(o.leader_won),
            o.points
        );
    }
    let _ = writeln!(
        out,
        "# summary rounds={} games={} moves={} decisions={} capped={}",
        rounds.len(),
        work.games,
        work.moves,
        work.decisions,
        work.capped
    );
    out
}

/// The mode and the round outcomes of a rounds file.
pub fn parse_rounds(text: &str) -> Result<(Mode, Vec<RoundOutcome>), String> {
    let mode = text
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("# equity rounds mode="))
        .and_then(|l| l.split_whitespace().next())
        .and_then(|m| match m {
            "1v1" => Some(Mode::OneVsOne),
            "2v2" => Some(Mode::TwoVsTwo),
            _ => None,
        })
        .ok_or("not an equity rounds file")?;
    let mut rounds = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.first() != Some(&"round") {
            continue;
        }
        let bad = || format!("line {}: {line}", n + 1);
        if w.len() != 11
            || w[3] != "leader"
            || w[5] != "won"
            || w[7] != "points"
            || w[9] != "bonuses"
        {
            return Err(bad());
        }
        let bonuses = match w[10] {
            "-" => vec![],
            b => b
                .chars()
                .map(|c| match c {
                    'L' => Ok(true),
                    'O' => Ok(false),
                    _ => Err(bad()),
                })
                .collect::<Result<_, _>>()?,
        };
        rounds.push(RoundOutcome {
            leader_won: match w[6] {
                "1" => true,
                "0" => false,
                _ => return Err(bad()),
            },
            points: w[8].parse().map_err(|_| bad())?,
            bonuses,
        });
    }
    if rounds.is_empty() {
        return Err("the file holds no rounds".into());
    }
    Ok((mode, rounds))
}

/// W in millionths, as the table file and the solver hold it.
fn millionths(p: f64) -> u32 {
    (p * 1e6).round() as u32
}

/// The table file of W for `target` (docs/FORMATS.md): a line per lead and
/// own score with W in millionths for each score of the other team, and
/// the largest difference between the tables of the rounds' two halves.
pub fn table_text(mode: Mode, w: &Equity, rounds: &[RoundOutcome], from: &str) -> String {
    let target = w.target;
    let half = rounds.len() / 2;
    let (first, second) = (
        Equity::build(target, &rounds[..half]),
        Equity::build(target, &rounds[half..]),
    );
    let mut diff = 0.0f64;
    for leads in [true, false] {
        for a in 0..target {
            for b in 0..target {
                diff = diff.max((first.win(a, b, leads) - second.win(a, b, leads)).abs());
            }
        }
    }
    let mut out = format!(
        "# equity table mode={} target={target} rounds={} from={from}\n# halves: the tables of rounds 1 to {half} and {} to {} differ by at most {} millionths\n",
        mode.name(),
        rounds.len(),
        half + 1,
        rounds.len(),
        millionths(diff)
    );
    for (name, leads) in [("lead", true), ("other", false)] {
        for a in 0..target {
            let _ = write!(out, "{name} {a}");
            for b in 0..target {
                let _ = write!(out, " {}", millionths(w.win(a, b, leads)));
            }
            out.push('\n');
        }
    }
    out
}

/// The round starts `--check` simulates for a target: (our score, theirs,
/// whether we lead).
pub fn check_starts(target: u16) -> [(u16, u16, bool); 5] {
    let t = target;
    [
        (0, 0, true),
        (0, 0, false),
        (t / 2, t / 2, true),
        (t * 4 / 5, t * 19 / 20, false),
        (t * 9 / 10, t / 2, true),
    ]
}

/// Games played from `start` by drawing rounds at random from `rounds`,
/// with the scoring rules applied here on their own: the share our team
/// wins, against W's value from the recursion, with its z score.
pub fn simulate(
    w: &Equity,
    rounds: &[RoundOutcome],
    start: (u16, u16, bool),
    games: u32,
    seed: u64,
) -> (f64, f64, f64) {
    let mut rng = Rng::new(seed);
    let goal = u32::from(w.target);
    let mut won = 0u32;
    for _ in 0..games {
        let (mut ours, mut theirs, mut we_lead) = (u32::from(start.0), u32::from(start.1), start.2);
        loop {
            let r = &rounds[rng.below(rounds.len() as u32) as usize];
            for &to_leader in &r.bonuses {
                let to_us = to_leader == we_lead;
                let s = if to_us { &mut ours } else { &mut theirs };
                if *s + 25 < goal {
                    *s += 25;
                }
            }
            let we_won = r.leader_won == we_lead;
            if we_won {
                ours += u32::from(r.points);
            } else {
                theirs += u32::from(r.points);
            }
            if ours >= goal || theirs >= goal {
                won += u32::from(we_won);
                break;
            }
            we_lead = we_won;
        }
    }
    let p = w.win(start.0, start.1, start.2);
    let share = f64::from(won) / f64::from(games);
    let se = (p * (1.0 - p) / f64::from(games)).sqrt().max(1e-9);
    (share, p, (share - p) / se)
}
