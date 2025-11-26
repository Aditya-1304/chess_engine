use std::fmt;
use crate::types::{PieceType, Square};
/* 
  Bits 0-5 from square (64 squares) 
  Bits 6-11 to square (64 squares)
  Bits 12-15: Flags (promotion, castling)
*/
pub type Move = u16;

pub const QUIET_MOVE_FLAG: u16 = 0b0000;
pub const DOUBLE_PAWN_PUSH_FLAG: u16 = 0b0001;
pub const KING_CASTLE_FLAG: u16 = 0b0010;
pub const QUEEN_CASTLE_FLAG: u16 = 0b0011;
pub const CAPTURE_FLAG: u16 = 0b0100;
pub const EN_PASSANT_CAPTURE_FLAG: u16 = 0b0101;

pub const KNIGHT_PROMOTION_FLAG: u16 = 0b1000;
pub const BISHOP_PROMOTION_FLAG: u16 = 0b1001;
pub const ROOK_PROMOTION_FLAG: u16 = 0b1010;
pub const QUEEN_PROMOTION_FLAG: u16 = 0b1011;

pub const KNIGHT_PROMOTION_CAPTURE_FLAG: u16 = 0b1100;
pub const BISHOP_PROMOTION_CAPTURE_FLAG: u16 = 0b1101;
pub const ROOK_PROMOTION_CAPTURE_FLAG: u16 = 0b1110;
pub const QUEEN_PROMOTION_CAPTURE_FLAG: u16 = 0b1111;

/// Creates a new move from its components.
pub fn new(from: Square, to: Square, flag: u16) -> Move {
  (from as u16) | ((to as u16) << 6) | (flag << 12)
}

/// Extracts the from_sqaure from a move
pub fn from_sq(m: Move) -> Square {
  (m & 0x3F) as Square
}

/// Extracts the to_square from a move
pub fn to_sq(m: Move) -> Square {
  ((m >> 6) & 0x3f) as Square
}

/// Extracts the flag from a move
pub fn flag(m: Move) -> u16 {
  (m >> 12) & 0xF
}

/// Checks if a move is a capture
pub fn is_capture(m: Move) -> bool {
  flag(m) & 0b0100 != 0
}

/// Checks if a move is a promotion
pub fn is_promotion(m: Move) -> bool {
  flag(m) & 0b1000 != 0
}

/// Gets the promotion piece type from a promotion move
pub fn promotion_piece(m: Move) -> PieceType {
  match flag(m) & 0b0011 {
    0 => PieceType::Knight,
    1 => PieceType::Bishop,
    2 => PieceType::Rook,
    _ => PieceType::Queen,
  }
}

pub struct MoveList {
  moves: [Move; 256],
  count: usize,
}

impl MoveList {
  pub fn new() -> Self {
    MoveList { moves: [0; 256], count: 0 }
  }

  pub fn push(&mut self, m: Move) {
    self.moves[self.count] = m;
    self.count += 1; 
  }

  pub fn len(&self) -> usize {
    self.count
  }

  pub fn iter(&self) -> std::slice::Iter<'_,Move> {
    self.moves[..self.count].iter()
  }

  pub fn as_mut_slice(&mut self) -> &mut [Move] {
    &mut self.moves[..self.count]
  }

  #[inline]
  pub fn get(&self, index: usize) -> Move {
    self.moves[index]
  }
  
  #[inline]
  pub fn set(&mut self, index: usize, m: Move) {
    self.moves[index] = m;
  }

}

pub fn format_square(sq: Square) -> String {
  let file = sq % 8;
  let rank = sq / 8;
  format!("{}{}", (b'a' + file) as char, (b'1' + rank) as char)
}

pub fn format(m: Move) -> String {
  let from = from_sq(m);
  let to = to_sq(m);
  let mut s = format!("{}{}", format_square(from), format_square(to));

  if is_promotion(m) {
    let ch = match promotion_piece(m) {
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

impl fmt::Display for MoveList {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "MoveList len={}", self.len())
  }
}