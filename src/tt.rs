/*This file implemets a thread safe Transposition Table */
use crate::moves::Move;
use crate::board::ZHash;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TTFlag {
  Exact,
  Alpha, // upper bound
  Beta,  // lower bound
}

#[derive(Clone, Copy)]
pub struct TTEntry {
  pub key: ZHash,
  pub move_best: Option<Move>,
  pub score: i32,
  pub depth: u8,
  pub flag: TTFlag,
}

pub struct TranspositionTable {
  table: Vec<Option<TTEntry>>,
  size: usize,
}

impl TranspositionTable {
  pub fn new(mb_size: usize) -> Self {
    let size = (mb_size * 1024 * 1024) / std::mem::size_of::<Option<TTEntry>>();
    Self { table: vec![None; size], size }
  }

  pub fn probe(&self, key: ZHash) -> Option<TTEntry> {
    let index = (key as usize) % self.size;
    if let Some(entry) = self.table[index] {
      if entry.key == key {
        return Some(entry);
      }
    }
    None
  }

  pub fn store(&mut self, key: ZHash, move_best: Option<Move>, score: i32, depth: u8, flag: TTFlag) {
    let index = (key as usize) % self.size;
    

    if let Some(existing) = self.table[index] {
      if existing.key == key && depth < existing.depth {
        return;
      }
    }
    // Simple replacement scheme: Always replace
    // Better schemes exist (depth-preferred, etc.) will implement this before starting for MVP-3
    self.table[index] = Some(TTEntry {
        key,
        move_best,
        score,
        depth,
        flag,
    });
  }
    
  pub fn clear(&mut self) {
      self.table.fill(None);
  }
}
