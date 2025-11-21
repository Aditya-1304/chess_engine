use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use crate::moves::{Move,new, QUIET_MOVE_FLAG};
use crate::board::ZHash;

const ENTRY_SIZE: usize = 16;

pub struct OpeningBook {
  file: Option<BufReader<File>>
}

impl OpeningBook {
  pub fn new(path: &str) -> Self {
    let file = File::open(path).ok().map(BufReader::new);
    Self { file }
  }

  pub fn get_move(&mut self, hash: ZHash) -> Option<Move> {
    let reader = self.file.as_mut()?;

    // PolyGlot hashes are big-endian. Your engine might need to re-hash 
    // specifically for PolyGlot if your Zobrist keys differ (they likely do).
    // FOR NOW: We assume you will eventually align your Zobrist keys to PolyGlot 
    // or just use this for structure. 
    // *Note:* Most engines use a "PolyGlot Adapter" to calculate the 
    // specific PolyGlot hash just for the book probe.

    let key = hash;

    let file_len = reader.get_ref().metadata().ok()?.len();
    let mut low = 0;
    let mut high = (file_len / ENTRY_SIZE as u64) - 1;

    while low <= high {
      let mid = (low + high) / 2;
      reader.seek(SeekFrom::Start(mid * ENTRY_SIZE as u64)).ok()?;

      let mut buf = [0u8; 16];
      reader.read_exact(&mut buf).ok()?;

      let entry_key = u64::from_be_bytes(buf[0..8].try_into().unwrap());
      let entry_move = u16::from_be_bytes(buf[8..10].try_into().unwrap());

      if entry_key == key {
        return Some(self.polygot_move_to_internal(entry_move));
      } else if entry_key < key {
        low = mid + 1;
      } else {
        if mid == 0 { break; }
        high = mid - 1;
      }
    }
    None
  }

  /// Converts ploygot moves to the engine moves
  fn polygot_move_to_internal(&self, pg_move: u16) -> Move {
    let to = (pg_move & 0x3F) as u8;
    let from = ((pg_move >> 6) & 0x3F) as u8;
    let promo = (pg_move >> 12) & 0x7;

    new(from, to, QUIET_MOVE_FLAG)
  }
}