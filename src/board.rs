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
    let mut board = Board::default();
    let parts: Vec<&str> = fen.split_whitespace().collect();
    if parts.len() != 6 {
      return Err("Invalid FEN: must have 6 fields");
    }

    // setting the correct piece placement on the board
    let piece_placement = parts[0];
    let mut rank = 7;
    let mut file = 0;
    for ch in piece_placement.chars() {
      if ch == '/' {
        rank -= 1;
        file = 0;
      } else if let Some(digit) = ch.to_digit(10) {
          file += digit as u8;
      } else {
        if rank < 0 || file > 7 {
          return Err("Invalid FEN: piece placement format error");
        }
        let square = rank * 8 + file;
        let color = if ch.is_uppercase() {
          Color::White
        } else {
          Color::Black
        };
        let piece_type = match ch.to_ascii_lowercase() {
          'p' => PieceType::Pawn,
          'n' => PieceType::Knight,
          'b' => PieceType::Bishop,
          'r' => PieceType::Rook,
          'q' => PieceType::Queen,
          'k' => PieceType::King,
          _ => return Err("Invalid FEN: Unknown piece character"),
        };
        board.pieces[piece_type as usize][color as usize] |= 1 << square;
        file += 1;

      }
    }

    // occupancy calculation (basic for calculation for now)
    for pt_idx in 0..6 {
      board.occupancy[Color::White as usize] |= board.pieces[pt_idx][Color::White as usize];
      board.occupancy[Color::Black as usize] |= board.pieces[pt_idx][Color::Black as usize];
    }
    board.occupancy[2] = 
      board.occupancy[Color::White as usize] | board.occupancy[Color::Black as usize];
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