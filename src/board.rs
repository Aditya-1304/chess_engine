use crate::types::{Bitboard, Color, PieceType, Square};

pub type ZHash = u64;

#[derive(Clone)]
pub struct UndoInfo {

}

#[derive(Clone)]
pub struct Board {
  pub pieces: [[Bitboard; 2]; 6], // pieces [PieceType][Color]
  pub occupancy: [Bitboard; 3], // occupancy: [0] = white, [1] = black, [2] = all
  pub side_to_move: Color,
  pub castling_rights: u8, // 4 bits: WhiteKingsSide, WhiteQueenSide, BlackKingSide, BlackQueenSide
  pub en_passant: Option<Square>,
  pub halfmove_clock: u8,
  pub fullmove_number: u32,
  pub zobrist_hash: ZHash,
  pub history: Vec<UndoInfo>, // stack for undoing the moves
}

impl Board {

  pub fn from_fen(fen: &str) -> Result<Board, &'static str> {
    Ok(Board::default())
  }

  pub fn to_fen(&self) -> String {
    String::new()
  }

  pub fn make_move(&mut self, m:crate::types::Move) -> UndoInfo {

    UndoInfo {}
  }

  pub fn unmake_move(&mut self, undo: UndoInfo) {
    
  }
}


impl Default for Board {
  fn default() -> Self {
    Board { 
      pieces: [[0;2]; 6], 
      occupancy: [0; 3], 
      side_to_move: Color::White, 
      castling_rights: 0, 
      en_passant: None, 
      halfmove_clock: 0, 
      fullmove_number: 1, 
      zobrist_hash: 0, 
      history: Vec::new(),
    }
  }
}