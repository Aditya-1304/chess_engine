use crate::{
 board::Board,
 moves::{self, MoveList},
 types::{Bitboard, Color, PieceType, Square}
};

static PAWN_ATTACKS: [[Bitboard; 64]; 2] = precompute_pawn_attacks();
static  KNIGHT_ATTACKS: [Bitboard; 64] = precompute_knight_attacks();
static KING_ATTACKS: [Bitboard; 64] = precompute_king_attacks();

pub fn pawn_attacks(color: Color, sq: Square) -> Bitboard { PAWN_ATTACKS[color as usize][sq as usize] }
pub fn knight_attacks(sq: Square) -> Bitboard { KNIGHT_ATTACKS[sq as usize] }
pub fn king_attacks(sq: Square) -> Bitboard { KING_ATTACKS[sq as usize] }

const fn precompute_pawn_attacks() -> [[Bitboard; 64]; 2] {
  let mut attacks = [[0; 64]; 2];
  let mut sq = 0;
  while sq < 64 {
    let mut bb = 0;
    if (sq / 8) < 7 {
      if (sq % 8) > 0 {
        bb |= 1 << (sq + 7); 
      }
      if (sq % 8) < 7 {
        bb |= 1 << (sq + 9);
      }
    }
    attacks[Color::White as usize][sq] = bb;

    bb = 0;
    if (sq / 8) > 0 {
      if (sq % 8) > 0 {
        bb |= 1 << (sq - 9);
      }
      if (sq % 8) < 7 {
        bb |= 1 << (sq - 7);
      }
    }
    attacks[Color::Black as usize][sq] = bb;
    sq += 1;
  }
  attacks
}

const fn precompute_knight_attacks() -> [Bitboard; 64] {
  let mut attacks = [0; 64];
  let mut sq = 0;
  while sq < 64 {
    let mut bb = 0;
    let rank = sq / 8;
    let file = sq % 8;

    if rank < 6 && file < 7 { bb |= 1 << (sq + 17); } // 2 up, 1 right
    if rank < 6 && file > 0 { bb |= 1 << (sq + 15); } // 2 up, 1 left
    if rank < 7 && file < 6 { bb |= 1 << (sq + 10); } // 1 up, 2 right
    if rank < 7 && file > 1 { bb |= 1 << (sq + 6); } // 1 up, 2 left
    if rank > 1 && file < 7 { bb |= 1 << (sq - 15); } // 2 down, 1 right
    if rank > 1 && file > 0 { bb |= 1 << (sq - 17); } // 2 down, 1 left
    if rank > 0 && file < 6 { bb |= 1 << (sq - 6); } // 1 down , 1 right
    if rank > 0 && file > 1 { bb |= 1 << (sq - 10); } // 1 down, 1 left

    attacks[sq] = bb;
    sq += 1;
  }
  attacks
}

const fn precompute_king_attacks() -> [Bitboard; 64] {
  let mut attacks = [0; 64];
  let mut sq = 0;
  while sq < 64 {
    let mut bb = 0;
    let rank = sq / 8;
    let file = sq % 8;

    if rank < 7 { bb |= 1 << (sq + 8); } // up
    if rank > 0 { bb |= 1 << (sq - 8); } // down
    if file < 7 { bb |= 1 << (sq + 1); } // right
    if file > 0 { bb |= 1 << (sq - 1); } // left
    if rank < 7 && file < 7 { bb |= 1 << (sq + 9); } // up-right
    if rank < 7 && file > 0 { bb |= 1 << (sq + 7); } // up-left
    if rank > 0 && file < 7 { bb |= 1 << (sq - 7); } // down-right
    if rank > 0 && file > 0 { bb |= 1 << (sq - 9); } // down-left

    attacks[sq] = bb;
    sq += 1;
  }
  attacks
}

pub fn generate_pseudo_legal_moves(board: &Board, list: &mut MoveList) {
  generate_pawn_moves(board, list);
  generate_knight_moves(board, list);
  generate_king_moves(board, list);
}

fn generate_king_moves(board: &Board, list: &mut MoveList) {
  let us = board.side_to_move;
  let them = if us == Color::White { Color::Black } else { Color::White };
  let our_pieces = board.occupancy[us as usize];
  let all_pieces = board.occupancy[2];

  let king_sq = board.pieces[PieceType::King as usize][us as usize].trailing_zeros() as Square;

  let mut attacks = KING_ATTACKS[king_sq as usize] & !our_pieces;
  while attacks != 0 {
    let to_sq = attacks.trailing_zeros() as Square;
    let flag = if (1 << to_sq) & board.occupancy[them as usize] != 0 {
      moves::CAPTURE_FLAG
    } else {
      moves::QUIET_MOVE_FLAG
    };
    list.push(moves::new(king_sq, to_sq, flag));
    attacks &= attacks - 1;
  }

  if board.is_square_attacked(king_sq, them) {
    return;
  }

  if us == Color::White {
    // Kingside
    if (board.castling_rights & 0b0001) != 0 && (all_pieces & 0x60) == 0 {
      if !board.is_square_attacked(5, them) && !board.is_square_attacked(6, them) {
        list.push(moves::new(4, 6, moves::KING_CASTLE_FLAG));
      }
    }
    // Queenside
    if (board.castling_rights & 0b0010) != 0 && (all_pieces & 0xE) == 0 {
      if !board.is_square_attacked(2, them) && !board.is_square_attacked(3, them) {
        list.push(moves::new(4, 2, moves::QUEEN_CASTLE_FLAG));
      }
    }
} else {
    // Kingside
    if (board.castling_rights & 0b0100) != 0 && (all_pieces & 0x6000000000000000) == 0 {
      if !board.is_square_attacked(61, them) && !board.is_square_attacked(62, them) {
        list.push(moves::new(60, 62, moves::KING_CASTLE_FLAG));
      }
    }
    // Queenside
    if (board.castling_rights & 0b1000) != 0 && (all_pieces & 0xE00000000000000) == 0 {
      if !board.is_square_attacked(58, them) && !board.is_square_attacked(59, them) {
        list.push(moves::new(60, 58, moves::QUEEN_CASTLE_FLAG));
      }
    }
  }
}

fn generate_knight_moves(board: &Board, list: &mut MoveList) {
  let us = board.side_to_move;
  let our_pieces = board.occupancy[us as usize];
  let their_pieces = board.occupancy[if us == Color::White { 1 } else { 0 }];
  let mut knights = board.pieces[PieceType::Knight as usize][us as usize];

  while knights != 0 {
    let from_sq = knights.trailing_zeros() as Square;
    let attacks = KNIGHT_ATTACKS[from_sq as usize];

    let mut quiet_moves = attacks & !our_pieces & !their_pieces;
    while quiet_moves != 0 {
      let to_sq = quiet_moves.trailing_zeros() as Square;
      list.push(moves::new(from_sq, to_sq, moves::QUIET_MOVE_FLAG));
      quiet_moves &= quiet_moves - 1;
    }

    let mut captures = attacks & their_pieces;
    while captures != 0 {
      let to_sq = captures.trailing_zeros() as Square;
      list.push(moves::new(from_sq, to_sq, moves::CAPTURE_FLAG));
      captures &= captures - 1;
    }

    knights &= knights - 1;
  }
}

fn generate_pawn_moves(board: &Board, list: &mut MoveList) {
  let us = board.side_to_move;
  let them = if us == Color::White { Color::Black } else { Color::White };
  let our_pawns = board.pieces[PieceType::Pawn as usize][us as usize];
  let their_pieces = board.occupancy[them as usize];
  let all_pieces = board.occupancy[2];

  let (up, rank_3, rank_7) = if us == Color::White {
    (8, 0xFF00u64, 0xFF000000000000u64) // White: Rank 2, Rank 7
  } else {
    (-8, 0xFF000000000000u64, 0xFF00u64) // Black: Rank 7, Rank 2
  };

  let mut pawns = our_pawns;
  while pawns != 0 {
    let from_sq = pawns.trailing_zeros() as Square;
    let from_bb = 1 << from_sq;

    let to_sq = (from_sq as i8 + up) as Square;
    if (1 << to_sq) & all_pieces == 0 {
            
      if (from_bb & rank_7) != 0 {
        list.push(moves::new(from_sq, to_sq, moves::QUEEN_PROMOTION_FLAG));
        list.push(moves::new(from_sq, to_sq, moves::ROOK_PROMOTION_FLAG));
        list.push(moves::new(from_sq, to_sq, moves::BISHOP_PROMOTION_FLAG));
        list.push(moves::new(from_sq, to_sq, moves::KNIGHT_PROMOTION_FLAG));
      } else {
        list.push(moves::new(from_sq, to_sq, moves::QUIET_MOVE_FLAG));
      }

      if (from_bb & rank_3) != 0 {
        let to_sq_double = (from_sq as i8 + 2 * up) as Square;
        if (1 << to_sq_double) & all_pieces == 0 {
          list.push(moves::new(
            from_sq,
            to_sq_double,
            moves::DOUBLE_PAWN_PUSH_FLAG,
          ));
        }
      }
    }

    let mut attacks = PAWN_ATTACKS[us as usize][from_sq as usize] & their_pieces;
      while attacks != 0 {
        let to_sq = attacks.trailing_zeros() as Square;
        if (from_bb & rank_7) != 0 {
          list.push(moves::new(from_sq, to_sq, moves::QUEEN_PROMOTION_CAPTURE_FLAG));
          list.push(moves::new(from_sq, to_sq, moves::ROOK_PROMOTION_CAPTURE_FLAG));
          list.push(moves::new(from_sq, to_sq, moves::BISHOP_PROMOTION_CAPTURE_FLAG));
          list.push(moves::new(from_sq, to_sq, moves::KNIGHT_PROMOTION_CAPTURE_FLAG));
        } else {
          list.push(moves::new(from_sq, to_sq, moves::CAPTURE_FLAG));
        }
        attacks &= attacks - 1;
      }

      if let Some(ep_sq) = board.en_passant {
        if PAWN_ATTACKS[us as usize][from_sq as usize] & (1 << ep_sq) != 0 {
          list.push(moves::new(from_sq, ep_sq, moves::EN_PASSANT_CAPTURE_FLAG));
        }
      }

    pawns &= pawns - 1;
  }

}