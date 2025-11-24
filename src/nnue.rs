// src/nnue.rs
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::sync::OnceLock;

use crate::board::Board;
use crate::types::{Accumulator, Color, PieceType, Square};

const INPUT_SIZE: usize = 41024;
const LAYER1_SIZE: usize = 256;
const QA: i32 = 255;
const QB: i32 = 64;
const Q_OUTPUT: i32 = 16;

pub static NETWORK: OnceLock<Network> = OnceLock::new();

pub struct Network {
    pub feature_weights: Vec<i16>, // Made public for direct access if needed
    pub feature_biases: Vec<i16>,
    l2_weights: Vec<i8>,
    l2_biases: Vec<i32>,
    l3_weights: Vec<i8>,
    l3_biases: Vec<i32>,
    output_weights: Vec<i8>,
    output_bias: i32,
}

impl Network {
    pub fn load(path: &str) -> io::Result<Self> {
        let f = File::open(path)?;
        let mut reader = BufReader::new(f);
        
        let feature_biases = read_i16_vec(&mut reader, LAYER1_SIZE)?;
        let feature_weights = read_i16_vec(&mut reader, INPUT_SIZE * LAYER1_SIZE)?;
        let l2_weights = read_i8_vec(&mut reader, 512 * 32)?;
        let l2_biases = read_i32_vec(&mut reader, 32)?;
        let l3_weights = read_i8_vec(&mut reader, 32 * 32)?;
        let l3_biases = read_i32_vec(&mut reader, 32)?;
        let output_weights = read_i8_vec(&mut reader, 32)?;
        let output_bias = read_i32_vec(&mut reader, 1)?[0];

        Ok(Self {
            feature_weights, feature_biases, l2_weights, l2_biases,
            l3_weights, l3_biases, output_weights, output_bias
        })
    }
}

// --- INCREMENTAL HELPERS ---

// Helper to update a single feature in the accumulator (Add/Subtract)
pub fn update_feature(acc: &mut Accumulator, index: usize, add: bool) {
    let net = NETWORK.get().expect("NNUE not initialized");
    let offset = index * 256;
    if add {
        for i in 0..256 {
            acc.values[i] = acc.values[i].wrapping_add(net.feature_weights[offset + i]);
        }
    } else {
        for i in 0..256 {
            acc.values[i] = acc.values[i].wrapping_sub(net.feature_weights[offset + i]);
        }
    }
}

pub fn refresh_accumulator(board: &Board) -> [Accumulator; 2] {
    let net = NETWORK.get().expect("NNUE not initialized");
    let mut accs = [Accumulator::default(); 2];

    accs[0].values.copy_from_slice(&net.feature_biases);
    accs[1].values.copy_from_slice(&net.feature_biases);

    for sq in 0..64 {
        if let Some(pt) = board.piece_type_on(sq as u8) {

            if pt == PieceType::King {
              continue;
            }
            
            let color = if (board.occupancy[0] & (1 << sq)) != 0 { Color::White } else { Color::Black };

            let w_king = board.pieces[PieceType::King as usize][Color::White as usize].trailing_zeros() as u8;
            let w_idx = halfkp_index(w_king, sq as u8, pt, color, Color::White);
            update_feature(&mut accs[0], w_idx, true);

            let b_king = board.pieces[PieceType::King as usize][Color::Black as usize].trailing_zeros() as u8;
            let b_idx = halfkp_index(b_king, sq as u8, pt, color, Color::Black);
            update_feature(&mut accs[1], b_idx, true);
        }
    }
    accs
}

// --- EVALUATION LOGIC ---

fn crelu(x: i16) -> i32 {
    x.clamp(0, 127) as i32
}

pub fn evaluate(board: &Board) -> i32 {
    let net = match NETWORK.get() {
        Some(n) => n,
        None => return 0,
    };

    // Use the accumulator attached to the board (Fast!)
    // If you haven't updated board.rs yet, this will use the empty default,
    // so ensure you do Step 2 immediately after.
    let (white_acc, black_acc) = (&board.accumulator[0], &board.accumulator[1]);

    let (us, them) = if board.side_to_move == Color::White {
        (white_acc, black_acc)
    } else {
        (black_acc, white_acc)
    };

    // Layer 2
    let mut l2_out = [0i32; 32];
    for i in 0..32 {
        let mut sum = net.l2_biases[i];
        // US (First 256)
        for j in 0..256 {
            sum += crelu(us.values[j]) * (net.l2_weights[i * 512 + j] as i32);
        }
        // THEM (Second 256) - BUG FIX: This was using 'us.values' before!
        for j in 0..256 {
            sum += crelu(them.values[j]) * (net.l2_weights[i * 512 + 256 + j] as i32);
        }
        l2_out[i] = sum;
    }

    // Layer 3
    let mut l3_out = [0i32; 32];
    for i in 0..32 {
        let mut sum = net.l3_biases[i];
        for j in 0..32 {
            let input_val = l2_out[j].clamp(0, 127);
            sum += input_val * (net.l3_weights[i * 32 + j] as i32);
        }
        l3_out[i] = sum;
    }

    // Output
    let mut sum = net.output_bias;
    for j in 0..32 {
        let input_val = l3_out[j].clamp(0, 127);
        sum += input_val * (net.output_weights[j] as i32);
    }

    sum * 100 / (QA * QB / Q_OUTPUT)
}

// BUG FIX: Added Perspective Flipping for Black
pub fn halfkp_index(king_sq: Square, piece_sq: Square, pt: PieceType, pc: Color, k_color: Color) -> usize {
    let (k_sq, p_sq) = if k_color == Color::White {
        (king_sq, piece_sq)
    } else {
        (king_sq ^ 56, piece_sq ^ 56) // Vertical Flip for Black
    };

    let p_idx = match pt {
        PieceType::Pawn => 0,
        PieceType::Knight => 1,
        PieceType::Bishop => 2,
        PieceType::Rook => 3,
        PieceType::Queen => 4,
        PieceType::King => 5,
    };
    
    // HalfKP Logic:
    // If the piece is same color as King => 0..5
    // If the piece is opponent color => 6..11
    let p_type_offset = if pc == k_color { p_idx } else { p_idx + 5 };

    (k_sq as usize * 640) + (p_type_offset * 64) + (p_sq as usize)
}

// Helpers... (Read functions are same as before)
fn read_i16_vec<R: Read>(reader: &mut R, len: usize) -> io::Result<Vec<i16>> {
    let mut buffer = vec![0u8; len * 2];
    reader.read_exact(&mut buffer)?;
    let mut out = Vec::with_capacity(len);
    for chunk in buffer.chunks_exact(2) {
        out.push(i16::from_le_bytes([chunk[0], chunk[1]]));
    }
    Ok(out)
}
fn read_i32_vec<R: Read>(reader: &mut R, len: usize) -> io::Result<Vec<i32>> {
    let mut buffer = vec![0u8; len * 4];
    reader.read_exact(&mut buffer)?;
    let mut out = Vec::with_capacity(len);
    for chunk in buffer.chunks_exact(4) {
        out.push(i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(out)
}
fn read_i8_vec<R: Read>(reader: &mut R, len: usize) -> io::Result<Vec<i8>> {
    let mut buffer = vec![0u8; len];
    reader.read_exact(&mut buffer)?;
    Ok(buffer.iter().map(|&b| b as i8).collect())
}