use crate::{
 board::Board,
 moves::{self, MoveList},
 types::{Bitboard, Color, PieceType, Square}
};
use std::sync::OnceLock;

#[derive(Clone, Copy)]
struct Magic {
  mask: Bitboard,
  magic: u64,
  attacks: &'static [Bitboard],
  shift: u32,
}

static PAWN_ATTACKS: [[Bitboard; 64]; 2] = precompute_pawn_attacks();
static  KNIGHT_ATTACKS: [Bitboard; 64] = precompute_knight_attacks();
static KING_ATTACKS: [Bitboard; 64] = precompute_king_attacks();
static BISHOP_MAGIC_TABLE: OnceLock<[Magic; 64]> = OnceLock::new();
static ROOK_MAGIC_TABLE: OnceLock<[Magic; 64]> = OnceLock::new();

pub fn init() {
    BISHOP_MAGIC_TABLE.get_or_init(init_bishop_magics);
    ROOK_MAGIC_TABLE.get_or_init(init_rook_magics);
}

pub fn pawn_attacks(color: Color, sq: Square) -> Bitboard { 
  PAWN_ATTACKS[color as usize][sq as usize] 
}

pub fn knight_attacks(sq: Square) -> Bitboard { 
  KNIGHT_ATTACKS[sq as usize] 
}

pub fn king_attacks(sq: Square) -> Bitboard { 
  KING_ATTACKS[sq as usize] 
}

pub fn get_bishop_attacks(sq: Square, occupancy: Bitboard) -> Bitboard {
  let magics = BISHOP_MAGIC_TABLE.get().unwrap();
  let magic = &magics[sq as usize];
  let index = ((occupancy & magic.mask).wrapping_mul(magic.magic)) >> magic.shift;
  magic.attacks[index as usize]
}

pub fn get_rook_attacks(sq: Square, occupancy: Bitboard) -> Bitboard {
  let magics = ROOK_MAGIC_TABLE.get().unwrap();
  let magic = &magics[sq as usize];
  let index = ((occupancy & magic.mask).wrapping_mul(magic.magic)) >> magic.shift;
  magic.attacks[index as usize]
}


pub fn is_square_attacked(board: &Board, sq: Square, attacker_color: Color) -> bool {
  let opponent_pieces = board.pieces;

  let victim_color = if attacker_color == Color::White { Color::Black } else { Color::White };
  let pawn_attacks = pawn_attacks(victim_color, sq);
  if (pawn_attacks & board.pieces[PieceType::Pawn as usize][attacker_color as usize]) != 0 {
    return true;
  }

  let knight_attacks = knight_attacks(sq);
  if (knight_attacks & board.pieces[PieceType::Knight as usize][attacker_color as usize]) != 0 {
    return true;
  }

  let king_attacks = king_attacks(sq);
  if (king_attacks & board.pieces[PieceType::King as usize][attacker_color as usize]) != 0 {
    return true;
  }

  let bishop_attacks = get_bishop_attacks(sq, board.occupancy[2]);
  if (bishop_attacks
      & (opponent_pieces[PieceType::Bishop as usize][attacker_color as usize]
          | opponent_pieces[PieceType::Queen as usize][attacker_color as usize]))
      != 0
  {
      return true;
  }

  let rook_attacks = get_rook_attacks(sq, board.occupancy[2]);
  if (rook_attacks
      & (opponent_pieces[PieceType::Rook as usize][attacker_color as usize]
          | opponent_pieces[PieceType::Queen as usize][attacker_color as usize]))
      != 0
  {
      return true;
  }

  false
}

pub fn generate_pseudo_legal_moves(board: &Board, list: &mut MoveList) {
  generate_pawn_moves(board, list);
  generate_knight_moves(board, list);
  generate_king_moves(board, list);
  generate_sliding_moves(board, list);
}

fn generate_sliding_moves(board: &Board, list: &mut MoveList) {
  let us = board.side_to_move;
  let our_pieces = board.occupancy[us as usize];
  let their_pieces = board.occupancy[if us == Color::White { 1 } else { 0 }];
  let all_pieces = our_pieces | their_pieces;

  let mut bishops = board.pieces[PieceType::Bishop as usize][us as usize]
    | board.pieces[PieceType::Queen as usize][us as usize];
  while bishops != 0 {
    let from_sq = bishops.trailing_zeros() as Square;
    let attacks = get_bishop_attacks(from_sq, all_pieces) & !our_pieces;
    add_sliding_moves(from_sq, attacks, their_pieces, list);
    bishops &= bishops - 1;
  }

  let mut rooks = board.pieces[PieceType::Rook as usize][us as usize]
    | board.pieces[PieceType::Queen as usize][us as usize];
  while rooks != 0 {
    let from_sq = rooks.trailing_zeros() as Square;
    let attacks = get_rook_attacks(from_sq, all_pieces) & !our_pieces;
    add_sliding_moves(from_sq, attacks, their_pieces, list);
    rooks &= rooks - 1;
  }
}

fn add_sliding_moves(
  from_sq: Square,
  attacks: Bitboard,
  their_pieces: Bitboard,
  list: &mut MoveList,
) {
  let mut captures = attacks & their_pieces;
  while captures != 0{
    let to_sq = captures.trailing_zeros() as Square;
    list.push(moves::new(from_sq, to_sq, moves::CAPTURE_FLAG));
    captures &= captures - 1;
  }
  let mut quiet_moves = attacks & !their_pieces;
  while quiet_moves != 0 {
    let to_sq = quiet_moves.trailing_zeros() as Square;
    list.push(moves::new(from_sq, to_sq, moves::QUIET_MOVE_FLAG));
    quiet_moves &= quiet_moves - 1;
  }
}

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
    if rank > 0 && file < 6 { bb |= 1 << (sq - 6); } // 1 down , 2 right
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

fn generate_king_moves(board: &Board, list: &mut MoveList) {
  let us = board.side_to_move;
  let them = if us == Color::White { Color::Black } else { Color::White };
  let our_pieces = board.occupancy[us as usize];
  let all_pieces = board.occupancy[2];

  let king_sq = board.pieces[PieceType::King as usize][us as usize].trailing_zeros() as Square;

  // Normal king moves
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

  // Castling - can't castle from check
  if is_square_attacked(board, king_sq, them) {
    return;
  }

  if us == Color::White {
    // Kingside: squares f1(5) and g1(6) must be empty and not attacked
    if (board.castling_rights & 0b0001) != 0 
       && (all_pieces & 0x60) == 0  // f1 and g1 empty
       && !is_square_attacked(board,5, them)  // f1 not attacked
       && !is_square_attacked(board,6, them)  // g1 not attacked
    {
        list.push(moves::new(4, 6, moves::KING_CASTLE_FLAG));
    }
    // Queenside: squares d1(3), c1(2), b1(1) - only d1,c1 need attack check
    if (board.castling_rights & 0b0010) != 0 
       && (all_pieces & 0xE) == 0  // d1, c1, b1 empty
       && !is_square_attacked(board, 3, them)  // d1 not attacked
       && !is_square_attacked(board, 2, them)  // c1 not attacked
    {
        list.push(moves::new(4, 2, moves::QUEEN_CASTLE_FLAG));
    }
  } else {
    // Black kingside
    if (board.castling_rights & 0b0100) != 0 
       && (all_pieces & 0x6000000000000000) == 0
       && !is_square_attacked(board, 61, them) 
       && !is_square_attacked(board, 62, them)
    {
        list.push(moves::new(60, 62, moves::KING_CASTLE_FLAG));
    }
    // Black queenside
    if (board.castling_rights & 0b1000) != 0 
       && (all_pieces & 0xE00000000000000) == 0
       && !is_square_attacked(board, 59, them) 
       && !is_square_attacked(board, 58, them)
    {
        list.push(moves::new(60, 58, moves::QUEEN_CASTLE_FLAG));
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
    (8i8, 0xFF00u64, 0xFF000000000000u64) // White: Rank 2, Rank 7
  } else {
    (-8i8, 0xFF000000000000u64, 0xFF00u64) // Black: Rank 7, Rank 2
  };

  let mut pawns = our_pawns;
  while pawns != 0 {
    let from_sq = pawns.trailing_zeros() as Square;
    let from_bb = 1 << from_sq;

    // Single push
    let to_sq_i8 = from_sq as i8 + up;
    if to_sq_i8 >= 0 && to_sq_i8 < 64 {
      let to_sq = to_sq_i8 as Square;
      if (1 << to_sq) & all_pieces == 0 {
        if (from_bb & rank_7) != 0 {
          list.push(moves::new(from_sq, to_sq, moves::QUEEN_PROMOTION_FLAG));
          list.push(moves::new(from_sq, to_sq, moves::ROOK_PROMOTION_FLAG));
          list.push(moves::new(from_sq, to_sq, moves::BISHOP_PROMOTION_FLAG));
          list.push(moves::new(from_sq, to_sq, moves::KNIGHT_PROMOTION_FLAG));
        } else {
          list.push(moves::new(from_sq, to_sq, moves::QUIET_MOVE_FLAG));
        }

        // Double push
        if (from_bb & rank_3) != 0 {
          let to_sq_double_i8 = from_sq as i8 + 2 * up;
          if to_sq_double_i8 >= 0 && to_sq_double_i8 < 64 {
            let to_sq_double = to_sq_double_i8 as Square;
            if (1 << to_sq_double) & all_pieces == 0 {
              list.push(moves::new(
                from_sq,
                to_sq_double,
                moves::DOUBLE_PAWN_PUSH_FLAG,
              ));
            }
          }
        }
      }
    }

    // Captures
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

    // En passant
    if let Some(ep_sq) = board.en_passant {
      if PAWN_ATTACKS[us as usize][from_sq as usize] & (1 << ep_sq) != 0 {
        list.push(moves::new(from_sq, ep_sq, moves::EN_PASSANT_CAPTURE_FLAG));
      }
    }

    pawns &= pawns - 1;
  }
}


fn init_bishop_magics() -> [Magic; 64] {
    let mut bishop_attacks = Vec::new();
    let mut attacks_info = Vec::new();
    for sq in 0..64 {
        let mask = bishop_mask(sq);
        let relevant_bits = mask.count_ones();
        let table_size = 1 << relevant_bits;
        let start_index = bishop_attacks.len();
        attacks_info.push((start_index, table_size));

        for i in 0..table_size {
            let occupancy = occupancy_from_index(i, mask);
            bishop_attacks.push(bishop_attacks_slow(sq, occupancy));
        }
    }

    let static_attacks = Box::leak(bishop_attacks.into_boxed_slice());

    let mut magics = [Magic {
        mask: 0,
        magic: 0,
        attacks: &[],
        shift: 0,
    }; 64];
    for sq in 0..64 {
        let mask = bishop_mask(sq);
        let relevant_bits = mask.count_ones();
        let (start, len) = attacks_info[sq as usize];
        magics[sq as usize] = Magic {
            mask,
            magic: BISHOP_MAGICS[sq as usize],
            attacks: &static_attacks[start..start + len],
            shift: 64 - relevant_bits,
        };
    }
    magics
}

fn init_rook_magics() -> [Magic; 64] {
    let mut rook_attacks = Vec::new();
    let mut attacks_info = Vec::new();
    for sq in 0..64 {
        let mask = rook_mask(sq);
        let relevant_bits = mask.count_ones();
        let table_size = 1 << relevant_bits;
        let start_index = rook_attacks.len();
        attacks_info.push((start_index, table_size));

        for i in 0..table_size {
            let occupancy = occupancy_from_index(i, mask);
            rook_attacks.push(rook_attacks_slow(sq, occupancy));
        }
    }

    let static_attacks = Box::leak(rook_attacks.into_boxed_slice());

    let mut magics = [Magic {
        mask: 0,
        magic: 0,
        attacks: &[],
        shift: 0,
    }; 64];
    for sq in 0..64 {
        let mask = rook_mask(sq);
        let relevant_bits = mask.count_ones();
        let (start, len) = attacks_info[sq as usize];
        magics[sq as usize] = Magic {
            mask,
            magic: ROOK_MAGICS[sq as usize],
            attacks: &static_attacks[start..start + len],
            shift: 64 - relevant_bits,
        };
    }
    magics
}

fn occupancy_from_index(index: usize, mut mask: Bitboard) -> Bitboard {
  let mut occupancy = 0;
  for i in 0..mask.count_ones() {
    let sq = mask.trailing_zeros();
    mask &= mask - 1;
    if (index >> i) & 1 != 0 {
      occupancy |= 1 << sq;
    }
  }
  occupancy
}
fn bishop_mask(sq: Square) -> Bitboard {
    let mut result = 0;
    let r = sq / 8;
    let f = sq % 8;

    // Up-right
    for i in 1.. {
        if r + i > 6 || f + i > 6 { break; } // Stop *one* square before edge
        result |= 1 << ((r + i) * 8 + (f + i));
    }
    // Up-left
    for i in 1.. {
        if r + i > 6 || f < i || f - i < 1 { break; } // Stop *one* square before edge
        result |= 1 << ((r + i) * 8 + (f - i));
    }
    // Down-right
    for i in 1.. {
        if r < i || r - i < 1 || f + i > 6 { break; } // Stop *one* square before edge
        result |= 1 << ((r - i) * 8 + (f + i));
    }
    // Down-left
    for i in 1.. {
        if r < i || r - i < 1 || f < i || f - i < 1 { break; } // Stop *one* square before edge
        result |= 1 << ((r - i) * 8 + (f - i));
    }
    result
}

// ADD THIS CORRECT VERSION
fn rook_mask(sq: Square) -> Bitboard {
    let mut result = 0;
    let r = sq / 8;
    let f = sq % 8;

    // Right
    for i in (f + 1)..7 { // Loop from f+1 up to (but not including) 7
        result |= 1 << (r * 8 + i);
    }
    // Left
    for i in 1..f { // Loop from 1 up to (but not including) f
        result |= 1 << (r * 8 + i);
    }
    // Up
    for i in (r + 1)..7 { // Loop from r+1 up to (but not including) 7
        result |= 1 << (i * 8 + f);
    }
    // Down
    for i in 1..r { // Loop from 1 up to (but not including) r
        result |= 1 << (i * 8 + f);
    }
    result
}
fn bishop_attacks_slow(sq: Square, occupancy: Bitboard) -> Bitboard {
    let mut attacks = 0;
    let dirs = [7, 9, -7, -9]; // up-left, up-right, down-left, down-right

    for &dir in &dirs {
        let mut s = sq as i8;
        loop {
            let prev_s = s;
            s += dir;

            // Check for out-of-bounds
            if s < 0 || s >= 64 {
                break;
            }

            // Check for wrap-around: the file distance between the previous
            // and current square must be exactly 1 for a diagonal move.
            let prev_file = prev_s % 8;
            let new_file = s % 8;
            if (new_file - prev_file).abs() != 1 {
                break;
            }

            attacks |= 1 << s;
            if (1 << s) & occupancy != 0 {
                break; // Stop at first blocker
            }
        }
    }
    attacks
}

fn rook_attacks_slow(sq: Square, occupancy: Bitboard) -> Bitboard {
    let mut attacks = 0;
    let dirs = [1, -1, 8, -8]; // right, left, up, down
    let r = sq / 8; // The *original* rank
    let f = sq % 8; // The *original* file

    for &dir in &dirs {
        let mut s = sq as i8;
        loop {
            s += dir;

            // 1. Check for out-of-bounds
            if s < 0 || s >= 64 {
                break;
            }

            let new_sq = s as Square;
            let nr = new_sq / 8; // The new rank
            let nf = new_sq % 8; // The new file

            // 2. Check for wrap-around (compare new rank/file to *original*)
            if (dir == 1 || dir == -1) && nr != r { // Horizontal move changed rank
                break;
            }
            if (dir == 8 || dir == -8) && nf != f { // Vertical move changed file
                break;
            }

            attacks |= 1 << new_sq;
            if (1 << new_sq) & occupancy != 0 {
                break; // Stop at first blocker
            }
        }
    }
    attacks
}

static BISHOP_MAGICS: [u64; 64] = [
    0x400408448408400,
    0x2004208a004208,
    0x10190041080202,
    0x10806080400,
    0x204000212008,
    0x1000810040402,
    0x404000804080,
    0x4080200000,
    0x80800400080400,
    0x401004040808,
    0x20080104010,
    0x80004010002,
    0x20002008080,
    0x8080040200,
    0x4000802080,
    0x2100000,
    0x800080400,
    0x8000808040,
    0x1000050040,
    0x400000208,
    0x8000400,
    0x400080,
    0x2000,
    0x80,
    0x804010020,
    0x40100080080,
    0x200800400,
    0x8008040,
    0x8000,
    0x80,
    0x40,
    0,
    0x402000100,
    0x20100004,
    0x1008020,
    0x8040,
    0,
    0,
    0,
    0,
    0x800080100,
    0x400040080,
    0x20002004,
    0x8008,
    0,
    0,
    0,
    0,
    0x400000080,
    0x200000040,
    0x100000020,
    0x8000000,
    0,
    0,
    0,
    0,
    0x8000020400,
    0x4000010200,
    0x2000008100,
    0x1000004080,
    0x8000002040,
    0x4000001020,
    0x2000000810,
    0x1000000400,
];
static ROOK_MAGICS: [u64; 64] = [
    0x8a80104000800020,
    0x140002000100040,
    0x28000100004020,
    0x100008000200040,
    0x20002001008040,
    0x1000400020080,
    0x200040080100,
    0x28008004002000,
    0x8000800800400,
    0x10000200100,
    0x20000100040,
    0x8000800200,
    0x4000100008,
    0x8000400,
    0x400080,
    0x80,
    0x8000808004000,
    0x1004000802000,
    0x208004000100,
    0x1000800200040,
    0x200040010008,
    0x8000800040,
    0x1000800,
    0x2000,
    0x808008004000,
    0x10200100080,
    0x401000400,
    0x80080020,
    0x400010,
    0x800,
    0x40,
    0,
    0x808004002000,
    0x10100080040,
    0x400800200,
    0x80040010,
    0x20008,
    0x400,
    0x20,
    0,
    0x808002001000,
    0x10080040020,
    0x40040020,
    0x800200,
    0x100,
    0x2,
    0,
    0,
    0x40800800400,
    0x8040040020,
    0x20200200,
    0x40100,
    0x8,
    0,
    0,
    0,
    0x4004080200800,
    0x80020800400,
    0x1001040020,
    0x2000400,
    0x800,
    0,
    0,
    0,
];