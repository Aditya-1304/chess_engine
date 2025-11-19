use chess_engine::{board::Board, movegen};
use std::env;
use std::time::Instant;

const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn main() {
    movegen::init();
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_help();
        return;
    }

    match args[1].as_str() {
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
    let nps = if seconds > 0.0 { (nodes as f64 / seconds) as u64 } else { 0 };
    
    println!("-----------------------------");
    println!("Nodes: {}", nodes);
    println!("Time:  {:.3} s", seconds);
    println!("NPS:   {}", nps);
    println!("-----------------------------");
}

fn print_help() {
    println!("Chess_engine v0.1");
    println!("Commands:");
    println!("  --fen \"<FEN>\"        : Print board from FEN");
    println!("  perft <depth>        : Run perft on starting position");
    println!("  perft <depth> \"<FEN>\": Run perft on specific position");
}