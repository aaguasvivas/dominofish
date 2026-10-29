//! The web board's engine interface (web/, docs/FORMATS.md, Board requests).
//! The page keeps the game record and sends one text request per call: a
//! command line, then the record text. The reply is "ok" and a body (a
//! record or JSON), or "error <reason>". Nothing here keeps state between
//! calls.

use std::fmt::Write as _;

use dominofish_core::bots::Bot;
use dominofish_core::infoset::InfoSet;
use dominofish_core::pimc::{self, Limits};
use dominofish_core::record::{GameRecord, RoundRecord, move_text, parse_move};
use dominofish_core::rng::Rng;
use dominofish_core::rules::{Deal, End, Event, Mode, Move, MoveList, Phase, State, team};
use dominofish_core::solver::{self, Table};
use dominofish_core::tile::tiles;

/// The node cap of each solve in an analysis request; a capped value is
/// marked in the reply.
const ANALYSIS_NODES: u64 = 2_000_000;
/// Moves of the best line in an analysis reply.
const LINE_MOVES: usize = 40;
/// DominoFish's own engine on the board (PIMC, docs/PLAN.md D18): 32
/// worlds per move, half the engine's default, so a phone answers in about
/// a second (R5 measured no gain from 32 to 64 worlds in 2v2), the analysis
/// node cap, and a 1 MB table. It sees only the InfoSet of the seat it
/// plays or advises.
const ENGINE: Limits = Limits {
    worlds: 32,
    nodes: ANALYSIS_NODES,
    table_bits: 16,
};

/// Handles one request; see the module comment.
pub fn handle(request: &str) -> String {
    let (command, body) = request.split_once('\n').unwrap_or((request, ""));
    let words: Vec<&str> = command.split_whitespace().collect();
    let reply = match words.as_slice() {
        ["new", mode, target, seed] => new_game(mode, target, seed),
        [cmd, rest @ ..] => match GameRecord::from_text(body) {
            Ok(record) => match (*cmd, rest) {
                ("view", []) => view(&record),
                ("play", mv) => play(record, mv),
                ("undo", []) => undo(record),
                ("next", [seed]) => next_round(record, seed),
                ("bot", [name, seed]) => bot_move(&record, name, seed),
                ("engine", [seed]) => engine(&record, seed),
                ("analyze", []) => analyze(&record),
                _ => Err(format!("unknown request {command}")),
            },
            Err(e) => Err(format!("bad record: {e:?}")),
        },
        [] => Err("empty request".into()),
    };
    match reply {
        Ok(body) => format!("ok\n{body}"),
        Err(e) => format!("error {e}"),
    }
}

fn seed_of(s: &str) -> Result<u64, String> {
    s.parse().map_err(|_| format!("bad seed {s}"))
}

fn new_game(mode: &str, target: &str, seed: &str) -> Result<String, String> {
    let mode = match mode {
        "1v1" => Mode::OneVsOne,
        "2v2" => Mode::TwoVsTwo,
        _ => return Err(format!("bad mode {mode}")),
    };
    let target: u16 = match target {
        "100" => 100,
        "200" => 200,
        _ => return Err(format!("bad target {target}")),
    };
    let deal = Deal::shuffled(&mut Rng::new(seed_of(seed)?));
    let record = GameRecord {
        mode,
        target,
        rounds: vec![RoundRecord {
            deal,
            moves: vec![],
        }],
    };
    Ok(record.to_text())
}

/// The current round's start state and the state after its last move.
fn current(record: &GameRecord) -> (State, State) {
    let r = record.rounds.len() - 1;
    let (start, _) = record.round_start(r).expect("a parsed record replays");
    let mut state = start;
    for &mv in &record.rounds[r].moves {
        state.play(mv);
    }
    (start, state)
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn tile_list(mask: u32) -> String {
    let items: Vec<String> = tiles(mask).map(|t| json_str(&t.to_string())).collect();
    format!("[{}]", items.join(","))
}

fn event_json(e: &Event) -> String {
    match *e {
        Event::PaseSalida { team } => {
            format!("{{\"kind\":\"salida\",\"team\":{team},\"points\":25}}")
        }
        Event::PaseCorrido { team } => {
            format!("{{\"kind\":\"corrido\",\"team\":{team},\"points\":25}}")
        }
        Event::Domino { seat, points } => format!(
            "{{\"kind\":\"domino\",\"seat\":{seat},\"team\":{},\"points\":{points}}}",
            team(seat)
        ),
        Event::Capicua { seat, points } => format!(
            "{{\"kind\":\"capicua\",\"seat\":{seat},\"team\":{},\"points\":{points}}}",
            team(seat)
        ),
        Event::Tranque {
            blocker,
            rival,
            blocker_pips,
            rival_pips,
            winner,
            points,
        } => format!(
            "{{\"kind\":\"tranque\",\"seat\":{winner},\"team\":{},\"points\":{points},\"blocker\":{blocker},\"rival\":{rival},\"blocker_pips\":{blocker_pips},\"rival_pips\":{rival_pips}}}",
            team(winner)
        ),
    }
}

fn move_json(state: &State, mv: Move) -> String {
    let mut out = format!("{{\"move\":{}", json_str(&move_text(mv)));
    if let Move::Play { tile, end } = mv {
        let end = if state.board == 0 {
            "lead"
        } else if end == End::Left {
            "left"
        } else {
            "right"
        };
        let _ = write!(
            out,
            ",\"tile\":{},\"end\":\"{end}\"",
            json_str(&tile.to_string())
        );
    }
    out.push('}');
    out
}

/// The position after the record's last move: everything the page draws.
/// Hidden hands are included; the page shows them only when asked to.
fn view(record: &GameRecord) -> Result<String, String> {
    let r = record.rounds.len() - 1;
    let (start, _) = current(record);
    let mut state = start;
    // The line of play, each tile oriented as it lies, left to right.
    let mut line: Vec<(u8, u8)> = Vec::new();
    if state.board != 0 {
        line.push((state.left, state.right));
    }
    let mut history = Vec::new();
    let mut result = String::from("null");
    for &mv in &record.rounds[r].moves {
        let seat = state.turn;
        let was_empty = state.board == 0;
        let pile_before: Vec<String> = state.boneyard().iter().map(|t| t.to_string()).collect();
        let played = state.play(mv);
        let mut item = format!("{{\"seat\":{seat},\"move\":{}", json_str(&move_text(mv)));
        match mv {
            Move::Play { tile, end } => {
                if was_empty {
                    line.push((state.left, state.right));
                } else if end == End::Left {
                    line.insert(0, (state.left, tile.other(state.left)));
                } else {
                    line.push((tile.other(state.right), state.right));
                }
            }
            Move::Draw => {
                let drawn: Vec<String> = pile_before[state.boneyard().len()..]
                    .iter()
                    .rev()
                    .map(|t| json_str(t))
                    .collect();
                let _ = write!(item, ",\"drawn\":[{}]", drawn.join(","));
            }
            Move::Pass => {}
        }
        if let Some(e) = played.event {
            let _ = write!(item, ",\"event\":{}", event_json(&e));
            if state.phase != Phase::Playing {
                result = event_json(&e);
            }
        }
        item.push('}');
        history.push(item);
    }
    let mut legal = MoveList::new();
    state.legal_moves(&mut legal);
    let legal: Vec<String> = legal
        .as_slice()
        .iter()
        .map(|&mv| move_json(&state, mv))
        .collect();
    let hands: Vec<String> = (0..state.mode.seats())
        .map(|s| tile_list(state.hands[s as usize]))
        .collect();
    let opening = if r == 0 {
        let (s, t) = (start.starter, line_opening(&start));
        format!("{{\"seat\":{s},\"tile\":{}}}", json_str(&t))
    } else {
        "null".into()
    };
    let lines: Vec<String> = line.iter().map(|(a, b)| format!("[{a},{b}]")).collect();
    Ok(format!(
        "{{\"mode\":\"{}\",\"target\":{},\"round\":{},\"scores\":[{},{}],\"phase\":\"{}\",\"turn\":{},\"starter\":{},\"seats\":{},\"hands\":[{}],\"line\":[{}],\"ends\":{},\"boneyard\":{},\"legal\":[{}],\"history\":[{}],\"result\":{result},\"opening\":{opening}}}",
        state.mode.name(),
        state.target,
        r + 1,
        state.scores[0],
        state.scores[1],
        match state.phase {
            Phase::Playing => "playing",
            Phase::RoundOver => "round_over",
            Phase::Finished => "finished",
        },
        state.turn,
        state.starter,
        state.mode.seats(),
        hands.join(","),
        lines.join(","),
        if state.board == 0 {
            "null".to_string()
        } else {
            format!("[{},{}]", state.left, state.right)
        },
        state.boneyard().len(),
        legal.join(","),
        history.join(","),
    ))
}

/// Round 1's automatic tile, the only tile on its start board.
fn line_opening(start: &State) -> String {
    tiles(start.board)
        .next()
        .map_or(String::new(), |t| t.to_string())
}

fn play(mut record: GameRecord, words: &[&str]) -> Result<String, String> {
    let mv = parse_move(words).ok_or_else(|| format!("bad move {}", words.join(" ")))?;
    let (_, state) = current(&record);
    if !state.is_legal(mv) {
        return Err(format!("illegal move {}", words.join(" ")));
    }
    let mv = state.canonical(mv);
    record.rounds.last_mut().expect("a round").moves.push(mv);
    Ok(record.to_text())
}

fn undo(mut record: GameRecord) -> Result<String, String> {
    if record
        .rounds
        .last_mut()
        .expect("a round")
        .moves
        .pop()
        .is_none()
    {
        return Err("no move to take back in this round".into());
    }
    Ok(record.to_text())
}

fn next_round(mut record: GameRecord, seed: &str) -> Result<String, String> {
    let (_, state) = current(&record);
    if state.phase != Phase::RoundOver {
        return Err("the round is not over".into());
    }
    record.rounds.push(RoundRecord {
        deal: Deal::shuffled(&mut Rng::new(seed_of(seed)?)),
        moves: vec![],
    });
    Ok(record.to_text())
}

/// The InfoSet of the seat on turn: all a bot or the engine may see.
fn seat_on_turn(record: &GameRecord) -> Result<InfoSet, String> {
    let (_, state) = current(record);
    if state.phase != Phase::Playing {
        return Err("the round is over".into());
    }
    let r = record.rounds.len() - 1;
    InfoSet::new(record, r, record.rounds[r].moves.len(), state.turn).map_err(|e| format!("{e:?}"))
}

/// The move a bot makes for the seat on turn, from that seat's InfoSet:
/// a baseline bot, or DominoFish ("pimc"), which plays a forced move at
/// once and otherwise the best move of its analysis.
fn bot_move(record: &GameRecord, name: &str, seed: &str) -> Result<String, String> {
    let info = seat_on_turn(record)?;
    let seed = seed_of(seed)?;
    if name == "pimc" {
        let mut legal = MoveList::new();
        info.legal_moves(&mut legal);
        let mv = if legal.len() == 1 {
            legal.as_slice()[0]
        } else {
            pimc::analyze(&info, &ENGINE, seed).best
        };
        return Ok(move_text(mv));
    }
    let bot = Bot::from_name(name).ok_or_else(|| format!("unknown bot {name}"))?;
    Ok(move_text(bot.decide(&info, &mut Rng::new(seed))))
}

/// DominoFish's analysis for the seat on turn, from its InfoSet alone:
/// every legal move with its mean round points for that seat's team over
/// the sampled worlds and the standard error, best first, and the number
/// of worlds and of consistent worlds.
fn engine(record: &GameRecord, seed: &str) -> Result<String, String> {
    let info = seat_on_turn(record)?;
    let (_, state) = current(record);
    let a = pimc::analyze(&info, &ENGINE, seed_of(seed)?);
    if a.fallback > 0 || a.inconsistent > 0 {
        return Err("the sampler failed".into());
    }
    let moves: Vec<String> = a
        .moves
        .iter()
        .map(|m| {
            let item = move_json(&state, m.mv);
            let se = m
                .standard_error(a.worlds)
                .map_or("null".to_string(), |x| format!("{x:.2}"));
            format!(
                "{{{},\"mean\":{:.2},\"se\":{se}}}",
                &item[1..item.len() - 1],
                m.mean(a.worlds)
            )
        })
        .collect();
    Ok(format!(
        "{{\"seat\":{},\"worlds\":{},\"consistent\":{},\"nodes\":{},\"capped\":{},\"moves\":[{}]}}",
        info.seat,
        a.worlds,
        a.info_size,
        a.nodes,
        a.capped,
        moves.join(",")
    ))
}

/// Perfect-information analysis of the seat on turn: every legal move's
/// exact rest-of-round margin (team 0 minus team 1), and the best line.
fn analyze(record: &GameRecord) -> Result<String, String> {
    let (_, mut state) = current(record);
    if state.phase != Phase::Playing {
        return Err("the round is over".into());
    }
    let mut table = Table::new(18);
    let (mut nodes, mut capped) = (0u64, false);
    let mut legal = MoveList::new();
    state.legal_moves(&mut legal);
    let mut values = Vec::new();
    for &mv in legal.as_slice() {
        let item = move_json(&state, mv);
        let before = state.scores;
        let played = state.play(mv);
        let gain = (i32::from(state.scores[0]) - i32::from(before[0]))
            - (i32::from(state.scores[1]) - i32::from(before[1]));
        let v = if state.phase == Phase::Playing {
            let s = solver::solve(&mut state, Some(&mut table), ANALYSIS_NODES);
            nodes += s.nodes;
            capped |= s.capped;
            gain + s.value
        } else {
            gain
        };
        state.undo(mv, played.undo);
        values.push(format!("{{{},\"value\":{v}}}", &item[1..item.len() - 1]));
    }
    let root = solver::solve(&mut state, Some(&mut table), ANALYSIS_NODES);
    nodes += root.nodes;
    capped |= root.capped;
    let mut line = Vec::new();
    let mut s = state;
    while s.phase == Phase::Playing && line.len() < LINE_MOVES {
        let best = solver::solve(&mut s, Some(&mut table), ANALYSIS_NODES);
        nodes += best.nodes;
        capped |= best.capped;
        line.push(format!(
            "{{\"seat\":{},{}",
            s.turn,
            &move_json(&s, best.best)[1..]
        ));
        s.play(best.best);
    }
    Ok(format!(
        "{{\"value\":{},\"moves\":[{}],\"line\":[{}],\"nodes\":{nodes},\"capped\":{capped}}}",
        root.value,
        values.join(","),
        line.join(",")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(reply: &str) -> &str {
        reply
            .strip_prefix("ok\n")
            .unwrap_or_else(|| panic!("{reply}"))
    }

    /// Plays a full game through requests, every seat a rule bot, and
    /// checks the views and an analysis along the way.
    #[test]
    fn a_game_plays_through_requests_in_both_modes() {
        for (mode, seed) in [("2v2", 5u64), ("1v1", 6)] {
            let mut record = body(&handle(&format!("new {mode} 100 {seed}"))).to_string();
            let mut moves = 0;
            loop {
                let view = handle(&format!("view\n{record}"));
                let v = body(&view);
                if v.contains("\"phase\":\"finished\"") {
                    break;
                }
                if v.contains("\"phase\":\"round_over\"") {
                    assert!(v.contains("\"result\":{"), "{v}");
                    record = body(&handle(&format!("next {}\n{record}", 100 + moves))).to_string();
                    continue;
                }
                if moves % 25 == 0 {
                    let a = handle(&format!("analyze\n{record}"));
                    assert!(body(&a).contains("\"moves\":[{"), "{a}");
                }
                let mv = body(&handle(&format!("bot rule {moves}\n{record}"))).to_string();
                record = body(&handle(&format!("play {mv}\n{record}"))).to_string();
                moves += 1;
            }
            assert!(moves > 20, "{mode}: {moves}");
        }
    }

    /// The numbers after each `"key":` in a JSON text, in order.
    fn numbers(text: &str, key: &str) -> Vec<f64> {
        let pat = format!("\"{key}\":");
        text.match_indices(&pat)
            .map(|(i, _)| {
                let rest = &text[i + pat.len()..];
                let end = rest.find([',', '}']).expect("a number ends");
                rest[..end].parse().expect("a number")
            })
            .collect()
    }

    /// DominoFish on the board, at late positions of rule-bot games in both
    /// modes (cheap enough for check.sh's debug build): the engine reply is
    /// for the seat on turn, ranks every legal move best first, and uses 32
    /// worlds; the engine opponent plays a legal move.
    #[test]
    fn the_engine_ranks_every_legal_move_and_plays_legally() {
        let mut checked = 0;
        for (mode, seed) in [("2v2", 21u64), ("1v1", 22)] {
            let mut record = body(&handle(&format!("new {mode} 100 {seed}"))).to_string();
            for moves in 0..400 {
                let view = handle(&format!("view\n{record}"));
                let v = body(&view);
                if !v.contains("\"phase\":\"playing\"") {
                    break;
                }
                let legal_start = v.find("\"legal\":[").expect("legal") + 9;
                let legal = &v[legal_start..legal_start + v[legal_start..].find(']').expect("]")];
                let n_legal = legal.matches("\"move\":").count();
                // The line is an array of pip pairs: [[a,b],[c,d],...].
                let line_start = v.find("\"line\":[").expect("line") + 8;
                let line = &v[line_start..line_start + v[line_start..].find("]]").unwrap_or(0)];
                let late = line.matches('[').count() >= 14;
                if late && n_legal > 1 {
                    let turn = numbers(v, "turn")[0];
                    let e = handle(&format!("engine {moves}\n{record}"));
                    let e = body(&e);
                    assert_eq!(numbers(e, "seat"), vec![turn], "{e}");
                    assert_eq!(numbers(e, "worlds"), vec![32.0], "{e}");
                    let means = numbers(e, "mean");
                    assert_eq!(means.len(), n_legal, "{e}");
                    assert!(means.windows(2).all(|w| w[0] >= w[1]), "not ranked: {e}");
                    let mv = body(&handle(&format!("bot pimc {moves}\n{record}"))).to_string();
                    assert!(
                        legal.contains(&format!("\"move\":\"{mv}\"")),
                        "{mv} not in {legal}"
                    );
                    checked += 1;
                }
                let mv = body(&handle(&format!("bot rule {moves}\n{record}"))).to_string();
                record = body(&handle(&format!("play {mv}\n{record}"))).to_string();
            }
        }
        assert!(checked >= 4, "{checked}");
    }

    #[test]
    fn analysis_values_are_the_solver_values() {
        let record = body(&handle("new 2v2 100 9")).to_string();
        let a = handle(&format!("analyze\n{record}"));
        let parsed = GameRecord::from_text(&record).unwrap();
        let (_, mut state) = current(&parsed);
        let s = solver::solve(&mut state, None, solver::NO_CAP);
        assert!(
            body(&a).starts_with(&format!("{{\"value\":{},", s.value)),
            "{a}"
        );
    }

    #[test]
    fn bad_requests_get_an_error() {
        let record = body(&handle("new 1v1 100 3")).to_string();
        for req in [
            String::new(),
            "new 3v3 100 1".into(),
            "new 1v1 150 1".into(),
            format!("play pass\n{record}"),
            format!("play nonsense\n{record}"),
            format!("next 1\n{record}"),
            format!("bot nosuch 1\n{record}"),
            "view\nnot a record".into(),
            format!("fly\n{record}"),
        ] {
            assert!(handle(&req).starts_with("error "), "accepted: {req:?}");
        }
    }
}
