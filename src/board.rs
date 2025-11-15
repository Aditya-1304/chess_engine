use std::fmt;

use crate::{moves, types::{Bitboard, Color, PieceType, Square}};

pub type ZHash = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UndoInfo {
  pub old_castling_rights: u8,
  pub old_en_passant: Option<Square>,
  pub old_halfmove_clock: u8,
  pub captured_piece_type: Option<PieceType>,
}

#[derive(Clone)]
pub struct Board {
  pub pieces: [[Bitboard; 2]; 6], // pieces [PieceType][Color]
  pub occupancy: [Bitboard; 3], // occupancy: [0] = white, [1] = black, [2] = all
  pub side_to_move: Color,
  pub castling_rights: u8, // 4 bits: WhiteKingsSide, WhiteQueenSide, BlackKingSide, BlackQueenSide
  pub en_passant: Option<Square>,
  pub halfmove_clock: u8,
  pub fullmove_number: u32,
  pub zobrist_hash: ZHash,
  pub history: Vec<UndoInfo>, // stack for undoing the moves
}


const WK_CASTLE: u8 = 0b0001;
const WQ_CASTLE: u8 = 0b0010;
const BK_CASTLE: u8 = 0b0100;
const BQ_CASTLE: u8 = 0b1000;

impl Board {

  pub fn from_fen(fen: &str) -> Result<Board, &'static str> {
    let mut board = Board::default();
    let parts: Vec<&str> = fen.split_whitespace().collect();
    if parts.len() != 6 {
      return Err("Invalid FEN: must have 6 fields");
    }

    // setting the correct piece placement on the board
    let piece_placement = parts[0];
    let mut rank = 7;
    let mut file = 0;
    for ch in piece_placement.chars() {
      if ch == '/' {
        rank -= 1;
        file = 0;
      } else if let Some(digit) = ch.to_digit(10) {
          file += digit as u8;
      } else {
        if rank < 0 || file > 7 {
          return Err("Invalid FEN: piece placement format error");
        }
        let square = rank * 8 + file;
        let color = if ch.is_uppercase() {
          Color::White
        } else {
          Color::Black
        };
        let piece_type = match ch.to_ascii_lowercase() {
          'p' => PieceType::Pawn,
          'n' => PieceType::Knight,
          'b' => PieceType::Bishop,
          'r' => PieceType::Rook,
          'q' => PieceType::Queen,
          'k' => PieceType::King,
          _ => return Err("Invalid FEN: Unknown piece character"),
        };
        board.pieces[piece_type as usize][color as usize] |= 1 << square;
        file += 1;

      }
    }

    // occupancy calculation (basic for calculation for now)
    for pt_idx in 0..6 {
      board.occupancy[Color::White as usize] |= board.pieces[pt_idx][Color::White as usize];
      board.occupancy[Color::Black as usize] |= board.pieces[pt_idx][Color::Black as usize];
    }
    board.occupancy[2] = 
      board.occupancy[Color::White as usize] | board.occupancy[Color::Black as usize];

    // which side to move??
    board.side_to_move = match parts[1] {
        "w" => Color::White,
        "b" => Color::Black,
        _ => return Err("Invalid FEN: Invalid side to move"),
    };

    // Castling rights
    board.castling_rights = 0;
    for ch in parts[2].chars() {
      match ch {
        'K' => board.castling_rights |= 0b0001,
        'Q' => board.castling_rights |= 0b0010,
        'k' => board.castling_rights |= 0b0100,
        'q' => board.castling_rights |= 0b1000,
        '-' => {}
        _ => return Err("Invalid FEN: invalid castling rights"),
      }
    }

    // En passant check 
    board.en_passant = if parts[3] == "-" {
      None
    } else {
      let chars: Vec<char> = parts[3].chars().collect();
      if chars.len() != 2 {
        return Err("Invalid FEN: invalid en passant square");
      }
      let file = (chars[0] as u8) - b'a';
      let rank = (chars[1] as u8) - b'1';
      if file > 7 || rank > 7 {
        return Err("Invaid FEN: invalid en passent sqaure");
      }
      Some(rank * 8 + file)
    };

    // halfmove 
    board.halfmove_clock = parts[4]
      .parse()
      .map_err(|_| "Invalid FEN: invalid halfmove clock")?;

    board.fullmove_number = parts[5]
      .parse()
      .map_err(|_| "Invalid FEN: invalid fullmove number")?;

    Ok(board)
  }

  pub fn to_fen(&self) -> String {
    let mut fen = String::with_capacity(90);

    for rank in (0..8).rev() {
      let mut empty_squares = 0;
      for file in 0..8 {
        let square = rank * 8 + file;
        let bit = 1 << square;
        let mut piece_char = None;

        for pt_idx in 0..6 {
          if (self.pieces[pt_idx][Color::White as usize] & bit) != 0 {
            piece_char = Some(match pt_idx.into() {
              PieceType::Pawn => 'P',
              PieceType::Knight => 'N',
              PieceType::Bishop => 'B',
              PieceType::Rook => 'R',
              PieceType::Queen => 'Q',
              PieceType::King => 'K',
            });
            break;
          }
          if (self.pieces[pt_idx][Color::Black as usize] & bit) != 0 {
            piece_char = Some(match pt_idx.into() {
              PieceType::Pawn => 'p',
              PieceType::Knight => 'n',
              PieceType::Bishop => 'b',
              PieceType::Rook => 'r',
              PieceType::Queen => 'q',
              PieceType::King => 'k',
            });
            break;
          }
        }
        if let Some(pc) = piece_char {
          if empty_squares > 0 {
            fen.push_str(&empty_squares.to_string());
            empty_squares = 0;
          }
          fen.push(pc);
        } else {
          empty_squares += 1;
        }
      }
      if empty_squares > 0 {
        fen.push_str(&empty_squares.to_string());
      }
      if rank > 0 {
        fen.push('/');
      }
    }

    fen.push(' ');
    fen.push(match self.side_to_move {
      Color::White => 'w',
      Color::Black => 'b',
    });

    fen.push(' ');
    let mut castling_str = String::new();
    if self.castling_rights & 0b0001 != 0 { castling_str.push('K'); }
    if self.castling_rights & 0b0010 != 0 { castling_str.push('Q'); }
    if self.castling_rights & 0b0100 != 0 { castling_str.push('k'); }
    if self.castling_rights & 0b1000 != 0 { castling_str.push('q'); }
    if castling_str.is_empty() {
      fen.push('-');
    } else {
      fen.push_str(&castling_str);
    }

    fen.push(' ');
    if let Some(sq) = self.en_passant {
      let file = (sq % 8) as u8 + b'a';
      let rank = (sq / 8) as u8 + b'1';
      fen.push(file as char);
      fen.push(rank as char);
    } else {
      fen.push('-');
    }

    fen.push(' ');
    fen.push_str(&self.halfmove_clock.to_string());

    fen.push(' ');
    fen.push_str(&self.fullmove_number.to_string());

    fen
  }

  fn piece_type_on(&self, sq: Square) -> Option<PieceType> {
    let bit = 1 << sq;
    for pt_idx in 0..6 {
      if (self.pieces[pt_idx][0] | self.pieces[pt_idx][1]) & bit != 0 {
        return Some(PieceType::from(pt_idx));
      }
    }
    None
  }

  fn move_piece(&mut self, pt: PieceType, c: Color, from: Square, to: Square) {
    let from_to_bb = (1 << from) | (1 << to);
    self.pieces[pt as usize][c as usize] ^= from_to_bb;
    self.occupancy[c as usize] ^= from_to_bb;
    self.occupancy[2] ^= from_to_bb;
  }

  fn add_piece(&mut self, pt: PieceType, c: Color, sq: Square) {
    let bit = 1 << sq;
    self.pieces[pt as usize][c as usize] |= bit;
    self.occupancy[c as usize] |= bit;
    self.occupancy[2] |= bit;
  }

  fn remove_piece(&mut self, pt: PieceType, c: Color, sq: Square) {
    let bit = 1 << sq;
    self.pieces[pt as usize][c as usize] &= !bit;
    self.occupancy[c as usize] &= !bit;
    self.occupancy[2] &= !bit;
  }

  pub fn make_move(&mut self, m:crate::types::Move) -> UndoInfo {
    let from = moves::from_sq(m);
    let to = moves::to_sq(m);
    let flag = moves::flag(m);
    let us = self.side_to_move;
    let them = if us == Color::White { Color::Black } else { Color::White };

    let undo = UndoInfo {
      old_castling_rights: self.castling_rights,
      old_en_passant: self.en_passant,
      old_halfmove_clock: self.halfmove_clock,
      captured_piece_type: self.piece_type_on(to),
    };

    let moving_piece = self.piece_type_on(from).unwrap();

    /// captures handling
    if moves::is_capture(m) {
      if flag == moves::EN_PASSANT_CAPTURE_FLAG {
        let captured_sq = if us == Color::White { to - 8 } else { to + 8 };
        self.remove_piece(PieceType::Pawn, them, captured_sq);
      }else {
        let captured_piece = undo.captured_piece_type.unwrap();
        self.remove_piece(captured_piece, them, to);
      }
    }

    self.move_piece(moving_piece, us, from, to);

    if moves::is_promotion(m) {
      let promo_piece = moves::promotion_piece(m);
      self.remove_piece(PieceType::Pawn, us, to);
      self.add_piece(promo_piece, us, to);
    } else if flag == moves::KING_CASTLE_FLAG {
        let (rook_from, rook_to) = if us == Color::White { (7,5) } else { (63, 61)};
        self.move_piece(PieceType::Rook, us, rook_from, rook_to);
    } else if flag == moves::QUEEN_CASTLE_FLAG {
      let (rook_from, rook_to) = if us == Color::White { (0, 3) } else { (56, 59) };
      self.move_piece(PieceType::Rook, us, rook_from, rook_to);
    }

    self.en_passant = if flag == moves::DOUBLE_PAWN_PUSH_FLAG {
      Some(if us == Color::White {from + 8} else { from - 8 })
    } else {
      None
    };

    if moving_piece == PieceType::Pawn || moves::is_capture(m) {
      self.halfmove_clock = 0;
    } else {
      self.halfmove_clock += 1;
    }

    if us == Color::Black {
      self.fullmove_number += 1;
    }

    self.castling_rights &= !( (1 << from) | (1 << to) ).trailing_zeros() as u8;


    self.side_to_move = them;

    undo
  }

  pub fn unmake_move(&mut self, undo: UndoInfo) {

  }
}

impl fmt::Display for Board {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      let mut output = String::new();
      for rank in (0..8).rev() {
        output.push_str(&format!("{} ", rank + 1));
        for file in 0..8 {
          let square = rank * 8 + file;
          let bit = 1 << square;
          let mut piece_char = '.';

          for pt_idx in 0..6 {
            if (self.pieces[pt_idx][Color::White as usize] & bit ) != 0 {
              piece_char = match pt_idx.into() {
                PieceType::Pawn => 'P',
                PieceType::Knight => 'N',
                PieceType::Bishop => 'B',
                PieceType::Rook => 'R',
                PieceType::Queen => 'Q',
                PieceType::King => 'K',
              };
              break;
            }

            if (self.pieces[pt_idx][Color::Black as usize] & bit) != 0 {
              piece_char = match pt_idx.into() {
                PieceType::Pawn => 'p',
                PieceType::Knight => 'n',
                PieceType::Bishop => 'b',
                PieceType::Rook => 'r',
                PieceType::Queen => 'q',
                PieceType::King => 'k',
              };
              break;
            }
          }
          output.push_str(&format!("{} ", piece_char));
        }
        output.push('\n');
      }
      output.push_str("  a b c d e f g h");
      writeln!(f, "{}", output)?;
      writeln!(f, "Side to move: {:?}", self.side_to_move)?;
      writeln!(
        f,
        "Castling: {}{}{}{}",
        if self.castling_rights & 0b1 > 0 { "K" } else { "" },
        if self.castling_rights & 0b10 > 0 { "Q" } else { "" },
        if self.castling_rights & 0b100 > 0 { "k" } else { "" },
        if self.castling_rights & 0b1000 > 0 { "q" } else { "" }
      )?;
      Ok(())
  }
}

impl Default for Board {
  fn default() -> Self {
    Board { 
      pieces: [[0;2]; 6], 
      occupancy: [0; 3], 
      side_to_move: Color::White, 
      castling_rights: 0, 
      en_passant: None, 
      halfmove_clock: 0, 
      fullmove_number: 1, 
      zobrist_hash: 0, 
      history: Vec::new(),
    }
  }
}


#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fen_round_trip() {
    let fens = [
      "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
      "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
      "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
      "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    ];
    for fen_str in fens.iter() {
      let board = Board::from_fen(fen_str).expect("Failed to parse FEN");
      assert_eq!(board.to_fen(), *fen_str);
    }
  }
}