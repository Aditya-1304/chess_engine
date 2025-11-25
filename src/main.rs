use chess_engine::{
    board::Board,
    movegen,
    moves::{self, Move},
    nnue,
    search::Searcher,
    types::PieceType,
    uci,
};
use std::env;
use std::time::Instant;

const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn main() {
    movegen::init();

    println!("Loading NNUE...");
    match nnue::Network::load("nn-62ef826d1a6d.nnue") {
        Ok(net) => {
            nnue::NETWORK.set(net).ok();
            println!("NNUE loaded successfully!");
        }
        Err(e) => {
            println!("Warning: Could not load NNUE: {}", e);
            println!("Falling back to classical evaluation.");
        }
    }

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        uci::main_loop();
        return;
    }
    match args[1].as_str() {
        "uci" => {
            uci::main_loop();
        }
        "--fen" => {
            if args.len() > 2 {
                let fen = &args[2];
                match Board::from_fen(fen) {
                    Ok(board) => println!("{}", board),
                    Err(e) => eprintln!("Error: {}", e),
                }
            } else {
                eprintln!("Error: --fen requires a FEN string");
            }
        }
        "perft" => {
            if args.len() > 2 {
                let depth = args[2].parse::<u8>().unwrap_or(1);
                let mut board = if args.len() > 3 {
                    Board::from_fen(&args[3]).expect("Invalid FEN")
                } else {
                    Board::from_fen(START_FEN).expect("Invalid Start FEN")
                };

                run_perft(&mut board, depth);
            } else {
                eprintln!("Usage: cargo run -- perft <depth> [optional_fen]");
            }
        }
        "search" => {
            // Usage: cargo run -- search <depth> [fen]
            if args.len() > 2 {
                let depth = args[2].parse::<u8>().unwrap_or(4);
                let mut board = if args.len() > 3 {
                    Board::from_fen(&args[3]).expect("Invalid FEN")
                } else {
                    Board::from_fen(START_FEN).expect("Invalid Start FEN")
                };

                run_search(&mut board, depth);
            } else {
                eprintln!("Usage: cargo run -- search <depth> [optional_fen]");
            }
        }
        _ => print_help(),
    }
}

fn run_perft(board: &mut Board, depth: u8) {
    println!("Running perft depth {}...", depth);
    println!("{}", board);

    let start = Instant::now();
    let nodes = board.perft(depth);
    let duration = start.elapsed();

    let seconds = duration.as_secs_f64();
    let nps = if seconds > 0.0 {
        (nodes as f64 / seconds) as u64
    } else {
        0
    };

    println!("-----------------------------");
    println!("Nodes: {}", nodes);
    println!("Time:  {:.3} s", seconds);
    println!("NPS:   {}", nps);
    println!("-----------------------------");
}

fn run_search(board: &mut Board, depth: u8) {
    println!("Searching depth {}...", depth);
    println!("{}", board);

    let mut searcher = Searcher::new();
    let start = Instant::now();

    let (score, best_move) = searcher.search(board, depth);

    let duration = start.elapsed();
    let seconds = duration.as_secs_f64();
    let nps = if seconds > 0.0 {
        (searcher.nodes as f64 / seconds) as u64
    } else {
        0
    };

    println!("-----------------------------");
    if let Some(m) = best_move {
        println!("Best Move: {}", format_move(m));
    } else {
        println!("Best Move: None (Stalemate/Checkmate)");
    }

    // Pretty print score
    if score > 30000 {
        let moves_to_mate = (32000 - score + 1) / 2;
        println!("Score:     Mate in {}", moves_to_mate);
    } else if score < -30000 {
        let moves_to_mate = (32000 + score) / 2;
        println!("Score:     Mate in -{}", moves_to_mate);
    } else {
        println!("Score:     {:.2}", score as f32 / 100.0);
    }

    println!("Nodes:     {}", searcher.nodes);
    println!("Time:      {:.3} s", seconds);
    println!("NPS:       {}", nps);
    println!("-----------------------------");
}

fn format_move(m: Move) -> String {
    let from = moves::from_sq(m);
    let to = moves::to_sq(m);

    let f_file = (from % 8) as u8;
    let f_rank = (from / 8) as u8;
    let t_file = (to % 8) as u8;
    let t_rank = (to / 8) as u8;

    let mut s = format!(
        "{}{}{}{}",
        (b'a' + f_file) as char,
        (b'1' + f_rank) as char,
        (b'a' + t_file) as char,
        (b'1' + t_rank) as char
    );

    if moves::is_promotion(m) {
        let ch = match moves::promotion_piece(m) {
            PieceType::Knight => 'n',
            PieceType::Bishop => 'b',
            PieceType::Rook => 'r',
            PieceType::Queen => 'q',
            _ => 'q',
        };
        s.push(ch);
    }
    s
}

fn print_help() {
    println!("AdityaChess v1.0");
    println!("Commands:");
    println!("  --fen \"<FEN>\"          : Print board from FEN");
    println!("  perft <depth>          : Run perft on starting position");
    println!("  search <depth>         : Run alpha-beta search");
    println!("  search <depth> \"<FEN>\" : Run search on specific position");
}

