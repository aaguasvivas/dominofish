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
        summary: "Play a game of random legal moves and print every hand and move",
        usage: "dominofish selfplay --mode 1v1|2v2 --seed N [--target 100|200]",
        options: "  --mode 1v1|2v2      Heads-up or partners (parejas)
  --seed N            Seed of the deals and the moves
  --target 100|200    Points that win the game (default 100)",
    },
    Command {
        name: "engine",
        group: "Play",
        summary: "Run a bot that speaks the engine protocol on stdin and stdout",
        usage: "dominofish engine --bot random|heaviest|accompanied|rule [--salt N]",
        options: "  --bot NAME          random, heaviest (bota gorda), accompanied (la mas
                      acompanada), or rule (counts tiles, reads pass voids)
  --salt N            Mixed into the seed the referee sends, so two copies of
                      one bot play with independent randomness

  The protocol is in docs/FORMATS.md, Engine protocol.",
    },
    Command {
        name: "match",
        group: "Measure",
        summary: "Play duplicate pairs between two engines, with an optional SPRT",
        usage: "dominofish match --mode 1v1|2v2 --unit round|game --target 100|200
           --pairs N --seed N --engine-seed N --a CMD --b CMD --out FILE
           [--timeout-ms N] [--sprt MU0,MU1 [--sprt-track]]",
        options: "  --mode 1v1|2v2      Heads-up or partners (parejas)
  --unit round|game   Score round 1 of each game, or play to the target
  --target 100|200    Points that win the game
  --pairs N           Duplicate pairs: one deal played twice, sides swapped
                      (the maximum when --sprt is set)
  --seed N            Pair i plays deal seed N + i
  --engine-seed N     Base of the engines' own seeds, apart from the deals
  --a CMD, --b CMD    Engine commands; binaries must live under runs/bin/<commit>/
  --out FILE          Result file: header, seeds, one line per pair, summary
  --timeout-ms N      Deadline per reply (default 10000), a safety limit
  --sprt MU0,MU1      SPRT on the pair margin, alpha = beta = 0.05; stops at
                      a decision
  --sprt-track        Play every pair and report the first decision",
    },
    Command {
        name: "report",
        group: "Measure",
        summary: "Build docs/eval/REPORT.md from the match result files",
        usage: "dominofish report [--results DIR] [--out FILE] [--check]",
        options: "  --results DIR       Result directories (default docs/eval/results)
  --out FILE          The report (default docs/eval/REPORT.md)
  --check             Exit 1 when the report on disk is not current",
    },
    Command {
        name: "capi-fuzz",
        group: "Verify",
        summary: "Play random rounds in lockstep with Capi's engine and compare",
        usage: "dominofish capi-fuzz --mode 1v1|2v2 --rounds N --seed N",
        options: "  --rounds N          Rounds to play
  --seed N            Seed of the deals and the moves

  Needs CAPI_DIR to hold the pinned Capi commit; prints SKIP and exits 1
  otherwise.",
    },
    Command {
        name: "playout-bench",
        group: "Verify",
        summary: "Time random playouts on one core and print their checksum",
        usage: "dominofish playout-bench --mode 1v1|2v2 --rounds N --seed N",
        options: "  --rounds N          Rounds to play out
  --seed N            Seed, at most 2^32 - 1",
    },
];

pub fn find(name: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.name == name)
}

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
        out.push_str(&format!("  {:<15}{}\n", c.name, c.summary));
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
