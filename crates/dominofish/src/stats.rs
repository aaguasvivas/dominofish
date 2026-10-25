//! Match statistics (M2-07): 95% confidence intervals and the SPRT on
//! per-pair results. The SPRT is a normal-approximation GSPRT with the
//! sample variance (docs/PLAN.md, T1 and D14).

/// Mean, sample standard deviation, and a 95% normal confidence interval of
/// the mean.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    pub n: usize,
    pub mean: f64,
    pub sd: f64,
    pub lo: f64,
    pub hi: f64,
}

pub fn summary(xs: &[f64]) -> Summary {
    let n = xs.len();
    let mean = xs.iter().sum::<f64>() / (n.max(1) as f64);
    let var = if n > 1 {
        xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64
    } else {
        0.0
    };
    let sd = var.sqrt();
    let half = 1.96 * sd / (n.max(1) as f64).sqrt();
    Summary {
        n,
        mean,
        sd,
        lo: mean - half,
        hi: mean + half,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    H0,
    H1,
}

/// A sequential test of H0: mean mu0 against H1: mean mu1 on per-pair
/// results.
#[derive(Clone, Debug)]
pub struct Sprt {
    pub mu0: f64,
    pub mu1: f64,
    pub alpha: f64,
    pub beta: f64,
    /// No decision before this many pairs, while the variance is unstable.
    pub min_n: usize,
    n: usize,
    sum: f64,
    sumsq: f64,
    /// The first decision and the pair count at which it came, kept when a
    /// run plays on past it (a fixed-size table run).
    first: Option<(Decision, usize)>,
}

impl Sprt {
    pub fn new(mu0: f64, mu1: f64, alpha: f64, beta: f64) -> Sprt {
        assert!(mu1 > mu0);
        Sprt {
            mu0,
            mu1,
            alpha,
            beta,
            min_n: 16,
            n: 0,
            sum: 0.0,
            sumsq: 0.0,
            first: None,
        }
    }

    pub fn push(&mut self, x: f64) {
        self.n += 1;
        self.sum += x;
        self.sumsq += x * x;
        if self.first.is_none() {
            self.first = self.decision().map(|d| (d, self.n));
        }
    }

    /// The sequential test's outcome: its first decision, if any.
    pub fn first_decision(&self) -> Option<(Decision, usize)> {
        self.first
    }

    pub fn n(&self) -> usize {
        self.n
    }

    /// The lower (accept H0) and upper (accept H1) LLR bounds.
    pub fn bounds(&self) -> (f64, f64) {
        (
            (self.beta / (1.0 - self.alpha)).ln(),
            ((1.0 - self.beta) / self.alpha).ln(),
        )
    }

    /// The log-likelihood ratio with the sample variance. Results that are
    /// all equal (variance 0) count with a tiny variance, which decides by
    /// the side of the midpoint the mean falls on.
    pub fn llr(&self) -> f64 {
        if self.n < 2 {
            return 0.0;
        }
        let n = self.n as f64;
        let var = ((self.sumsq - self.sum * self.sum / n) / (n - 1.0)).max(1e-9);
        (self.mu1 - self.mu0) / var * (self.sum - n * (self.mu0 + self.mu1) / 2.0)
    }

    pub fn decision(&self) -> Option<Decision> {
        if self.n < self.min_n {
            return None;
        }
        let (lower, upper) = self.bounds();
        let llr = self.llr();
        if llr <= lower {
            Some(Decision::H0)
        } else if llr >= upper {
            Some(Decision::H1)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dominofish_core::bots::Bot;
    use dominofish_core::infoset::InfoSet;
    use dominofish_core::record::{GameRecord, RoundRecord};
    use dominofish_core::rng::{Rng, derive};
    use dominofish_core::rules::{Deal, Mode, Phase, State, team};

    #[test]
    fn summary_and_interval_match_the_formulas() {
        let s = summary(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        assert_eq!(s.n, 8);
        assert!((s.mean - 5.0).abs() < 1e-12);
        let sd = (32.0f64 / 7.0).sqrt();
        assert!((s.sd - sd).abs() < 1e-12);
        assert!((s.lo - (5.0 - 1.96 * sd / 8f64.sqrt())).abs() < 1e-12);
        let c = summary(&[3.0; 10]);
        assert_eq!((c.lo, c.hi), (3.0, 3.0));
    }

    #[test]
    fn sprt_bounds_and_simple_decisions() {
        let mut t = Sprt::new(0.0, 4.0, 0.05, 0.05);
        let (lo, hi) = t.bounds();
        assert!((lo + 2.944_438_979).abs() < 1e-6 && (hi - 2.944_438_979).abs() < 1e-6);
        for _ in 0..15 {
            t.push(0.0);
        }
        assert_eq!(t.decision(), None, "no decision before 16 pairs");
        t.push(0.0);
        assert_eq!(
            t.decision(),
            Some(Decision::H0),
            "identical results at 0 accept H0"
        );
        let mut u = Sprt::new(0.0, 4.0, 0.05, 0.05);
        for k in 0..400 {
            u.push(if k % 2 == 0 { 30.0 } else { -10.0 });
        }
        assert_eq!(
            u.decision(),
            Some(Decision::H1),
            "a mean of 10 against bounds 0 and 4"
        );
    }

    fn normal(rng: &mut Rng) -> f64 {
        let u = ((rng.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
        let v = ((rng.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
        (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
    }

    /// One duplicate pair of 2v2 first rounds in process, side A (`a`)
    /// against side B (`b`): A's points minus B's over both games.
    fn round_pair(a: Bot, b: Bot, deal_seed: u64) -> f64 {
        let deal = Deal::shuffled(&mut Rng::new(derive(deal_seed, 0)));
        let mut margin = 0i32;
        for g in 0..2usize {
            let bots: [Bot; 4] = core::array::from_fn(|s| if team(s as u8) == g { a } else { b });
            let mut record = GameRecord {
                mode: Mode::TwoVsTwo,
                target: 100,
                rounds: vec![RoundRecord {
                    deal,
                    moves: vec![],
                }],
            };
            let (mut state, _) = State::first_round(Mode::TwoVsTwo, 100, &deal);
            let mut rngs: [Rng; 4] =
                core::array::from_fn(|s| Rng::new(derive(deal_seed, 100 + s as u64)));
            while state.phase == Phase::Playing {
                let seat = state.turn;
                let info = InfoSet::new(&record, 0, record.rounds[0].moves.len(), seat).unwrap();
                let mv = bots[seat as usize].decide(&info, &mut rngs[seat as usize]);
                state.play(mv);
                record.rounds[0].moves.push(mv);
            }
            margin += i32::from(state.scores[g]) - i32::from(state.scores[1 - g]);
        }
        f64::from(margin)
    }

    /// T1 (docs/PLAN.md, pre-registered): error rates of the SPRT on
    /// synthetic per-pair results.
    #[test]
    fn t1_sprt_error_rates_match_alpha_and_beta() {
        const RUNS: u64 = 4000;
        const MAX: usize = 20_000;
        let band = 3.0 * (0.05f64 * 0.95 / RUNS as f64).sqrt();
        let empirical: Vec<f64> = {
            let raw: Vec<f64> = (0..2000)
                .map(|i| round_pair(Bot::Rule, Bot::Accompanied, 7_100_000 + i))
                .collect();
            let m = raw.iter().sum::<f64>() / raw.len() as f64;
            raw.iter().map(|x| x - m).collect()
        };
        let emp_sd = summary(&empirical).sd;
        let configs: [(&str, f64, f64); 2] =
            [("normal", 40.0, 4.0), ("empirical", emp_sd, 0.1 * emp_sd)];
        let mut total_capped = 0u64;
        let mut lines = Vec::new();
        for (c, &(name, sd, mu1)) in configs.iter().enumerate() {
            let mut wrong = [0u64; 2];
            for (h, truth) in [0.0, mu1].into_iter().enumerate() {
                for k in 0..RUNS {
                    let mut rng =
                        Rng::new(7_000_000 + (c as u64) * 1_000_000 + (h as u64) * 100_000 + k);
                    let mut t = Sprt::new(0.0, mu1, 0.05, 0.05);
                    let decision = loop {
                        let x = if c == 0 {
                            truth + sd * normal(&mut rng)
                        } else {
                            truth + empirical[rng.below(empirical.len() as u32) as usize]
                        };
                        t.push(x);
                        if let Some(d) = t.decision() {
                            break Some(d);
                        }
                        if t.n() >= MAX {
                            break None;
                        }
                    };
                    match (h, decision) {
                        (0, Some(Decision::H1)) | (1, Some(Decision::H0)) => wrong[h] += 1,
                        (_, None) => total_capped += 1,
                        _ => {}
                    }
                }
            }
            let rates = [wrong[0] as f64 / RUNS as f64, wrong[1] as f64 / RUNS as f64];
            lines.push(format!(
                "{name} (sd {sd:.2}, H1 {mu1:.2}): false H1 {:.4}, false H0 {:.4}",
                rates[0], rates[1]
            ));
            for r in rates {
                assert!(
                    (r - 0.05).abs() <= band,
                    "{name}: error rate {r} outside 0.05 +- {band:.4}"
                );
            }
        }
        let capped_share = total_capped as f64 / (4 * RUNS) as f64;
        assert!(
            capped_share < 0.01,
            "{capped_share} of runs reached the maximum"
        );
        println!(
            "sprt t1: {}; band [{:.4}, {:.4}]; runs at the maximum {total_capped}",
            lines.join("; "),
            0.05 - band,
            0.05 + band
        );
    }
}
