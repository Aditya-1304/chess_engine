use crate::types::{Color, PieceType, Square};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::sync::OnceLock;

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
    let mut rng = StdRng::seed_from_u64(1070373371371371371);
    let mut pieces = [[[0; 64]; 2]; 6];
    for pt_idx in 0..6 {
      for c_idx in 0..2 {
        for sq_idx in 0..64 {
          pieces[pt_idx][c_idx][sq_idx] = rng.random();
        }
      }
    }

    let mut castling = [0; 16];
    for i in 0..16 {
      castling[i] = rng.random();
    }

    let mut en_passant_file = [0; 8];
    for i in 0..8 {
      en_passant_file[i] = rng.random();
    }

    ZobristKeys { pieces, castling, en_passant_file, side_to_move: rng.random() }
  }
}

/// Returns a reference to the only Zobristkeys instance
pub fn keys() -> &'static ZobristKeys {
    ZOBRIST_KEYS.get_or_init(ZobristKeys::new)
}