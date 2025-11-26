use crate::board::Board;
use crate::movegen;
use crate::types::{Color, PieceType, Square};
use pyrrhic_rs::{EngineAdapter, TableBases, WdlProbeResult, DtzProbeValue};
use std::path::Path;
use std::sync::RwLock;

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

    let ep = if let Some(sq) = board.en_passant {
        sq as u32
    } else {
        0
    };

    let turn = board.side_to_move == Color::White;

    match tb.probe_wdl(
        white, black, kings, queens, rooks, bishops, knights, pawns, ep, turn,
    ) {
        Ok(res) => Some(res),
        Err(_) => None,
    }
}

/// Probe DTZ at root and return the best move info (from_sq, to_sq, promo, wdl_score)
pub fn probe_root(board: &Board, tb: &SyzygyTB) -> Option<(u8, u8, u8, i32)> {
    let white = board.occupancy[Color::White as usize];
    let black = board.occupancy[Color::Black as usize];

    let kings = board.pieces[PieceType::King as usize][0] | board.pieces[PieceType::King as usize][1];
    let queens = board.pieces[PieceType::Queen as usize][0] | board.pieces[PieceType::Queen as usize][1];
    let rooks = board.pieces[PieceType::Rook as usize][0] | board.pieces[PieceType::Rook as usize][1];
    let bishops = board.pieces[PieceType::Bishop as usize][0] | board.pieces[PieceType::Bishop as usize][1];
    let knights = board.pieces[PieceType::Knight as usize][0] | board.pieces[PieceType::Knight as usize][1];
    let pawns = board.pieces[PieceType::Pawn as usize][0] | board.pieces[PieceType::Pawn as usize][1];

    let ep = board.en_passant.map(|sq| sq as u32).unwrap_or(0);
    let turn = board.side_to_move == Color::White;
    let rule50 = board.halfmove_clock as u32;

    match tb.probe_root(white, black, kings, queens, rooks, bishops, knights, pawns, rule50, ep, turn) {
        Ok(result) => {
            // Extract from root field
            match result.root {
                DtzProbeValue::DtzResult(dtz_result) => {
                    let from = dtz_result.from_square;
                    let to = dtz_result.to_square;
                    let promo = match dtz_result.promotion {
                        pyrrhic_rs::Piece::Queen => 4,
                        pyrrhic_rs::Piece::Rook => 3,
                        pyrrhic_rs::Piece::Bishop => 2,
                        pyrrhic_rs::Piece::Knight => 1,
                        _ => 0,
                    };
                    let wdl_score = match dtz_result.wdl {
                        WdlProbeResult::Win => 1,
                        WdlProbeResult::CursedWin => 1,
                        WdlProbeResult::Loss => -1,
                        WdlProbeResult::BlessedLoss => -1,
                        WdlProbeResult::Draw => 0,
                    };
                    Some((from, to, promo, wdl_score))
                }
                DtzProbeValue::Checkmate => None, 
                DtzProbeValue::Stalemate => None, 
                DtzProbeValue::Failed => None,
            }
        }
        Err(_) => None,
    }
}