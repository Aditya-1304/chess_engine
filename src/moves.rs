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

