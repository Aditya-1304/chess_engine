use std::io::{self, BufRead};
use crate::board::Board;
use crate::search::Searcher;
use crate::moves::{self, Move, MoveList, format};
use crate::types::{Color, PieceType};
pub fn main_loop() {
    let stdin = io::stdin();
    let mut board = Board::default();
    let mut searcher = Searcher::new();
    // Set a default hash size or handle 'setoption' later
    // searcher.tt = TranspositionTable::new(64); 
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
        // Check legality (simplified, just check if king is attacked after)
        // In a real engine we might want a is_legal() helper, but we can rely on make/unmake check
        // or just assume the GUI sends legal moves.
        // However, we need to match the string.
        
        if format(m) == move_str {
            return m;
        }
    }
    0
}
// fn format_move(m: Move) -> String {
//     let from = moves::from_sq(m);
//     let to = moves::to_sq(m);
    
//     let f_file = (from % 8) as u8;
//     let f_rank = (from / 8) as u8;
//     let t_file = (to % 8) as u8;
//     let t_rank = (to / 8) as u8;
//     let mut s = format!(
//         "{}{}{}{}",
//         (b'a' + f_file) as char,
//         (b'1' + f_rank) as char,
//         (b'a' + t_file) as char,
//         (b'1' + t_rank) as char
//     );
//     if moves::is_promotion(m) {
//         let ch = match moves::promotion_piece(m) {
//             PieceType::Knight => 'n',
//             PieceType::Bishop => 'b',
//             PieceType::Rook => 'r',
//             PieceType::Queen => 'q',
//             _ => 'q',
//         };
//         s.push(ch);
//     }
//     s
// }
fn parse_go(cmd: &str, searcher: &mut Searcher, board: &mut Board) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let mut depth = 6; // Default depth
    let mut wtime = 0;
    let mut btime = 0;
    let mut movetime = 0;
    let mut movestogo = 30; // Assumption
    let mut i = 1;
    while i < parts.len() {
        match parts[i] {
            "depth" => {
                if i + 1 < parts.len() {
                    depth = parts[i+1].parse().unwrap_or(6);
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
                    movestogo = parts[i+1].parse().unwrap_or(30);
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