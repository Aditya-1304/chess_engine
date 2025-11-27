use crate::{
  board::Board, nnue, types::{Color, PieceType}
};

const PAWN_VALUE: i32 = 100;
const KNIGHT_VALUE: i32 = 320;
const BISHOP_VALUE: i32 = 330;
const ROOK_VALUE: i32 = 500;
const QUEEN_VALUE: i32 = 900;
const KING_VALUE: i32 = 20000;

#[rustfmt::skip]
const PAWN_TABLE: [i32; 64] = [
    0,   0,   0,   0,   0,   0,   0,   0, 
    5,  10,  10, -20, -20,  10,  10,   5, 
    5,  -5, -10,   0,   0, -10,  -5,   5, 
    0,   0,   0,  20,  20,   0,   0,   0, 
    5,   5,  10,  25,  25,  10,   5,   5, 
   10,  10,  20,  30,  30,  20,  10,  10, 
   50,  50,  50,  50,  50,  50,  50,  50, 
    0,   0,   0,   0,   0,   0,   0,   0, 
];

#[rustfmt::skip]
const KNIGHT_TABLE: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50,
    -40,-20,  0,  0,  0,  0,-20,-40,
    -30,  0, 10, 15, 15, 10,  0,-30,
    -30,  5, 15, 20, 20, 15,  5,-30,
    -30,  0, 15, 20, 20, 15,  0,-30,
    -30,  5, 10, 15, 15, 10,  5,-30,
    -40,-20,  0,  5,  5,  0,-20,-40,
    -50,-40,-30,-30,-30,-30,-40,-50,
];

#[rustfmt::skip]
const BISHOP_TABLE: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5, 10, 10,  5,  0,-10,
    -10,  5,  5, 10, 10,  5,  5,-10,
    -10,  0, 10, 10, 10, 10,  0,-10,
    -10, 10, 10, 10, 10, 10, 10,-10,
    -10,  5,  0,  0,  0,  0,  5,-10,
    -20,-10,-10,-10,-10,-10,-10,-20,
];

#[rustfmt::skip]
const ROOK_TABLE: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10, 10, 10, 10, 10,  5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     0,  0,  0,  5,  5,  0,  0,  0
];

#[rustfmt::skip]
const QUEEN_TABLE: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5,  5,  5,  5,  0,-10,
     -5,  0,  5,  5,  5,  5,  0, -5,
      0,  0,  5,  5,  5,  5,  0, -5,
    -10,  5,  5,  5,  5,  5,  0,-10,
    -10,  0,  5,  0,  0,  0,  0,-10,
    -20,-10,-10, -5, -5,-10,-10,-20
];

#[rustfmt::skip]
const KING_TABLE: [i32; 64] = [
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -20,-30,-30,-40,-40,-30,-30,-20,
    -10,-20,-20,-20,-20,-20,-20,-10,
     20, 20,  0,  0,  0,  0, 20, 20,
     20, 30, 10,  0,  0, 10, 30, 20
];

pub fn evaluate(board: &Board) -> i32 {
  if nnue::is_enabled() {
    return nnue::evaluate(board);
  }

  let mut score = 0;

  for pt in 0..6 {
    let piece_type = PieceType::from(pt);

    let mut white_pieces = board.pieces[pt][Color::White as usize];
    while white_pieces != 0 {
      let sq = white_pieces.trailing_zeros() as usize;
      score += get_piece_value(piece_type);
      score += get_pst_value(piece_type, sq, Color::White);
      white_pieces &= white_pieces - 1;
    }

    let mut black_pieces = board.pieces[pt][Color::Black as usize];
    while black_pieces != 0 {
      let sq = black_pieces.trailing_zeros() as usize;
      score -= get_piece_value(piece_type);
      score -= get_pst_value(piece_type, sq, Color::Black);
      black_pieces &= black_pieces - 1;
    }
  }

  if board.side_to_move == Color::White {
    score
  } else {
    -score
  }
}

fn get_piece_value(pt: PieceType) -> i32 {
  match pt {
    PieceType::Pawn => PAWN_VALUE,
    PieceType::Knight => KNIGHT_VALUE,
    PieceType::Bishop => BISHOP_VALUE,
    PieceType::Rook => ROOK_VALUE,
    PieceType::Queen => QUEEN_VALUE,
    PieceType::King => KING_VALUE,
  }
}

fn get_pst_value(pt: PieceType, sq: usize, color: Color) -> i32 {
  let table = match pt {
    PieceType::Pawn => &PAWN_TABLE,
    PieceType::Knight => &KNIGHT_TABLE,
    PieceType::Bishop => &BISHOP_TABLE,
    PieceType::Rook => &ROOK_TABLE,
    PieceType::Queen => &QUEEN_TABLE,
    PieceType::King => &KING_TABLE,
  };

  if color == Color::White {
    table[sq]
  } else {
    table[sq ^ 56]
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::board::Board;

  #[test]
  fn test_eval_startpos() {
    let board = Board::default();
    assert_eq!(evaluate(&board), 0);
  }

  #[test]
  fn test_eval_material_imbalance() {
    let board = Board::from_fen("4k3/8/8/8/4P3/8/8/4K3 w - - 0 1").unwrap();
    let score = evaluate(&board);
    assert!(score > 0, "White should be winning with extra pawn");
    assert_eq!(score, 100 + 25);
  }

  #[test]
  fn test_eval_symmetry() {
    let board_w = Board::from_fen("7k/8/8/8/8/8/8/N6K w - - 0 1").unwrap();
    let score_w = evaluate(&board_w);

    let board_b = Board::from_fen("n6k/8/8/8/8/8/8/7K b - - 0 1").unwrap();
    let score_b = evaluate(&board_b);

    assert_eq!(score_w, score_b);
  }
}