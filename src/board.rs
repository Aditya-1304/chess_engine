use std::fmt;

use crate::{
  movegen,
  moves::{self, Move, MoveList},
  nnue,
  types::{Accumulator, Bitboard, Color, PieceType, Square},
  zobrist,
};

pub type ZHash = u64;

#[derive(Clone, Debug)]
pub struct UndoInfo {
  pub old_castling_rights: u8,
  pub old_en_passant: Option<Square>,
  pub old_halfmove_clock: u8,
  pub captured_piece_type: Option<PieceType>,
  pub old_zobrist_hash: ZHash,
}

#[derive(Clone)]
pub struct Board {
  pub pieces: [[Bitboard; 2]; 6],
  pub occupancy: [Bitboard; 3],
  pub side_to_move: Color,
  pub castling_rights: u8,
  pub en_passant: Option<Square>,
  pub halfmove_clock: u8,
  pub fullmove_number: u32,
  pub zobrist_hash: ZHash,
  pub history: Vec<UndoInfo>,
  pub accumulator: [Accumulator; 2],
}

const WK_CASTLE: u8 = 0b0001;
const WQ_CASTLE: u8 = 0b0010;
const BK_CASTLE: u8 = 0b0100;
const BQ_CASTLE: u8 = 0b1000;

static CASTLE_MASK: [u8; 64] = [
  !WQ_CASTLE,
  0xFF,
  0xFF,
  0xFF,
  !(WK_CASTLE | WQ_CASTLE),
  0xFF,
  0xFF,
  !WK_CASTLE,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  0xFF,
  !BQ_CASTLE,
  0xFF,
  0xFF,
  0xFF,
  !(BK_CASTLE | BQ_CASTLE),
  0xFF,
  0xFF,
  !BK_CASTLE,
];

impl Board {

    pub fn clone_for_search(&self) -> Self {
      Board {
        pieces: self.pieces,
        occupancy: self.occupancy,
        side_to_move: self.side_to_move,
        castling_rights: self.castling_rights,
        en_passant: self.en_passant,
        halfmove_clock: self.halfmove_clock,
        fullmove_number: self.fullmove_number,
        zobrist_hash: self.zobrist_hash,
        history: Vec::with_capacity(128),
        accumulator: self.accumulator,
      }
    }
    pub fn from_fen(fen: &str) -> Result<Board, &'static str> {
        let mut board = Board::default();
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 6 {
          return Err("Invalid FEN: must have 6 fields");
        }

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
                if file > 7 {
                    return Err("Invalid FEN: piece placement");
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
                    _ => return Err("Invalid FEN: Unknown piece"),
                };
                board.add_piece(piece_type, color, square);
                file += 1;
            }
        }

        board.side_to_move = match parts[1] {
            "w" => Color::White,
            "b" => Color::Black,
            _ => return Err("Invalid side"),
        };

        board.castling_rights = 0;
        for ch in parts[2].chars() {
            match ch {
                'K' => board.castling_rights |= 0b0001,
                'Q' => board.castling_rights |= 0b0010,
                'k' => board.castling_rights |= 0b0100,
                'q' => board.castling_rights |= 0b1000,
                _ => {}
            }
        }

        board.en_passant = if parts[3] == "-" {
            None
        } else {
            let chars: Vec<char> = parts[3].chars().collect();
            let f = (chars[0] as u8) - b'a';
            let r = (chars[1] as u8) - b'1';
            Some(r * 8 + f)
        };

        board.halfmove_clock = parts[4].parse().unwrap_or(0);
        board.fullmove_number = parts[5].parse().unwrap_or(1);
        board.zobrist_hash = board.calculate_zobrist_hash();

        // Initialize NNUE
        if nnue::NETWORK.get().is_some() {
            board.accumulator = nnue::refresh_accumulator(&board);
        }

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
        if self.castling_rights & 0b0001 != 0 {
            castling_str.push('K');
        }
        if self.castling_rights & 0b0010 != 0 {
            castling_str.push('Q');
        }
        if self.castling_rights & 0b0100 != 0 {
            castling_str.push('k');
        }
        if self.castling_rights & 0b1000 != 0 {
            castling_str.push('q');
        }
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

    pub fn piece_type_on(&self, sq: Square) -> Option<PieceType> {
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

    fn calculate_zobrist_hash(&self) -> ZHash {
        let keys = zobrist::keys();
        let mut hash: ZHash = 0;
        for pt_idx in 0..6 {
            for c_idx in 0..2 {
                let mut bb = self.pieces[pt_idx][c_idx];
                while bb != 0 {
                    let sq = bb.trailing_zeros() as Square;
                    hash ^= keys.pieces[pt_idx][c_idx][sq as usize];
                    bb &= bb - 1;
                }
            }
        }
        hash ^= keys.castling[self.castling_rights as usize];
        if let Some(sq) = self.en_passant {
            let us = self.side_to_move;
            let our_pawns = self.pieces[PieceType::Pawn as usize][us as usize];
            let them = if us == Color::White {
                Color::Black
            } else {
                Color::White
            };
            let potential_attackers = movegen::pawn_attacks(them, sq);
            if (potential_attackers & our_pawns) != 0 {
                let file = (sq % 8) as usize;
                hash ^= keys.en_passant_file[file];
            }
        }
        if self.side_to_move == Color::White {
            hash ^= keys.side_to_move;
        }
        hash
    }

    pub fn make_move(&mut self, m: Move) -> UndoInfo {
        let keys = zobrist::keys();
        let mut hash = self.zobrist_hash;
        let from = moves::from_sq(m);
        let to = moves::to_sq(m);
        let flag = moves::flag(m);
        let us = self.side_to_move;
        let them = if us == Color::White {
            Color::Black
        } else {
            Color::White
        };
        let moving_piece = self.piece_type_on(from).unwrap();
        let captured = if moves::is_capture(m) {
            if flag == moves::EN_PASSANT_CAPTURE_FLAG {
                Some(PieceType::Pawn)
            } else {
                self.piece_type_on(to)
            }
        } else {
            None
        };

        // Save State
        let undo = UndoInfo {
            old_castling_rights: self.castling_rights,
            old_en_passant: self.en_passant,
            old_halfmove_clock: self.halfmove_clock,
            captured_piece_type: captured,
            old_zobrist_hash: self.zobrist_hash,
        };
        self.history.push(undo.clone());

        // NNUE Incremental Updates
        if nnue::NETWORK.get().is_some() {
            if moving_piece == PieceType::King {
            } else {
                self.apply_nnue_updates(m, moving_piece, captured, us, them, true);
            }
        }

        // Update Board State
        hash ^= keys.side_to_move;
        if let Some(sq) = self.en_passant {
            let capturers = self.pieces[PieceType::Pawn as usize][us as usize];
            if (movegen::pawn_attacks(them, sq) & capturers) != 0 {
                hash ^= keys.en_passant_file[(sq % 8) as usize];
            }
        }
        hash ^= keys.castling[self.castling_rights as usize];

        if let Some(cap_pt) = captured {
            if flag == moves::EN_PASSANT_CAPTURE_FLAG {
                let captured_sq = if us == Color::White { to - 8 } else { to + 8 };
                self.remove_piece(PieceType::Pawn, them, captured_sq);
                hash ^= keys.pieces[PieceType::Pawn as usize][them as usize][captured_sq as usize];
            } else {
                self.remove_piece(cap_pt, them, to);
                hash ^= keys.pieces[cap_pt as usize][them as usize][to as usize];
            }
        }

        self.move_piece(moving_piece, us, from, to);
        hash ^= keys.pieces[moving_piece as usize][us as usize][from as usize];
        hash ^= keys.pieces[moving_piece as usize][us as usize][to as usize];

        if moves::is_promotion(m) {
            let promo = moves::promotion_piece(m);
            self.remove_piece(PieceType::Pawn, us, to);
            self.add_piece(promo, us, to);
            hash ^= keys.pieces[PieceType::Pawn as usize][us as usize][to as usize];
            hash ^= keys.pieces[promo as usize][us as usize][to as usize];
        } else if flag == moves::KING_CASTLE_FLAG {
            let (rf, rt) = if us == Color::White { (7, 5) } else { (63, 61) };
            self.move_piece(PieceType::Rook, us, rf, rt);
            hash ^= keys.pieces[PieceType::Rook as usize][us as usize][rf as usize];
            hash ^= keys.pieces[PieceType::Rook as usize][us as usize][rt as usize];
        } else if flag == moves::QUEEN_CASTLE_FLAG {
            let (rf, rt) = if us == Color::White { (0, 3) } else { (56, 59) };
            self.move_piece(PieceType::Rook, us, rf, rt);
            hash ^= keys.pieces[PieceType::Rook as usize][us as usize][rf as usize];
            hash ^= keys.pieces[PieceType::Rook as usize][us as usize][rt as usize];
        }

        self.en_passant = if flag == moves::DOUBLE_PAWN_PUSH_FLAG {
            let ep_sq = if us == Color::White {
                from + 8
            } else {
                from - 8
            };
            if (movegen::pawn_attacks(them, ep_sq)
                & self.pieces[PieceType::Pawn as usize][them as usize])
                != 0
            {
                hash ^= keys.en_passant_file[(ep_sq % 8) as usize];
            }
            Some(ep_sq)
        } else {
            None
        };

        if moving_piece == PieceType::Pawn || captured.is_some() {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        }
        if us == Color::Black {
            self.fullmove_number += 1;
        }

        self.castling_rights &= CASTLE_MASK[from as usize];
        self.castling_rights &= CASTLE_MASK[to as usize];
        hash ^= keys.castling[self.castling_rights as usize];

        self.side_to_move = them;
        self.zobrist_hash = hash;

        // 4. King Move Refresh (Full refresh if king moved)
        if nnue::NETWORK.get().is_some() && moving_piece == PieceType::King {
            self.accumulator = nnue::refresh_accumulator(self);
        }

        undo
    }

    pub fn unmake_move(&mut self, m: Move, undo: UndoInfo) {
        let _ = self.history.pop();
        self.zobrist_hash = undo.old_zobrist_hash;

        let from = moves::from_sq(m);
        let to = moves::to_sq(m);
        let flag = moves::flag(m);
        let them = self.side_to_move;
        let us = if them == Color::White {
            Color::Black
        } else {
            Color::White
        };

        self.castling_rights = undo.old_castling_rights;
        self.en_passant = undo.old_en_passant;
        self.halfmove_clock = undo.old_halfmove_clock;
        if us == Color::Black {
            self.fullmove_number -= 1;
        }
        self.side_to_move = us;

        let mut moving_piece = self.piece_type_on(to).unwrap();
        if moves::is_promotion(m) {
            self.remove_piece(moving_piece, us, to);
            self.add_piece(PieceType::Pawn, us, to);
            moving_piece = PieceType::Pawn;
        } else if flag == moves::KING_CASTLE_FLAG {
            let (rf, rt) = if us == Color::White { (7, 5) } else { (63, 61) };
            self.move_piece(PieceType::Rook, us, rt, rf);
        } else if flag == moves::QUEEN_CASTLE_FLAG {
            let (rf, rt) = if us == Color::White { (0, 3) } else { (56, 59) };
            self.move_piece(PieceType::Rook, us, rt, rf);
        }

        self.move_piece(moving_piece, us, to, from);

        if let Some(cap_pt) = undo.captured_piece_type {
            if flag == moves::EN_PASSANT_CAPTURE_FLAG {
                let cap_sq = if us == Color::White { to - 8 } else { to + 8 };
                self.add_piece(PieceType::Pawn, them, cap_sq);
            } else {
                self.add_piece(cap_pt, them, to);
            }
        }

        if nnue::NETWORK.get().is_some() {
            if moving_piece == PieceType::King {

                self.accumulator = nnue::refresh_accumulator(self);
            } else {
                self.apply_nnue_updates(m, moving_piece, undo.captured_piece_type, us, them, false);
            }
        }
    }

    #[inline(always)]
    fn apply_nnue_updates(
        &mut self,
        m: Move,
        moving_piece: PieceType,
        captured: Option<PieceType>,
        us: Color,
        them: Color,
        forward: bool,
    ) {

        let wk_sq = self.pieces[PieceType::King as usize][Color::White as usize].trailing_zeros() as u8;
        let bk_sq = self.pieces[PieceType::King as usize][Color::Black as usize].trailing_zeros() as u8;

        let from = moves::from_sq(m);
        let to = moves::to_sq(m);
        let flag = moves::flag(m);

        let mut updates = [(0u8, PieceType::Pawn, Color::White, false); 8];
        let mut count = 0;

        updates[count] = (from, moving_piece, us, false);
        count += 1;

        // Capture handking
        if let Some(cap_pt) = captured {
            if flag == moves::EN_PASSANT_CAPTURE_FLAG {
                let cap_sq = if us == Color::White { to - 8 } else { to + 8 };
                updates[count] = (cap_sq, PieceType::Pawn, them, false);
                count += 1;
            } else {
                updates[count] = (to, cap_pt, them, false);
                count += 1;
            }
        }

        if moves::is_promotion(m) {
            updates[count] = (to, moves::promotion_piece(m), us, true);
            count += 1;
        } else {
            updates[count] = (to, moving_piece, us, true);
            count += 1;
        }

        // Castle handling
        if flag == moves::KING_CASTLE_FLAG {
            let (r_from, r_to) = if us == Color::White { (7, 5) } else { (63, 61) };
            updates[count] = (r_from, PieceType::Rook, us, false);
            count += 1;
            updates[count] = (r_to, PieceType::Rook, us, true);
            count += 1;
        } else if flag == moves::QUEEN_CASTLE_FLAG {
            let (r_from, r_to) = if us == Color::White { (0, 3) } else { (56, 59) };
            updates[count] = (r_from, PieceType::Rook, us, false);
            count += 1;
            updates[count] = (r_to, PieceType::Rook, us, true);
            count += 1;
        }

        for i in 0..count {
            let (sq, pt, color, is_add) = updates[i];
            let final_add = if forward { is_add } else { !is_add };

            // White's accumulator uses white king
            let idx_w = nnue::halfkp_index(wk_sq, sq, pt, color, Color::White);
            nnue::update_feature(&mut self.accumulator[0], idx_w, final_add);

            // Black's accumulator uses black king
            let idx_b = nnue::halfkp_index(bk_sq, sq, pt, color, Color::Black);
            nnue::update_feature(&mut self.accumulator[1], idx_b, final_add);
        }
    }

    pub fn generate_pseudo_legal_moves(&self, list: &mut MoveList) {
        movegen::generate_pseudo_legal_moves(self, list);
    }

    pub fn is_square_attacked(&self, sq: Square, attacker_color: Color) -> bool {
        movegen::is_square_attacked(self, sq, attacker_color)
    }

    pub fn perft(&mut self, depth: u8) -> u64 {
        if depth == 0 {
            return 1;
        }

        let mut nodes = 0;
        let mut move_list = MoveList::new();
        self.generate_pseudo_legal_moves(&mut move_list);

        for &m in move_list.iter() {
            let undo = self.make_move(m);

            let us = if self.side_to_move == Color::White {
                Color::Black
            } else {
                Color::White
            };
            let king_sq =
                self.pieces[PieceType::King as usize][us as usize].trailing_zeros() as Square;

            if !self.is_square_attacked(king_sq, self.side_to_move) {
                nodes += self.perft(depth - 1);
            }
            self.unmake_move(m, undo);
        }
        nodes
    }

    pub fn is_repetition(&self) -> bool {
        let mut count = 0;
        for undo in self.history.iter().rev() {
            if undo.old_halfmove_clock == 0 {
                break;
            }
            if undo.old_zobrist_hash == self.zobrist_hash {
                count += 1;
                if count >= 2 {
                    return true;
                }
            }
        }
        false
    }

    pub fn make_null_move(&mut self) -> Option<Square> {
        let keys = zobrist::keys();
        let old_ep = self.en_passant;

        if let Some(ep) = self.en_passant {
          let them = if self.side_to_move == Color::White {
            Color::Black
          } else {
            Color::White
          };
          let our_pawns = self.pieces[PieceType::Pawn as usize][self.side_to_move as usize];

          if (movegen::pawn_attacks(them, ep) & our_pawns) != 0 {
            self.zobrist_hash ^= keys.en_passant_file[(ep % 8) as usize];
          }   
          self.en_passant = None;
        }

        self.zobrist_hash ^= keys.side_to_move;
        self.side_to_move = if self.side_to_move == Color::White {
            Color::Black
        } else {
            Color::White
        };

        old_ep
    }

    pub fn unmake_null_move(&mut self, old_ep: Option<Square>) {
        let keys = zobrist::keys();

        self.side_to_move = if self.side_to_move == Color::White {
            Color::Black
        } else {
            Color::White
        };

        self.zobrist_hash ^= keys.side_to_move;

        if let Some(ep) = old_ep {

          let them = if self.side_to_move == Color::White {
            Color::Black
          } else {
              Color::White
          };
          let our_pawn = self.pieces[PieceType::Pawn as usize][self.side_to_move as usize];
          if (movegen::pawn_attacks(them, ep) & our_pawn) != 0 {
            self.zobrist_hash ^= keys.en_passant_file[(ep % 8) as usize];
          }
            self.en_passant = Some(ep);
        }
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
                    if (self.pieces[pt_idx][Color::White as usize] & bit) != 0 {
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
            if self.castling_rights & 0b1 > 0 {
                "K"
            } else {
                ""
            },
            if self.castling_rights & 0b10 > 0 {
                "Q"
            } else {
                ""
            },
            if self.castling_rights & 0b100 > 0 {
                "k"
            } else {
                ""
            },
            if self.castling_rights & 0b1000 > 0 {
                "q"
            } else {
                ""
            }
        )?;
        Ok(())
    }
}

impl Default for Board {
    fn default() -> Self {
        Board {
            pieces: [[0; 2]; 6],
            occupancy: [0; 3],
            side_to_move: Color::White,
            castling_rights: 0,
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            zobrist_hash: 0,
            history: Vec::new(),
            accumulator: [Accumulator::default(); 2],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movegen;

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

    #[test]
    fn make_unmake_move() {
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let mut board = Board::from_fen(fen).unwrap();
        let original_fen = board.to_fen();
        let original_hash = board.zobrist_hash;

        let m = moves::new(36, 26, moves::QUIET_MOVE_FLAG);
        let undo = board.make_move(m);
        board.unmake_move(m, undo);

        assert_eq!(original_fen, board.to_fen());
        assert_eq!(original_hash, board.zobrist_hash);
    }

    #[test]
    fn perft_startpos() {
        movegen::init();
        let mut board =
            Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        assert_eq!(board.perft(1), 20);
        assert_eq!(board.perft(2), 400);
    }

    #[test]
    fn perft_kiwi() {
        movegen::init();
        let mut board =
            Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1")
                .unwrap();
        assert_eq!(board.perft(1), 48);
        assert_eq!(board.perft(2), 2039);
        assert_eq!(board.perft(3), 97862);
    }

    #[test]
    fn perft_position_3() {
        movegen::init();
        let mut board = Board::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap();
        assert_eq!(board.perft(1), 14);
        assert_eq!(board.perft(2), 191);
        assert_eq!(board.perft(3), 2812);
    }
}
