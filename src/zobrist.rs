use std::sync::OnceLock;
use crate::polygot_keys::POLYGOT_RANDOM;

pub type ZHash = u64;

// holds all the precomputed random numbers for zobrist hashing
pub struct ZobristKeys {
  pub pieces: [[[ZHash; 64]; 2]; 6],
  pub castling: [ZHash; 16],
  pub en_passant_file: [ZHash; 8],
  pub side_to_move: ZHash,
}

static ZOBRIST_KEYS: OnceLock<ZobristKeys> = OnceLock::new();

impl ZobristKeys {
  fn new() -> Self {
    // let mut rng = StdRng::seed_from_u64(1070373371371371371);
    let mut pieces = [[[0; 64]; 2]; 6];
    for pt_idx in 0..6 {
      for c_idx in 0..2 {
        for sq_idx in 0..64 {
          let polyglot_piece_idx = 2 * pt_idx + ( 1 - c_idx);
          let offset = 64 * polyglot_piece_idx;
          pieces[pt_idx][c_idx][sq_idx] = POLYGOT_RANDOM[offset + sq_idx];
        }
      }
    }

    let mut castling = [0; 16];
    let k_wk = POLYGOT_RANDOM[768];
    let k_wq = POLYGOT_RANDOM[769];
    let k_bk = POLYGOT_RANDOM[770];
    let k_bq = POLYGOT_RANDOM[771];

    for mask in 0..16 {
      let mut hash = 0; 
        if (mask & 0b0001) != 0 { hash ^= k_wk; }
        if (mask & 0b0010) != 0 { hash ^= k_wq; }
        if (mask & 0b0100) != 0 { hash ^= k_bk; }
        if (mask & 0b1000) != 0 { hash ^= k_bq; }
        castling[mask] = hash;
      
    }

    let mut en_passant_file = [0; 8];
    for i in 0..8 {
      en_passant_file[i] = POLYGOT_RANDOM[772 + i];
    }

    let side_to_move = POLYGOT_RANDOM[780];
    ZobristKeys { pieces, castling, en_passant_file, side_to_move}
  }
}

/// Returns a reference to the only Zobristkeys instance
pub fn keys() -> &'static ZobristKeys {
    ZOBRIST_KEYS.get_or_init(ZobristKeys::new)
}