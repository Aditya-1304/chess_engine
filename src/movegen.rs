use crate::{
    board::Board,
    moves::{self, MoveList},
    types::{Bitboard, Color, PieceType, Square},
};
use std::ptr;

// --- Types & Statics ---

#[derive(Clone, Copy, Debug)]
struct Magic {
    mask: Bitboard,
    magic: u64,
    attacks_idx: usize, // Offset into the global attack buffer
    shift: u32,
}

const EMPTY_MAGIC: Magic = Magic {
    mask: 0,
    magic: 0,
    attacks_idx: 0,
    shift: 0,
};

// Global buffers using static mut (Ownership)
static mut BISHOP_ATTACKS_BUF: Vec<Bitboard> = Vec::new();
static mut ROOK_ATTACKS_BUF: Vec<Bitboard> = Vec::new();

// Raw pointers for hot-path access (Performance)
static mut BISHOP_ATTACKS_PTR: *const Bitboard = ptr::null();
static mut ROOK_ATTACKS_PTR: *const Bitboard = ptr::null();

static mut BISHOP_MAGICS: [Magic; 64] = [EMPTY_MAGIC; 64];
static mut ROOK_MAGICS: [Magic; 64] = [EMPTY_MAGIC; 64];

static PAWN_ATTACKS: [[Bitboard; 64]; 2] = precompute_pawn_attacks();
static KNIGHT_ATTACKS: [Bitboard; 64] = precompute_knight_attacks();
static KING_ATTACKS: [Bitboard; 64] = precompute_king_attacks();

// --- Initialization ---

pub fn init() {
    unsafe {
        // Prevent double initialization
        if !BISHOP_ATTACKS_PTR.is_null() { return; }

        // Initialize Bishops
        // Note: We use ptr::addr_of_mut! to avoid creating intermediate references
        // which triggers the static_mut_refs lint in Rust 2024.
        init_magics(
            ptr::addr_of_mut!(BISHOP_MAGICS).cast(), 
            ptr::addr_of_mut!(BISHOP_ATTACKS_BUF), 
            &BISHOP_MAGIC_NUMBERS, 
            bishop_mask, 
            bishop_attacks_slow
        );
        // Get pointer from the Vec (laundering through raw pointer)
        BISHOP_ATTACKS_PTR = (*ptr::addr_of!(BISHOP_ATTACKS_BUF)).as_ptr();

        // Initialize Rooks
        init_magics(
            ptr::addr_of_mut!(ROOK_MAGICS).cast(), 
            ptr::addr_of_mut!(ROOK_ATTACKS_BUF), 
            &ROOK_MAGIC_NUMBERS, 
            rook_mask, 
            rook_attacks_slow
        );
        ROOK_ATTACKS_PTR = (*ptr::addr_of!(ROOK_ATTACKS_BUF)).as_ptr();
    }
}

// --- Hot Path Attack Lookups (Optimized) ---

#[inline(always)]
pub fn pawn_attacks(color: Color, sq: Square) -> Bitboard {
    PAWN_ATTACKS[color as usize][sq as usize]
}

#[inline(always)]
pub fn knight_attacks(sq: Square) -> Bitboard {
    KNIGHT_ATTACKS[sq as usize]
}

#[inline(always)]
pub fn king_attacks(sq: Square) -> Bitboard {
    KING_ATTACKS[sq as usize]
}

#[inline(always)]
pub fn get_bishop_attacks(sq: Square, occupancy: Bitboard) -> Bitboard {
    unsafe {
        // We use addr_of! and direct pointer arithmetic to avoid creating 
        // a reference to the whole array or the static itself.
        let magic_ptr = ptr::addr_of!(BISHOP_MAGICS).cast::<Magic>().add(sq as usize);
        let m = &*magic_ptr; // Dereference just the single element
        
        let idx = ((occupancy & m.mask).wrapping_mul(m.magic)) >> m.shift;
        *BISHOP_ATTACKS_PTR.add(m.attacks_idx + idx as usize)
    }
}

#[inline(always)]
pub fn get_rook_attacks(sq: Square, occupancy: Bitboard) -> Bitboard {
    unsafe {
        let magic_ptr = ptr::addr_of!(ROOK_MAGICS).cast::<Magic>().add(sq as usize);
        let m = &*magic_ptr;
        
        let idx = ((occupancy & m.mask).wrapping_mul(m.magic)) >> m.shift;
        *ROOK_ATTACKS_PTR.add(m.attacks_idx + idx as usize)
    }
}

#[inline(always)]
pub fn is_square_attacked(board: &Board, sq: Square, attacker_color: Color) -> bool {
    let victim = if attacker_color == Color::White { Color::Black } else { Color::White };
    
    // 1. Pawns
    if (pawn_attacks(victim, sq) & board.pieces[PieceType::Pawn as usize][attacker_color as usize]) != 0 {
        return true;
    }

    // 2. Knights
    if (knight_attacks(sq) & board.pieces[PieceType::Knight as usize][attacker_color as usize]) != 0 {
        return true;
    }

    // 3. Kings
    if (king_attacks(sq) & board.pieces[PieceType::King as usize][attacker_color as usize]) != 0 {
        return true;
    }

    // 4. Sliding Pieces
    let occ = board.occupancy[2];
    let attackers = board.occupancy[attacker_color as usize];

    // Bishops/Queens
    let bishop_like = board.pieces[PieceType::Bishop as usize][attacker_color as usize] 
                    | board.pieces[PieceType::Queen as usize][attacker_color as usize];
    
    if bishop_like != 0 {
        if (get_bishop_attacks(sq, occ) & bishop_like) != 0 {
            return true;
        }
    }

    // Rooks/Queens
    let rook_like = board.pieces[PieceType::Rook as usize][attacker_color as usize] 
                  | board.pieces[PieceType::Queen as usize][attacker_color as usize];
    
    if rook_like != 0 {
        if (get_rook_attacks(sq, occ) & rook_like) != 0 {
            return true;
        }
    }

    false
}

// --- Move Generation ---

pub fn generate_pseudo_legal_moves(board: &Board, list: &mut MoveList) {
    generate_pawn_moves(board, list);
    generate_knight_moves(board, list);
    generate_king_moves(board, list);
    generate_sliding_moves(board, list);
}

pub fn generate_captures(board: &Board, list: &mut MoveList) {
    generate_pawn_captures(board, list);
    generate_knight_captures(board, list);
    generate_king_captures(board, list);
    generate_sliding_captures(board, list);
}

fn generate_sliding_moves(board: &Board, list: &mut MoveList) {
    let us = board.side_to_move;
    let occ = board.occupancy[2];
    let our_pieces = board.occupancy[us as usize];
    let their_pieces = board.occupancy[if us == Color::White { 1 } else { 0 }];

    let mut bishops = board.pieces[PieceType::Bishop as usize][us as usize]
        | board.pieces[PieceType::Queen as usize][us as usize];
    while bishops != 0 {
        let from_sq = bishops.trailing_zeros() as Square;
        let attacks = get_bishop_attacks(from_sq, occ) & !our_pieces;
        add_sliding_moves(from_sq, attacks, their_pieces, list);
        bishops &= bishops - 1;
    }

    let mut rooks = board.pieces[PieceType::Rook as usize][us as usize]
        | board.pieces[PieceType::Queen as usize][us as usize];
    while rooks != 0 {
        let from_sq = rooks.trailing_zeros() as Square;
        let attacks = get_rook_attacks(from_sq, occ) & !our_pieces;
        add_sliding_moves(from_sq, attacks, their_pieces, list);
        rooks &= rooks - 1;
    }
}

fn generate_sliding_captures(board: &Board, list: &mut MoveList) {
    let us = board.side_to_move;
    let occ = board.occupancy[2];
    let their_pieces = board.occupancy[if us == Color::White { 1 } else { 0 }];

    let mut bishops = board.pieces[PieceType::Bishop as usize][us as usize]
        | board.pieces[PieceType::Queen as usize][us as usize];
    while bishops != 0 {
        let from_sq = bishops.trailing_zeros() as Square;
        let attacks = get_bishop_attacks(from_sq, occ) & their_pieces;
        add_sliding_captures(from_sq, attacks, list);
        bishops &= bishops - 1;
    }

    let mut rooks = board.pieces[PieceType::Rook as usize][us as usize]
        | board.pieces[PieceType::Queen as usize][us as usize];
    while rooks != 0 {
        let from_sq = rooks.trailing_zeros() as Square;
        let attacks = get_rook_attacks(from_sq, occ) & their_pieces;
        add_sliding_captures(from_sq, attacks, list);
        rooks &= rooks - 1;
    }
}

fn add_sliding_moves(
    from_sq: Square,
    mut moves: Bitboard,
    their_pieces: Bitboard,
    list: &mut MoveList,
) {
    let mut captures = moves & their_pieces;
    while captures != 0 {
        let to_sq = captures.trailing_zeros() as Square;
        list.push(moves::new(from_sq, to_sq, moves::CAPTURE_FLAG));
        captures &= captures - 1;
    }
    
    let mut quiets = moves & !their_pieces;
    while quiets != 0 {
        let to_sq = quiets.trailing_zeros() as Square;
        list.push(moves::new(from_sq, to_sq, moves::QUIET_MOVE_FLAG));
        quiets &= quiets - 1;
    }
}

fn add_sliding_captures(from_sq: Square, mut captures: Bitboard, list: &mut MoveList) {
    while captures != 0 {
        let to_sq = captures.trailing_zeros() as Square;
        list.push(moves::new(from_sq, to_sq, moves::CAPTURE_FLAG));
        captures &= captures - 1;
    }
}

fn generate_pawn_moves(board: &Board, list: &mut MoveList) {
  let us = board.side_to_move;
  let them = if us == Color::White { Color::Black } else { Color::White };
  let our_pawns = board.pieces[PieceType::Pawn as usize][us as usize];
  let their_pieces = board.occupancy[them as usize];
  let all_pieces = board.occupancy[2];

  let (up, rank_start, rank_promo) = if us == Color::White {
    (8i8, 0xFF00u64, 0xFF000000000000u64)
  } else {
    (-8i8, 0xFF000000000000u64, 0xFF00u64)
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
        if (from_bb & rank_promo) != 0 {
          list.push(moves::new(from_sq, to_sq, moves::QUEEN_PROMOTION_FLAG));
          list.push(moves::new(from_sq, to_sq, moves::ROOK_PROMOTION_FLAG));
          list.push(moves::new(from_sq, to_sq, moves::BISHOP_PROMOTION_FLAG));
          list.push(moves::new(from_sq, to_sq, moves::KNIGHT_PROMOTION_FLAG));
        } else {
          list.push(moves::new(from_sq, to_sq, moves::QUIET_MOVE_FLAG));
        }

        // Double push
        if (from_bb & rank_start) != 0 {
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
      if (from_bb & rank_promo) != 0 {
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

  // Castling
  if is_square_attacked(board, king_sq, them) {
    return;
  }

  if us == Color::White {
    // Kingside
    if (board.castling_rights & 0b0001) != 0 
       && (all_pieces & 0x60) == 0  // f1 g1
       && !is_square_attacked(board,5, them)  
       && !is_square_attacked(board,6, them)  
    {
        list.push(moves::new(4, 6, moves::KING_CASTLE_FLAG));
    }
    // Queenside
    if (board.castling_rights & 0b0010) != 0 
       && (all_pieces & 0xE) == 0  // d1 c1 b1
       && !is_square_attacked(board, 3, them)  
       && !is_square_attacked(board, 2, them)  
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

fn generate_pawn_captures(board: &Board, list: &mut MoveList) {
  let us = board.side_to_move;
  let them = if us == Color::White { Color::Black } else { Color::White };
  let our_pawns = board.pieces[PieceType::Pawn as usize][us as usize];
  let their_pieces = board.occupancy[them as usize];

  let rank_promo = if us == Color::White {
     0xFF000000000000u64
  } else {
    0xFF00u64
  };

  let mut pawns = our_pawns;
  while pawns != 0 {
    let from_sq = pawns.trailing_zeros() as Square;
    let from_bb = 1 << from_sq;

    let mut attacks = PAWN_ATTACKS[us as usize][from_sq as usize] & their_pieces;
    while attacks != 0 {
      let to_sq = attacks.trailing_zeros() as Square;
      if (from_bb & rank_promo) !=0 {
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

fn generate_knight_captures(board: &Board, list: &mut MoveList) {
  let us = board.side_to_move;
  let their_pieces = board.occupancy[if us == Color::White { 1 } else { 0 }];
  let mut knights = board.pieces[PieceType::Knight as usize][us as usize];

  while knights != 0 {
    let from_sq = knights.trailing_zeros() as Square;
    let attacks = KNIGHT_ATTACKS[from_sq as usize];
    let mut captures = attacks & their_pieces;
    
    while captures != 0 {
      let to_sq = captures.trailing_zeros() as Square;
      list.push(moves::new(from_sq, to_sq, moves::CAPTURE_FLAG));
      captures &= captures - 1;
    }
    knights &= knights - 1;
  }
}

fn generate_king_captures(board: &Board, list: &mut MoveList) {
  let us = board.side_to_move;
  let their_pieces = board.occupancy[if us == Color::White { 1 } else { 0 }];
  let king_sq = board.pieces[PieceType::King as usize][us as usize].trailing_zeros() as Square;

  let mut attacks = KING_ATTACKS[king_sq as usize] & their_pieces;
  while attacks != 0 {
    let to_sq = attacks.trailing_zeros() as Square;
    list.push(moves::new(king_sq, to_sq, moves::CAPTURE_FLAG));
    attacks &= attacks - 1;
  }
}

// --- Initialization Helpers ---

// NOTE: We take *mut pointers to avoid creating references to static muts
unsafe fn init_magics(
    table_ptr: *mut Magic,
    attack_buf_ptr: *mut Vec<Bitboard>,
    magics: &[u64; 64],
    mask_fn: fn(Square) -> Bitboard,
    attack_fn: fn(Square, Bitboard) -> Bitboard,
) {
    // Launder pointer to reference locally for ease of use
    let attack_buf = &mut *attack_buf_ptr;
    let table_base = table_ptr;

    if attack_buf.capacity() == 0 { attack_buf.reserve(100_000); }

    for sq in 0..64 {
        let mask = mask_fn(sq as Square);
        let bits = mask.count_ones();
        let size = 1 << bits;
        let shift = 64 - bits;
        
        let start_idx = attack_buf.len();
        
        for _ in 0..size { attack_buf.push(0); }
        
        for i in 0..size {
            let occ = occupancy_from_index(i, mask);
            let att = attack_fn(sq as Square, occ);
            let magic_idx = ((occ.wrapping_mul(magics[sq])).wrapping_shr(shift)) as usize;
            attack_buf[start_idx + magic_idx] = att;
        }

        // Write directly to pointer offset
        let entry = Magic {
            mask,
            magic: magics[sq],
            attacks_idx: start_idx,
            shift,
        };
        *table_base.add(sq) = entry;
    }
}

// --- Mask & Slow Attack Generators ---

fn occupancy_from_index(index: usize, mut mask: Bitboard) -> Bitboard {
  let mut occupancy = 0;
  for i in 0..mask.count_ones() {
    let square = mask.trailing_zeros();
    mask &= !(1 << square);
    if (index & (1 << i)) != 0 {
      occupancy |= 1 << square;
    }
  }
  occupancy
}

fn bishop_mask(sq: Square) -> Bitboard {
    let mut result = 0;
    let r = (sq / 8) as i8;
    let f = (sq % 8) as i8;
    for (dr, df) in &[(-1, -1), (-1, 1), (1, -1), (1, 1)] {
        let mut nr = r + dr;
        let mut nf = f + df;
        while nr > 0 && nr < 7 && nf > 0 && nf < 7 {
            result |= 1 << (nr * 8 + nf);
            nr += dr;
            nf += df;
        }
    }
    result
}

fn rook_mask(sq: Square) -> Bitboard {
    let mut result = 0;
    let r = (sq / 8) as i8;
    let f = (sq % 8) as i8;
    for nr in (r + 1)..7 { result |= 1 << (nr * 8 + f); }
    for nr in 1..r { result |= 1 << (nr * 8 + f); }
    for nf in (f + 1)..7 { result |= 1 << (r * 8 + nf); }
    for nf in 1..f { result |= 1 << (r * 8 + nf); }
    result
}

fn bishop_attacks_slow(sq: Square, occupancy: Bitboard) -> Bitboard {
    let mut attacks = 0;
    let r = (sq / 8) as i8;
    let f = (sq % 8) as i8;
    for (dr, df) in &[(-1, -1), (-1, 1), (1, -1), (1, 1)] {
        let mut nr = r + dr;
        let mut nf = f + df;
        while nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
            let bit = 1 << (nr * 8 + nf);
            attacks |= bit;
            if (occupancy & bit) != 0 { break; }
            nr += dr;
            nf += df;
        }
    }
    attacks
}

fn rook_attacks_slow(sq: Square, occupancy: Bitboard) -> Bitboard {
    let mut attacks = 0;
    let r = (sq / 8) as i8;
    let f = (sq % 8) as i8;
    for (dr, df) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let mut nr = r + dr;
        let mut nf = f + df;
        while nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
            let bit = 1 << (nr * 8 + nf);
            attacks |= bit;
            if (occupancy & bit) != 0 { break; }
            nr += dr;
            nf += df;
        }
    }
    attacks
}

// --- Precomputed Tables ---

const fn precompute_pawn_attacks() -> [[Bitboard; 64]; 2] {
  let mut attacks = [[0; 64]; 2];
  let mut sq = 0;
  while sq < 64 {
    let mut bb = 0;
    if (sq / 8) < 7 {
      if (sq % 8) > 0 { bb |= 1 << (sq + 7); }
      if (sq % 8) < 7 { bb |= 1 << (sq + 9); }
    }
    attacks[Color::White as usize][sq] = bb;
    bb = 0;
    if (sq / 8) > 0 {
      if (sq % 8) > 0 { bb |= 1 << (sq - 9); }
      if (sq % 8) < 7 { bb |= 1 << (sq - 7); }
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
    let r = sq / 8;
    let f = sq % 8;
    if r < 6 && f < 7 { bb |= 1 << (sq + 17); }
    if r < 6 && f > 0 { bb |= 1 << (sq + 15); }
    if r < 7 && f < 6 { bb |= 1 << (sq + 10); }
    if r < 7 && f > 1 { bb |= 1 << (sq + 6); }
    if r > 1 && f < 7 { bb |= 1 << (sq - 15); }
    if r > 1 && f > 0 { bb |= 1 << (sq - 17); }
    if r > 0 && f < 6 { bb |= 1 << (sq - 6); }
    if r > 0 && f > 1 { bb |= 1 << (sq - 10); }
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
    let r = sq / 8;
    let f = sq % 8;
    if r < 7 { bb |= 1 << (sq + 8); }
    if r > 0 { bb |= 1 << (sq - 8); }
    if f < 7 { bb |= 1 << (sq + 1); }
    if f > 0 { bb |= 1 << (sq - 1); }
    if r < 7 && f < 7 { bb |= 1 << (sq + 9); }
    if r < 7 && f > 0 { bb |= 1 << (sq + 7); }
    if r > 0 && f < 7 { bb |= 1 << (sq - 7); }
    if r > 0 && f > 0 { bb |= 1 << (sq - 9); }
    attacks[sq] = bb;
    sq += 1;
  }
  attacks
}


// BISHOP MAGIC NUMBERS
static BISHOP_MAGIC_NUMBERS: [u64; 64] = [
    0x40440080810102,
    0x4831011a0a001e,
    0x206800840080a050,
    0x4040080008200,
    0x8484011850200,
    0x13c300828004800,
    0x1000808818c12488,
    0x4200809148200408,
    0x82098088100,
    0x108002022c010200,
    0x882800c08880,
    0xaa4081008000,
    0x200841c20802000,
    0x20020104204001,
    0xc1000c01093050e1,
    0x600360342082440,
    0x1002090a080818,
    0x2180410021208,
    0x4108000102040054,
    0x1cc8000404208c02,
    0x4001211200008,
    0x2924084200920810,
    0x400a20404010815,
    0x380400200421808,
    0x1520082810100104,
    0x21380210901100,
    0x6080820010041090,
    0x480004021020,
    0x9000840022020202,
    0x4024460013010302,
    0x6204008200a208,
    0x618020010c0600,
    0x26086020c42000,
    0x888029000028400,
    0x840201000800d0,
    0x420600800010104,
    0x40010100004440,
    0x6810245200104104,
    0x1220081062801,
    0x30c61020000a081,
    0x8012c2082000c046,
    0x214240208000200,
    0x1202820082084040,
    0x202013020800,
    0x840080100410407,
    0x5200283000082,
    0x44810204000200,
    0x5810018091140281,
    0xc140402184050,
    0x8001118801084002,
    0x8003908080418,
    0x408000406088008c,
    0x410001002020118,
    0x14231622020088,
    0x1020d181020060,
    0x4220041340450100,
    0x4002008401411000,
    0x2080010101108200,
    0xac49000020841000,
    0x9040040113840401,
    0x5010c40128a00,
    0x11801220090100,
    0x10428300c0080,
    0xc028814408020021,
];

// ROOK MAGIC NUMBERS (CORRECTED)
static ROOK_MAGIC_NUMBERS: [u64; 64] = [
    0x4680002330804004,
    0x100106040008500,
    0x80200188100081,
    0x208010000e802800,
    0x22000a0120045008,
    0x6100040081002802,
    0x80020000801100,
    0x118008688000c100,
    0x40800a60814004,
    0x3000808040002000,
    0x91007041002000,
    0x2001000900201001,
    0xa001000500100800,
    0x11020008a200300c,
    0x401006600210004,
    0x8402000043042082,
    0x2004208002824002,
    0x1196808040002000,
    0x30008020008131,
    0x4900220042000810,
    0x1008010005001810,
    0x4400808002000401,
    0x802a010100040200,
    0x220004440081,
    0x1804410100218000,
    0x8400a00040045000,
    0x62001c040300800,
    0x1c01002100081000,
    0x80c008080080086,
    0x10060002002490c8,
    0x80d0080400020110,
    0x500800080004100,
    0x1408800443002302,
    0x21a0003000c00140,
    0x200088801000,
    0x9212100080800800,
    0xa002052001488,
    0x114810c00800200,
    0x400200a302000804,
    0x401028042000504,
    0x6084822040108000,
    0x460008040008030,
    0x8001001020050040,
    0x128005000818029,
    0x111000408010010,
    0x400100080401000e,
    0xb05000600090004,
    0x1060010448820004,
    0x4080118040002080,
    0x5000e001400140,
    0x9880100020028080,
    0x400100008048180,
    0x24900101c080100,
    0x8028040042008080,
    0x2021100288010400,
    0x8100144400930200,
    0x8800401025008009,
    0x50110200244086,
    0x400b02004400901,
    0x2600408100101,
    0x22000820055002,
    0x192001001044802,
    0x1089000400860001,
    0x4100089020c201,
];