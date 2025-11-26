use crate::board::Board;
use crate::moves::{Move, MoveList, format};
use crate::search::Searcher;
use crate::syzygy::auto_load;
use crate::types::Color;
use std::io::{self, BufRead};

pub fn main_loop() {
    let stdin = io::stdin();
    let mut board = Board::default();
    let mut searcher = Searcher::new();
    auto_load();

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let cmd = line.trim();

        if cmd == "uci" {
            println!("id name AdityaChess");
            println!("id author Aditya");
            println!("option name SyzygyPath type string default <empty>");
            println!("uciok");
        } else if cmd == "isready" {
            println!("readyok");
        } else if cmd.starts_with("setoption") {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if parts.len() >= 5
                && parts[1] == "name"
                && parts[2] == "SyzygyPath"
                && parts[3] == "value"
            {
                let path = parts[4..].join(" ");
                crate::syzygy::init_global_syzygy(&path);
            }
        } else if cmd == "ucinewgame" {
            searcher.tt.clear();
            searcher.history = [[[0; 64];2]; 6];
            searcher.killers = [[None; 2]; 64];
            searcher.counter_moves = [[None; 64]; 6];
            searcher.prev_move = None;
        } else if cmd.starts_with("position") {
            parse_position(cmd, &mut board);
        } else if cmd.starts_with("go") {
            parse_go(cmd, &mut searcher, &mut board);
        } else if cmd == "stop" {
            searcher.stop = true;
        } else if cmd == "quit" {
            break;
        }
    }
}

fn parse_position(cmd: &str, board: &mut Board) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let mut moves_idx = 0;

    if parts.len() > 1 {
        if parts[1] == "startpos" {
            *board = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
                .unwrap();
            moves_idx = 2;
        } else if parts[1] == "fen" {
            // join parts until "moves"
            let mut fen = String::new();
            let mut i = 2;
            while i < parts.len() && parts[i] != "moves" {
                fen.push_str(parts[i]);
                fen.push(' ');
                i += 1;
            }
            if let Ok(b) = Board::from_fen(fen.trim()) {
                *board = b;
            }
            moves_idx = i;
        }
    }

    if moves_idx < parts.len() && parts[moves_idx] == "moves" {
        for i in (moves_idx + 1)..parts.len() {
            let move_str = parts[i];
            let m = parse_move(board, move_str);
            if m != 0 {
                board.make_move(m);
            }
        }
    }
}

fn parse_move(board: &Board, move_str: &str) -> Move {
    let mut move_list = MoveList::new();
    board.generate_pseudo_legal_moves(&mut move_list);
    for &m in move_list.iter() {
        if format(m) == move_str {
            return m;
        }
    }
    0
}

fn parse_go(cmd: &str, searcher: &mut Searcher, board: &mut Board) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let mut depth = 64;
    let mut wtime: u64 = 0;
    let mut btime: u64 = 0;
    let mut winc: u64 = 0;
    let mut binc: u64 = 0;
    let mut movetime: u64 = 0;
    let mut movestogo = None;
    let mut i = 1;
    while i < parts.len() {
        match parts[i] {
            "depth" => {
                if i + 1 < parts.len() {
                    depth = parts[i + 1].parse().unwrap_or(64);
                    i += 1;
                }
            }
            "wtime" => {
                if i + 1 < parts.len() {
                    wtime = parts[i + 1].parse().unwrap_or(0);
                    i += 1;
                }
            }
            "btime" => {
                if i + 1 < parts.len() {
                    btime = parts[i + 1].parse().unwrap_or(0);
                    i += 1;
                }
            }
            "winc" => {
                if i + 1 < parts.len() {
                    winc = parts[i + 1].parse().unwrap_or(0);
                    i += 1;
                }
            }
            "binc" => {
                if i + 1 < parts.len() {
                    binc = parts[i + 1].parse().unwrap_or(0);
                    i += 1;
                }
            }
            "movetime" => {
                if i + 1 < parts.len() {
                    movetime = parts[i + 1].parse().unwrap_or(0);
                    i += 1;
                }
            }
            "movestogo" => {
                if i + 1 < parts.len() {
                    movestogo = Some(parts[i + 1].parse().unwrap_or(25));
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let safety_margin = 200_u64;
    let mut time_limit: u64;
    let mut hard_limit: u64;

    if movetime > 0 {
        let spendable = movetime.saturating_sub(safety_margin);
        time_limit = spendable.max(5);
        time_limit = time_limit.min(movetime.saturating_sub(1).max(1));
        hard_limit = movetime.saturating_sub(5).max(time_limit + 10);
        hard_limit = hard_limit.min(movetime);
    } else {
        let (time_left, inc) = if board.side_to_move == Color::White {
            (wtime, winc)
        } else {
            (btime, binc)
        };
        let usable = time_left.saturating_sub(safety_margin);

        if usable == 0 {
            if inc == 0 {
                time_limit = 500;
                hard_limit = 800;
            } else {
                let inc_budget = inc.saturating_sub(safety_margin / 2).max(50);
                time_limit = inc_budget;
                hard_limit = (inc_budget + safety_margin).max(time_limit + 50);
                hard_limit = hard_limit.min(inc);
                time_limit = time_limit.min(hard_limit);
            }
        } else {
            let mtg = movestogo.unwrap_or(40).max(1) as u64;
            let base = usable / mtg;
            let inc_bonus = inc.saturating_mul(3) / 4;
            time_limit = base.saturating_add(inc_bonus).max(50);

            if movestogo.is_none() {
                let greedy = usable / 5 + inc / 2;
                time_limit = time_limit.min(greedy);
            }

            time_limit = time_limit.min(usable);
            hard_limit = (time_limit * 3 / 2 + safety_margin).min(
                time_left
                    .saturating_sub(safety_margin / 2)
                    .max(time_limit + 50),
            );
        }
    }

    searcher.time_soft_limit = time_limit as u128;
    searcher.time_hard_limit = hard_limit as u128;

    let (_score, best_move) = searcher.search(board, depth);

    if let Some(m) = best_move {
        println!("bestmove {}", format(m));
    } else {
        println!("bestmove 0000");
    }
}
