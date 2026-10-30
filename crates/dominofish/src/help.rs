//! The command-line help: a banner with the command list, and each
//! command's usage and options.

pub struct Command {
    pub name: &'static str,
    pub group: &'static str,
    pub summary: &'static str,
    pub usage: &'static str,
    pub options: &'static str,
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "selfplay",
        group: "Play",
        summary: "Play a random game and print every hand, move, and score",
        usage: "dominofish selfplay --mode 1v1|2v2 --seed N [--target 100|200]
           [--record FILE]",
        options: "  --mode 1v1|2v2      Heads-up or partners (parejas)
  --seed N            Seed of the deals and the moves
  --target 100|200    Points that win the game (default 100)
  --record FILE       Also write the game as a record (docs/FORMATS.md, Game
                      records), for dominofish analyze",
    },
    Command {
        name: "analyze",
        group: "Play",
        summary: "Rank the legal moves at one point of a game record (PIMC)",
        usage: "dominofish analyze --record FILE --round R --ply K [--worlds N]
           [--seed N] [--nodes N] [--table-bits B] [--objective points|win]
           [--equity DIR]",
        options: "  --record FILE       A game record (docs/FORMATS.md, Game records)
  --round R           The round, from 1
  --ply K             Moves already played in that round; the seat on turn
                      after them is analyzed, from what it can see
  --worlds N          Worlds sampled from that seat's information, 1 to 4096
                      (default 64)
  --seed N            Seed of the sampling (default 1)
  --nodes N           Node cap per solve (default 10000000)
  --table-bits B      Table of 2^B entries (default 16, at most 24)
  --objective OBJ     points (default): the round margin; win: the
                      probability to win the game, from the equity table W
  --equity DIR        Where the W tables are (default docs/eval/equity)

  Each value is sampled: the mean round points of the seat's team after the
  move, or its win probability, over worlds each solved exactly, with its
  standard error.",
    },
    Command {
        name: "engine",
        group: "Play",
        summary: "Run a bot over the engine protocol on stdin and stdout",
        usage: "dominofish engine --bot NAME [--worlds N] [--salt N] [--nodes N]
           [--table-bits B] [--objective points|win] [--equity DIR]",
        options: "  --bot NAME          random, heaviest (bota gorda), accompanied (la más
                      acompañada), rule (counts tiles, reads pass voids),
                      pimc (samples the hidden hands its seat cannot see and
                      solves each world), or omniscient: it sees every hand
                      and plays the exact solver's move, an upper bound that
                      cheats on purpose
  --worlds N          PIMC: worlds per move, 1 to 4096 (default 64)
  --nodes N           PIMC and omniscient: node cap per solve (default
                      10000000)
  --table-bits B      PIMC and omniscient: table of 2^B entries (default 16
                      for PIMC, 20 for omniscient; at most 24)
  --objective OBJ     PIMC: points (default), the round margin, or win, the
                      probability to win the game from the equity table W
  --equity DIR        PIMC: where the W tables are (default docs/eval/equity)
  --salt N            Mixed into the seed the referee sends, so two copies of
                      one bot play with independent randomness

  The protocol is in docs/FORMATS.md, Engine protocol.",
    },
    Command {
        name: "match",
        group: "Measure",
        summary: "Play duplicate pairs between two engines; optional SPRT",
        usage: "dominofish match --mode 1v1|2v2 --unit round|game --target 100|200
           --pairs N --seed N --engine-seed N --a CMD --b CMD --out FILE
           [--timeout-ms N] [--sprt MU0,MU1 [--sprt-track]] [--aivat J]",
        options: "  --mode 1v1|2v2      Heads-up or partners (parejas)
  --unit round|game   Score round 1 of each game, or play to the target
  --target 100|200    Points that win the game
  --pairs N           Duplicate pairs: one deal played twice, sides swapped
                      (the maximum when --sprt is set)
  --seed N            Pair i plays deal seed N + i
  --engine-seed N     Base of the engines' own seeds, apart from the deals
  --a CMD, --b CMD    Engine commands, run from runs/bin/<commit>/ copies
  --out FILE          Result file: header, seeds, one line per pair, summary
  --timeout-ms N      Deadline per reply (default 10000), a safety limit
  --sprt MU0,MU1      SPRT on the pair margin, alpha = beta = 0.05; stops at
                      a decision
  --sprt-track        Play every pair and report the first decision
  --aivat J           Also report the result corrected by AIVAT control
                      variates: each deal and draw against J sampled
                      outcomes (1 to 64), each baseline bot's choice against
                      its tied moves; the play and the raw result do not
                      change",
    },
    Command {
        name: "bench",
        group: "Measure",
        summary: "Solve the bench positions; prints Bench: <nodes>",
        usage: "dominofish bench [--positions FILE] [--nodes N] [--table-bits B]",
        options: "  --positions FILE    State text lines (default: fixtures/bench.txt, built
                      into the binary)
  --nodes N           Node cap per solve (default 10000000)
  --table-bits B      Table of 2^B entries of 16 bytes (default 20, at most 24)",
    },
    Command {
        name: "solve-stats",
        group: "Measure",
        summary: "Time and nodes of exact solves by tiles left",
        usage: "dominofish solve-stats --mode 1v1|2v2 --games N --seed N [--nodes N]
           [--table-bits B]",
        options: "  --mode 1v1|2v2      Heads-up or partners (parejas)
  --games N           Games; every position of rounds 1 and 2 is solved
  --seed N            Seed of the deals and the random moves
  --nodes N           Node cap per solve (default 10000000)
  --table-bits B      Table of 2^B entries of 16 bytes (default 20, at most 24)",
    },
    Command {
        name: "pimc-stats",
        group: "Measure",
        summary: "PIMC time per move and nodes by tiles left",
        usage: "dominofish pimc-stats --mode 1v1|2v2 --games N --seed N [--worlds N]
           [--nodes N] [--table-bits B]",
        options: "  --mode 1v1|2v2      Heads-up or partners (parejas)
  --games N           Random games (the selfplay games of seeds N to N+games-1);
                      every decision with more than one legal move is timed
  --seed N            First selfplay seed
  --worlds N          Worlds per move, 1 to 4096 (default 64)
  --nodes N           Node cap per solve (default 10000000)
  --table-bits B      Table of 2^B entries (default 16, at most 24)",
    },
    Command {
        name: "calibrate",
        group: "Measure",
        summary: "Calibration of PIMC's win probability in self-play games",
        usage: "dominofish calibrate --mode 1v1|2v2 --target 100|200 --games N --seed N
           --engine-seed N --out FILE [--threads N] [--worlds N] [--nodes N]
           [--table-bits B] [--equity DIR]
       dominofish calibrate --from FILE",
        options: "  --mode 1v1|2v2      Heads-up or partners (parejas)
  --target 100|200    The game's target score
  --games N           Self-play games between copies of PIMC with the win
                      objective
  --seed N            Game g deals from the seed N + g
  --engine-seed N     Base of the engines' own seeds, apart from the deals
  --out FILE          Every searched decision's prediction and baseline, and
                      the summary (docs/FORMATS.md, Calibration)
  --threads N         Games at once, 1 to 4 (default 1); the file is the same
                      at any count
  --worlds N          Worlds per move, 1 to 4096 (default 64)
  --nodes N           Node cap per solve (default 10000000)
  --table-bits B      Table of 2^B entries (default 16, at most 24)
  --equity DIR        Where the W tables are (default docs/eval/equity)
  --from FILE         Print the summary of a calibration file instead

  The summary: the Brier score of the engine's win probability and of the
  score-only baseline (W at the round's start), the Brier skill, each with
  a 95% interval from bootstrap resamples of whole games, and a
  reliability table in ten bins.",
    },
    Command {
        name: "equity",
        group: "Measure",
        summary: "The game equity table W from the engine's own rounds",
        usage: "dominofish equity --mode 1v1|2v2 --rounds N --seed N --engine-seed N
           --out-dir DIR [--threads N] [--worlds N] [--nodes N] [--table-bits B]
       dominofish equity --from FILE --out-dir DIR [--check GAMES]",
        options: "  --mode 1v1|2v2      Heads-up or partners (parejas)
  --rounds N          Rounds after the first to record, from self-play games
                      to 200 between copies of PIMC
  --seed N            Game g deals from the seed N + g
  --engine-seed N     Base of the engines' own seeds, apart from the deals
  --threads N         Games at once, 1 to 4 (default 1); the files are the
                      same at any count
  --worlds N          Worlds per move, 1 to 4096 (default 64)
  --nodes N           Node cap per solve (default 10000000)
  --table-bits B      Table of 2^B entries (default 16, at most 24)
  --from FILE         Build the tables from a rounds file instead
  --out-dir DIR       Writes rounds-<mode>.txt and w-<mode>-100.txt and
                      w-<mode>-200.txt (docs/FORMATS.md)
  --check GAMES       Also simulate GAMES games from five round starts per
                      target and print W beside the share won",
    },
    Command {
        name: "world-stats",
        group: "Measure",
        summary: "Exact world counts at decisions of random games, by board",
        usage: "dominofish world-stats --mode 1v1|2v2 --games N --seed N",
        options: "  --mode 1v1|2v2      Heads-up or partners (parejas)
  --games N           Random games (the selfplay games of seeds N to N+games-1)
  --seed N            First selfplay seed",
    },
    Command {
        name: "bench-positions",
        group: "Measure",
        summary: "Print full-round positions of seeded games",
        usage: "dominofish bench-positions --games N --seed N",
        options: "  --games N           Games per mode
  --seed N            Seed of the deals and the random moves",
    },
    Command {
        name: "report",
        group: "Measure",
        summary: "Build docs/eval/REPORT.md from the result files",
        usage: "dominofish report [--results DIR] [--out FILE] [--check]",
        options: "  --results DIR       Result directories (default docs/eval/results)
  --out FILE          The report (default docs/eval/REPORT.md)
  --check             Exit 1 when the report on disk is not current",
    },
    Command {
        name: "capi-fuzz",
        group: "Verify",
        summary: "Play rounds in lockstep with Capi's engine and compare",
        usage: "dominofish capi-fuzz --mode 1v1|2v2 --rounds N --seed N",
        options: "  --mode 1v1|2v2      Heads-up or partners (parejas)
  --rounds N          Rounds to play; the run fails unless every rule path
                      is reached (the fast check plays 10000 rounds)
  --seed N            Seed of the deals and the moves

  Reads Capi from CAPI_DIR (default /Users/Adelson/Desktop/personal/capi),
  which must hold the pinned commit, and runs it with the Node.js in NODE
  (default Node v20 from nvm); prints SKIP and exits 1 without the pin.",
    },
    Command {
        name: "playout-bench",
        group: "Verify",
        summary: "Time random playouts on one core, with a checksum",
        usage: "dominofish playout-bench --mode 1v1|2v2 --rounds N --seed N",
        options: "  --mode 1v1|2v2      Heads-up or partners (parejas)
  --rounds N          Rounds to play out
  --seed N            Seed, at most 2^32 - 1",
    },
];

pub fn find(name: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.name == name)
}

/// The width of the command-name column in the overview.
const NAME_COLUMN: usize = 17;

/// The banner and the command list.
pub fn overview() -> String {
    let mut out = format!(
        "DominoFish {}\nThe Stockfish of Dominican Dominoes.\n\nUsage: dominofish <command> [options]\n",
        env!("CARGO_PKG_VERSION")
    );
    let mut group = "";
    for c in COMMANDS {
        if c.group != group {
            group = c.group;
            out.push_str(&format!("\n{group}\n"));
        }
        out.push_str(&format!("  {:<NAME_COLUMN$}{}\n", c.name, c.summary));
    }
    out.push_str("\nRun \"dominofish <command> --help\" for its options.\n");
    out
}

pub fn command(c: &Command) -> String {
    format!(
        "{}\n\nUsage: {}\n\nOptions:\n{}\n",
        c.summary, c.usage, c.options
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_fits_80_columns() {
        assert!(COMMANDS.iter().all(|c| c.name.len() + 2 <= NAME_COLUMN));
        let mut texts = vec![overview()];
        texts.extend(COMMANDS.iter().map(command));
        for text in texts {
            for line in text.lines() {
                assert!(
                    line.chars().count() <= 80,
                    "{} columns: {line}",
                    line.chars().count()
                );
            }
        }
    }
}
