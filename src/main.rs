use chess_engine::board::{ Board};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
     
    if args.len() > 2 && args[1] == "--fen" {
        let fen = &args[2];
        match Board::from_fen(fen) {
            Ok(board) => {
                println!("Board created successfully from FEN.");
                println!("Side to move {:?}", board.side_to_move);
                println!("Castling rights: {}", board.castling_rights);
            }
            Err(e) => eprintln!("Error parsing FEN: {}", e),
        }
    } else {
        println!("ChessEngine v1.0");
    }
}
