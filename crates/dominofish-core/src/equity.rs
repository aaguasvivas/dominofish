//! The game equity table W (M6; docs/PLAN.md D20): the probability that a
//! team wins a game to a target from the start of a round after the first,
//! given both teams' scores and whether that team leads. It is built from
//! rounds the engine played (`RoundOutcome`) by a recursion over the scores
//! that applies Capi's scoring at each score: the strict cap on pase
//! bonuses (R25), no cap on capicua (R22), and a game won only by the
//! winner of a round (R21).

use crate::rules::{BONUS, Deal, Event, MAX_TARGET, Mode, Move, State, team};

/// A round after the first, seen from its leader's team: whether that team
/// won it, its points (a capicua included), and the teams of its pase
/// bonuses in order (true: the leader's team), each as if it paid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundOutcome {
    pub leader_won: bool,
    pub points: u16,
    pub bonuses: Vec<bool>,
}

/// A played round re-scored from its start with scores 0 and the largest
/// target, so that every pase shape pays (R23 to R25) and the round's
/// outcome at any score follows from it. `moves` are the round's moves
/// from `deal` with `leader` to lead.
pub fn rescore(mode: Mode, deal: &Deal, leader: u8, moves: &[Move]) -> RoundOutcome {
    let mut state = State::dealt(mode, MAX_TARGET, [0, 0], deal, leader);
    let mine = team(leader);
    let mut bonuses = Vec::new();
    for &mv in moves {
        let (winner, points) = match state.play(mv).event {
            Some(Event::PaseSalida { team: t } | Event::PaseCorrido { team: t }) => {
                bonuses.push(t == mine);
                continue;
            }
            Some(Event::Domino { seat, points } | Event::Capicua { seat, points }) => {
                (seat, points)
            }
            Some(Event::Tranque { winner, points, .. }) => (winner, points),
            None => continue,
        };
        // At 0 a team can receive (MAX_TARGET - 1) / 25 = 11 bonuses: one
        // fewer shows that none was refused.
        for side in [true, false] {
            let n = bonuses.iter().filter(|&&b| b == side).count();
            assert!(
                n < usize::from((MAX_TARGET - 1) / BONUS),
                "{n} bonuses for one team"
            );
        }
        return RoundOutcome {
            leader_won: team(winner) == mine,
            points,
            bonuses,
        };
    }
    panic!("the moves do not end the round");
}

/// One round from a round start: the scores [ours, theirs] after it and
/// its winning team (0: ours), with our team leading when `leads`. Each
/// bonus pays 25 only while its team stays below `goal` by more than 25
/// (R25); the round's points go to its winner with no cap (R22).
fn apply(r: &RoundOutcome, leads: bool, mut s: [u32; 2], goal: u32) -> ([u32; 2], usize) {
    for &to_leader in &r.bonuses {
        let k = usize::from(to_leader != leads);
        if s[k] + u32::from(BONUS) < goal {
            s[k] += u32::from(BONUS);
        }
    }
    let k = usize::from(r.leader_won != leads);
    s[k] += u32::from(r.points);
    (s, k)
}

/// W for one target, from a team's view.
#[derive(Clone, Debug, PartialEq)]
pub struct Equity {
    pub target: u16,
    /// [the team leads, the other team leads][its score * target + theirs].
    w: [Vec<f64>; 2],
}

impl Equity {
    /// The recursion of D20 over `rounds`, from high scores down: each start
    /// is the mean over the rounds of the start each one leads to.
    pub fn build(target: u16, rounds: &[RoundOutcome]) -> Equity {
        assert!(!rounds.is_empty() && target <= MAX_TARGET);
        let t = usize::from(target);
        let goal = u32::from(target);
        let n = rounds.len() as f64;
        let mut w = [vec![0.0; t * t], vec![0.0; t * t]];
        for a in (0..t).rev() {
            for b in (0..t).rev() {
                let start = [a as u32, b as u32];
                // For the team leading (l = 0) or not: c[l], the sum of the
                // outcomes of rounds that leave (a, b), and p[l][k], the
                // rounds that return to (a, b) with team k leading (0: ours).
                let mut c = [0.0f64; 2];
                let mut p = [[0.0f64; 2]; 2];
                for (l, cl) in c.iter_mut().enumerate() {
                    let leads = l == 0;
                    for r in rounds {
                        // k: the round's winning team, which leads next.
                        let (s, k) = apply(r, leads, start, goal);
                        if s[k] >= goal {
                            *cl += if k == 0 { 1.0 } else { 0.0 };
                        } else if s == start {
                            p[l][k] += 1.0;
                        } else {
                            *cl += w[k][s[0] as usize * t + s[1] as usize];
                        }
                    }
                }
                // x = (c0 + p00 x + p01 y) / n and y = (c1 + p10 x + p11 y) / n.
                let (a11, a12, a21, a22) = (n - p[0][0], -p[0][1], -p[1][0], n - p[1][1]);
                let det = a11 * a22 - a12 * a21;
                assert!(det > 0.0, "every round returns to the same scores");
                w[0][a * t + b] = (c[0] * a22 - a12 * c[1]) / det;
                w[1][a * t + b] = (a11 * c[1] - a21 * c[0]) / det;
            }
        }
        Equity { target, w }
    }

    /// The probability that a team wins from the start of a round with it at
    /// `mine`, the other team at `theirs`, and `leads` saying whether it
    /// leads the round.
    pub fn win(&self, mine: u16, theirs: u16, leads: bool) -> f64 {
        let t = usize::from(self.target);
        self.w[usize::from(!leads)][usize::from(mine) * t + usize::from(theirs)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    fn round(leader_won: bool, points: u16, bonuses: &[bool]) -> RoundOutcome {
        RoundOutcome {
            leader_won,
            points,
            bonuses: bonuses.to_vec(),
        }
    }

    /// Rounds drawn at random, with bonuses and some rounds of 0 points.
    fn random_rounds(n: usize, seed: u64) -> Vec<RoundOutcome> {
        let mut rng = Rng::new(seed);
        (0..n)
            .map(|_| {
                let bonuses: Vec<bool> = (0..rng.below(3)).map(|_| rng.below(2) == 0).collect();
                let points = if rng.below(10) == 0 {
                    0
                } else {
                    rng.below(70) as u16
                };
                round(rng.below(5) < 3, points, &bonuses)
            })
            .collect()
    }

    #[test]
    fn a_round_that_always_reaches_the_target_decides_the_game() {
        let w = Equity::build(100, &[round(true, 60, &[])]);
        assert_eq!((w.win(0, 0, true), w.win(0, 0, false)), (1.0, 0.0));
        assert_eq!((w.win(50, 90, true), w.win(50, 90, false)), (1.0, 0.0));
    }

    #[test]
    fn rounds_that_return_to_the_same_scores_solve_two_equations() {
        // The leader wins 0 or 100: it leads again until it wins.
        let w = Equity::build(100, &[round(true, 0, &[]), round(true, 100, &[])]);
        assert_eq!((w.win(0, 0, true), w.win(0, 0, false)), (1.0, 0.0));
        // The leader loses 0 (the lead passes) or wins 100: x = 1/2 + y/2
        // and y = x/2, so x = 2/3 and y = 1/3.
        let w = Equity::build(100, &[round(false, 0, &[]), round(true, 100, &[])]);
        assert!((w.win(0, 0, true) - 2.0 / 3.0).abs() < 1e-12);
        assert!((w.win(0, 0, false) - 1.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn a_bonus_pays_only_below_the_target_by_more_than_25() {
        // R25: at 74 a bonus pays (99 < 100), at 75 it does not.
        let r = round(true, 1, &[true]);
        assert_eq!(apply(&r, true, [74, 0], 100), ([100, 0], 0));
        assert_eq!(apply(&r, true, [75, 0], 100), ([76, 0], 0));
        // Seen from the team that does not lead: the bonus is theirs.
        let r = round(false, 1, &[true]);
        assert_eq!(apply(&r, false, [0, 74], 100), ([1, 99], 0));
        // R22: the round's points have no cap, so a capicua can pass 100.
        let r = round(true, 50, &[]);
        assert_eq!(apply(&r, true, [99, 0], 100), ([149, 0], 0));
    }

    #[test]
    fn a_rescored_round_gives_the_real_scores_at_the_real_start() {
        // Every round after the first of seeded bot games: the round
        // re-scored without the cap, then applied at its real start scores
        // with the cap, ends at the scores the rules engine gave it.
        let (mut checked, mut refused) = (0, 0);
        for mode in [Mode::TwoVsTwo, Mode::OneVsOne] {
            for (k, &bot) in crate::bots::Bot::ALL.iter().enumerate() {
                for g in 0..40u64 {
                    let target = if g % 2 == 0 { 100 } else { 200 };
                    let record = crate::referee::play_bots(
                        mode,
                        target,
                        [bot; 4],
                        7000 + 100 * k as u64 + g,
                    );
                    for r in 1..record.rounds.len() {
                        let (start, _) = record.round_start(r).unwrap();
                        let mut end = start;
                        for &mv in &record.rounds[r].moves {
                            end.play(mv);
                        }
                        let o = rescore(
                            mode,
                            &record.rounds[r].deal,
                            start.starter,
                            &record.rounds[r].moves,
                        );
                        let (mine, other) = (team(start.starter), 1 - team(start.starter));
                        let s = [
                            u32::from(start.scores[mine]),
                            u32::from(start.scores[other]),
                        ];
                        let (got, _) = apply(&o, true, s, u32::from(target));
                        let want = [u32::from(end.scores[mine]), u32::from(end.scores[other])];
                        assert_eq!(got, want, "{mode:?} {bot:?} game {g} round {r}");
                        let paid = (want[0] + want[1] - s[0] - s[1] - u32::from(o.points)) / 25;
                        refused += o.bonuses.len() - paid as usize;
                        checked += 1;
                    }
                }
            }
        }
        assert!(
            checked > 500 && refused > 0,
            "{checked} rounds, {refused} refused bonuses"
        );
        println!(
            "equity rescore: {checked} rounds after the first of bot games, re-scored and applied at their start scores, equal the rules engine's end scores; {refused} bonuses refused at the cap"
        );
    }

    #[test]
    fn the_two_teams_values_sum_to_one() {
        let rounds = random_rounds(300, 1);
        for target in [100, 200] {
            let w = Equity::build(target, &rounds);
            for a in 0..target {
                for b in 0..target {
                    let s = w.win(a, b, true) + w.win(b, a, false);
                    assert!((s - 1.0).abs() < 1e-9, "{target} {a} {b}: {s}");
                }
            }
        }
    }

    #[test]
    fn the_recursion_equals_a_simulation_of_games() {
        let rounds = random_rounds(200, 2);
        let target = 100;
        let w = Equity::build(target, &rounds);
        let mut rng = Rng::new(3);
        let games = 40_000;
        for (a, b, leads) in [(0, 0, true), (0, 0, false), (60, 85, true), (90, 40, false)] {
            let mut won = 0u32;
            for _ in 0..games {
                let (mut s, mut mine_leads) = ([u32::from(a), u32::from(b)], leads);
                loop {
                    let r = &rounds[rng.below(rounds.len() as u32) as usize];
                    for &to_leader in &r.bonuses {
                        let k = usize::from(to_leader != mine_leads);
                        if s[k] + 25 < u32::from(target) {
                            s[k] += 25;
                        }
                    }
                    let k = usize::from(r.leader_won != mine_leads);
                    s[k] += u32::from(r.points);
                    if s[k] >= u32::from(target) {
                        won += u32::from(k == 0);
                        break;
                    }
                    mine_leads = k == 0;
                }
            }
            let p = w.win(a, b, leads);
            let se = (p * (1.0 - p) / f64::from(games)).sqrt().max(1e-9);
            let z = (f64::from(won) / f64::from(games) - p) / se;
            assert!(z.abs() < 4.0, "({a}, {b}, {leads}): W {p}, simulated z {z}");
        }
    }
}
