pub type Bitboard = u64;
pub type Square = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
  White,
  Black,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C, align(64))]
pub struct Accumulator {
  pub values: [i16; 256],
}

impl Default for Accumulator {
  fn default() -> Self {
      Accumulator { values: [0; 256] }
  }
}

impl Accumulator {
  pub fn copy_from_slice(&mut self, src: &[i16]) {
    self.values.copy_from_slice(&src[0..256]);
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PieceType {
  Pawn,
  Knight,
  Bishop,
  Rook,
  Queen,
  King,
}

impl From<usize> for PieceType {
  fn from(val: usize) -> Self {
    match val {
        0 => PieceType::Pawn,
        1 => PieceType::Knight,
        2 => PieceType::Bishop,
        3 => PieceType::Rook,
        4 => PieceType::Queen,
        5 => PieceType::King,
        _=> unreachable!(),
    }
  }
}

pub type Move = u16;

