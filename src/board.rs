use std::fmt;

use crate::types::{Bitboard, Color, PieceType, Square};

pub type ZHash = u64;

#[derive(Clone)]
pub struct UndoInfo {

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
    Ok(board)
  }

  pub fn to_fen(&self) -> String {
    String::new()
  }

  pub fn make_move(&mut self, m:crate::types::Move) -> UndoInfo {

    UndoInfo {}
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