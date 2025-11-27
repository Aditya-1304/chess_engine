use crate::board::Board;
use crate::book::OpeningBook;
use crate::moves::{format, Move, MoveList};
use crate::syzygy::auto_load;
use crate::thread::ThreadPool;
use crate::types::Color;
use std::io::{self, BufRead};

pub fn main_loop() {
    let stdin = io::stdin();
    let mut board = Board::default();

    // Default to number of CPUs, capped at reasonable limit
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get().min(16))
        .unwrap_or(1);

    let mut thread_pool = ThreadPool::new(num_threads, 128); // 128MB TT
    let mut book = OpeningBook::new("Perfect2023.bin");

    if book.file.is_some() {
        println!("info string Opening book loaded successfully");
    } else {
        println!("info string Warning: book.bin not found");
    }

    auto_load();

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let cmd = line.trim();

        if cmd == "uci" {
            println!("id name AdityaChess");
            println!("id author Aditya");
            println!(
                "option name Threads type spin default {} min 1 max 256",
                num_threads
            );
            println!("option name Hash type spin default 128 min 1 max 16384");
            println!("option name SyzygyPath type string default <empty>");
            println!("uciok");
        } else if cmd == "isready" {
            println!("readyok");
        } else if cmd.starts_with("setoption") {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if parts.len() >= 5 && parts[1] == "name" && parts[3] == "value" {
                let name = parts[2].to_lowercase();
                let value = parts[4..].join(" ");

                match name.as_str() {
                    "threads" => {
                        if let Ok(n) = value.parse::<usize>() {
                            let n = n.max(1).min(256);
                            let hash_mb = 128; // Keep current hash size
                            thread_pool = ThreadPool::new(n, hash_mb);
                            println!("info string Threads set to {}", n);
                        }
                    }
                    "hash" => {
                        if let Ok(mb) = value.parse::<usize>() {
                            let mb = mb.max(1).min(16384);
                            let threads = thread_pool.num_threads;
                            thread_pool = ThreadPool::new(threads, mb);
                            println!("info string Hash set to {} MB", mb);
                        }
                    }
                    "syzygypath" => {
                        crate::syzygy::init_global_syzygy(&value);
                    }
                    _ => {}
                }
            }
        } else if cmd == "ucinewgame" {
            thread_pool.clear();
        } else if cmd.starts_with("position") {
            parse_position(cmd, &mut board);
        } else if cmd.starts_with("go") {
            parse_go(cmd, &thread_pool, &mut board, &mut book);
        } else if cmd == "stop" {
            thread_pool.stop();
        } else if cmd == "quit" {
            thread_pool.stop();
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

fn parse_go(cmd: &str, thread_pool: &ThreadPool, board: &mut Board, book: &mut OpeningBook) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let mut depth = 64u8;
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
            "infinite" => {
                depth = 64;
            }
            _ => {}
        }
        i += 1;
    }

    // Check book first
    if let Some(book_move) = book.get_move(board.zobrist_hash) {
        let mut move_list = MoveList::new();
        board.generate_pseudo_legal_moves(&mut move_list);
        for &m in move_list.iter() {
            if crate::moves::from_sq(m) == crate::moves::from_sq(book_move)
                && crate::moves::to_sq(m) == crate::moves::to_sq(book_move)
            {
                println!("bestmove {}", format(m));
                return;
            }
        }
    }

    let safety_margin = 200_u64;
    let time_limit: u64;
    let hard_limit: u64;

    if movetime > 0 {
        let spendable = movetime.saturating_sub(safety_margin);
        time_limit = spendable.max(5).min(movetime.saturating_sub(1).max(1));
        hard_limit = movetime.saturating_sub(5).max(time_limit + 10).min(movetime);
    } else if wtime > 0 || btime > 0 {
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
                time_limit = inc_budget.min(inc);
                hard_limit = (inc_budget + safety_margin).max(time_limit + 50).min(inc);
            }
        } else {
            let mtg = movestogo.unwrap_or(40).max(1) as u64;
            let base = usable / mtg;
            let inc_bonus = inc.saturating_mul(3) / 4;
            let mut tl = base.saturating_add(inc_bonus).max(50);

            if movestogo.is_none() {
                let greedy = usable / 5 + inc / 2;
                tl = tl.min(greedy);
            }

            time_limit = tl.min(usable);
            hard_limit = (tl * 3 / 2 + safety_margin)
                .min(time_left.saturating_sub(safety_margin / 2).max(tl + 50));
        }
    } else {
        // Infinite search or depth-only
        time_limit = u64::MAX;
        hard_limit = u64::MAX;
    }

    let (_score, best_move) = thread_pool.search(
        board,
        depth,
        time_limit as u128,
        hard_limit as u128,
    );

    if let Some(m) = best_move {
        println!("bestmove {}", format(m));
    } else {
        println!("bestmove 0000");
    }
}