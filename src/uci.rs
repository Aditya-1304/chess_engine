use std::io::{self, BufRead};
use crate::board::Board;
use crate::search::Searcher;
use crate::moves::{Move, MoveList, format};
use crate::types::{Color};


pub fn main_loop() {
    let stdin = io::stdin();
    let mut board = Board::default();
    let mut searcher = Searcher::new();

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let cmd = line.trim();
        
        if cmd == "uci" {
            println!("id name AdityaChess");
            println!("id author Aditya");
            println!("uciok");
        } else if cmd == "isready" {
            println!("readyok");
        } else if cmd == "ucinewgame" {
            searcher.tt.clear();
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
            *board = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
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
    let mut depth = 64; // Default depth
    let mut wtime = 0;
    let mut btime = 0;
    let mut movetime = 0;
    let mut movestogo = 30; // Assumption
    let mut i = 1;
    while i < parts.len() {
        match parts[i] {
            "depth" => {
                if i + 1 < parts.len() {
                    depth = parts[i+1].parse().unwrap_or(64);
                    i += 1;
                }
            }
            "wtime" => {
                if i + 1 < parts.len() {
                    wtime = parts[i+1].parse().unwrap_or(0);
                    i += 1;
                }
            }
            "btime" => {
                if i + 1 < parts.len() {
                    btime = parts[i+1].parse().unwrap_or(0);
                    i += 1;
                }
            }
            "movetime" => {
                if i + 1 < parts.len() {
                    movetime = parts[i+1].parse().unwrap_or(0);
                    i += 1;
                }
            }
            "movestogo" => {
                if i + 1 < parts.len() {
                    movestogo = parts[i+1].parse().unwrap_or(25);
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let mut time_limit = 0;
    if movetime > 0 {
        time_limit = movetime;
    } else {
        let time_left = if board.side_to_move == Color::White { wtime } else { btime };
        if time_left > 0 {
            time_limit = time_left / movestogo;
        }
    }
    
    searcher.time_limit_ms = time_limit as u128;
    
    if time_limit == 0 {
         searcher.time_limit_ms = 5000;
    }
    let (_score, best_move) = searcher.search(board, depth);
    
    if let Some(m) = best_move {
        println!("bestmove {}", format(m));
    } else {
        println!("bestmove 0000");
    }
}