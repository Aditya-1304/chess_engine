use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use rand::Rng;
use crate::moves::{BISHOP_PROMOTION_FLAG, KNIGHT_PROMOTION_FLAG, Move, QUEEN_PROMOTION_FLAG, QUIET_MOVE_FLAG, ROOK_PROMOTION_FLAG, new};
use crate::board::ZHash;

const ENTRY_SIZE: usize = 16;

#[derive(Debug)]
struct Entry {
  key: u64,
  move_raw: u16,
  weight: u16,
  learn: u32,
}
pub struct OpeningBook {
  pub file: Option<BufReader<File>>
}

impl OpeningBook {
  pub fn new(path: &str) -> Self {
    let file = File::open(path).ok().map(BufReader::new);
    Self { file }
  }

  pub fn get_move(&mut self, hash: ZHash) -> Option<Move> {
    let reader = self.file.as_mut()?;


    let file_len = reader.get_ref().metadata().ok()?.len();
    let num_entries = file_len / ENTRY_SIZE as u64;

    let mut low = 0;
    let mut high = (file_len / ENTRY_SIZE as u64) - 1;
    let mut found_idx = None;

    while low <= high {
      let mid = (low + high) / 2;
      reader.seek(SeekFrom::Start(mid * ENTRY_SIZE as u64)).ok()?;

      let mut buf_key = [0u8; 8];
      reader.read_exact(&mut buf_key).ok()?;

      let entry_key = u64::from_be_bytes(buf_key[0..8].try_into().unwrap());
      // let entry_move = u16::from_be_bytes(buf[8..10].try_into().unwrap());

      if entry_key == hash {
        found_idx = Some(mid);
        break;
      } else if entry_key < hash {
        low = mid + 1;
      } else {
        if mid == 0 { break; }
        high = mid - 1;
      }
    }

    let idx = found_idx?;

    let mut first_idx = idx;
    while first_idx > 0 {
      reader.seek(SeekFrom::Start((first_idx - 1) * ENTRY_SIZE as u64)).ok()?;
      let mut buf_key = [0u8; 8];
      reader.read_exact(&mut buf_key).ok()?;
      if u64::from_be_bytes(buf_key) == hash {
        first_idx -= 1;
      } else {
        break;
      }
    }

    let mut entries = Vec::new();
    let mut curr_idx = first_idx;

    loop{
      if curr_idx >= num_entries { break; }
      reader.seek(SeekFrom::Start(curr_idx * ENTRY_SIZE as u64)).ok()?;
      let mut buf = [0u8; 16];
      reader.read_exact(&mut buf).ok()?;

      let key = u64::from_be_bytes(buf[0..8].try_into().unwrap());
      if key != hash { break; }

      let move_raw = u16::from_be_bytes(buf[8..10].try_into().unwrap());
      let weight = u16::from_be_bytes(buf[10..12].try_into().unwrap());
      let learn = u32::from_be_bytes(buf[12..16].try_into().unwrap());

      entries.push(Entry { key, move_raw, weight, learn});
      curr_idx += 1;
    }

    let total_weight: u32 = entries.iter().map(|e| e.weight as u32).sum();
    if total_weight == 0 { return None; }

    let mut rng = rand::rng();
    let mut choice = rng.random_range(0..total_weight);

    for entry in entries {
      let w = entry.weight as u32;
      if choice < w {
        return Some(self.polygot_move_to_internal(entry.move_raw));
      }
      choice -= w;
    }
    None

  }

  /// Converts ploygot moves to the engine moves
  fn polygot_move_to_internal(&self, pg_move: u16) -> Move {
    let to = (pg_move & 0x3F) as u8;
    let from = ((pg_move >> 6) & 0x3F) as u8;
    let promo = (pg_move >> 12) & 0x7;
    
    // map polygot promotion codes to engine flags
    let flag = match promo {
      0 => QUIET_MOVE_FLAG,
      1 => KNIGHT_PROMOTION_FLAG,
      2 => BISHOP_PROMOTION_FLAG,
      3 => ROOK_PROMOTION_FLAG,
      4 => QUEEN_PROMOTION_FLAG,
      _ => QUIET_MOVE_FLAG,
    };

    new(from, to, flag)
  }
}