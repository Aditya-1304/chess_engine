use chess_engine::board::{ Board};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
     
    if args.len() > 2 && args[1] == "--fen" {
        let fen = &args[2];
        match Board::from_fen(fen) {
            Ok(board) => {
                println!("{}", board);
            }
            Err(e) => eprintln!("Error parsing FEN: {}", e),
        }
    } else {
        println!("ChessEngine v1.0");
        println!("Usage: cargo run -- --fen \"<FEN_STRING>\"");
        println!("\nExample (starting position):");
        println!("cargo run -- --fen \"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1\"");
    }
}
