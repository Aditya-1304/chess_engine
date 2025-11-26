use crate::board::Board;
use crate::movegen;
use crate::types::{Color, PieceType, Square};
use pyrrhic_rs::{DtzProbeResult, EngineAdapter, TableBases, WdlProbeResult};
use std::path::Path;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct SyzygyAdapter;

impl EngineAdapter for SyzygyAdapter {
    fn pawn_attacks(color: pyrrhic_rs::Color, square: u64) -> u64 {
        let c = match color {
            pyrrhic_rs::Color::White => Color::White,
            pyrrhic_rs::Color::Black => Color::Black,
        };
        movegen::pawn_attacks(c, square as Square)
    }

    fn knight_attacks(square: u64) -> u64 {
        movegen::knight_attacks(square as Square)
    }

    fn bishop_attacks(square: u64, occupied: u64) -> u64 {
        movegen::get_bishop_attacks(square as Square, occupied)
    }

    fn rook_attacks(square: u64, occupied: u64) -> u64 {
        movegen::get_rook_attacks(square as Square, occupied)
    }

    fn queen_attacks(square: u64, occupied: u64) -> u64 {
        movegen::get_bishop_attacks(square as Square, occupied)
            | movegen::get_rook_attacks(square as Square, occupied)
    }

    fn king_attacks(square: u64) -> u64 {
        movegen::king_attacks(square as Square)
    }
}

pub type SyzygyTB = TableBases<SyzygyAdapter>;

// Global storage for TableBases
pub static SYZYGY_TB: RwLock<Option<SyzygyTB>> = RwLock::new(None);

pub fn init_global_syzygy(path: &str) {
    match TableBases::<SyzygyAdapter>::new(path) {
        Ok(tb) => {
            println!("info string Syzygy tablebases found at: {}", path);
            println!("info string Syzygy max pieces: {}", tb.max_pieces());
            let mut lock = SYZYGY_TB.write().unwrap();
            *lock = Some(tb);
        }
        Err(e) => {
            println!("info string Syzygy init failed: {:?}", e);
        }
    }
}

pub fn auto_load() {
  if Path::new("syzygy").exists() {
    init_global_syzygy("syzygy");
    return;
  }

  if let Ok(path) = std::env::var("SYZYGY_PATH") {
    if Path::new(&path).exists() {
        init_global_syzygy(&path);
    }
  }
}


pub fn get_global_syzygy() -> Option<SyzygyTB> {
    let lock = SYZYGY_TB.read().unwrap();
    lock.clone()
}

pub fn probe_wdl(board: &Board, tb: &SyzygyTB) -> Option<WdlProbeResult> {
    let white = board.occupancy[Color::White as usize];
    let black = board.occupancy[Color::Black as usize];

    let kings =
        board.pieces[PieceType::King as usize][0] | board.pieces[PieceType::King as usize][1];
    let queens =
        board.pieces[PieceType::Queen as usize][0] | board.pieces[PieceType::Queen as usize][1];
    let rooks =
        board.pieces[PieceType::Rook as usize][0] | board.pieces[PieceType::Rook as usize][1];
    let bishops =
        board.pieces[PieceType::Bishop as usize][0] | board.pieces[PieceType::Bishop as usize][1];
    let knights =
        board.pieces[PieceType::Knight as usize][0] | board.pieces[PieceType::Knight as usize][1];
    let pawns =
        board.pieces[PieceType::Pawn as usize][0] | board.pieces[PieceType::Pawn as usize][1];

    // pyrrhic-rs expects ep as u32 (square index) or 0 if none.
    // Since 0 is A1 and A1 is not a valid EP square, 0 is safe for None.
    let ep = if let Some(sq) = board.en_passant {
        sq as u32
    } else {
        0
    };

    // pyrrhic-rs: White=1, Black=0.
    // engine: White=0, Black=1.
    // We need to pass 'turn' as bool. usually true=White.
    let turn = board.side_to_move == Color::White;

    match tb.probe_wdl(
        white, black, kings, queens, rooks, bishops, knights, pawns, ep, turn,
    ) {
        Ok(res) => Some(res),
        Err(_) => None,
    }
}

pub fn probe_dtz(board: &Board, tb: &SyzygyTB) -> Option<DtzProbeResult> {
    let white = board.occupancy[Color::White as usize];
    let black = board.occupancy[Color::Black as usize];
    
    let kings = board.pieces[PieceType::King as usize][0] | board.pieces[PieceType::King as usize][1];
    let queens = board.pieces[PieceType::Queen as usize][0] | board.pieces[PieceType::Queen as usize][1];
    let rooks = board.pieces[PieceType::Rook as usize][0] | board.pieces[PieceType::Rook as usize][1];
    let bishops = board.pieces[PieceType::Bishop as usize][0] | board.pieces[PieceType::Bishop as usize][1];
    let knights = board.pieces[PieceType::Knight as usize][0] | board.pieces[PieceType::Knight as usize][1];
    let pawns = board.pieces[PieceType::Pawn as usize][0] | board.pieces[PieceType::Pawn as usize][1];
    
    let ep = if let Some(sq) = board.en_passant { sq as u32 } else { 0 };
    let turn = board.side_to_move == Color::White;
    let rule50 = board.halfmove_clock as u32;

    match tb.probe_root(white, black, kings, queens, rooks, bishops, knights, pawns, rule50, ep, turn) {
        Ok(res) => Some(res),
        Err(_) => None,
    }
}