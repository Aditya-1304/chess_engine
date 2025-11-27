use std::fs::File;
use std::io::{self, BufReader, Read, Seek};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::board::Board;
use crate::types::{Accumulator, Color, PieceType, Square};

static USE_AVX2: AtomicBool = AtomicBool::new(false);
static CPU_FEATURES_CHECKED: AtomicBool = AtomicBool::new(false);
static NNUE_ENABLED: AtomicBool = AtomicBool::new(false);
static mut NETWORK_PTR: *const Network = std::ptr::null();
static mut CACHED_USE_AVX2: bool = false;
static mut CACHED_NNUE_ENABLED: bool = false;

// HalfKP: 64 king squares * (64 squares * 10 piece types + 1) = 64 * 641 = 41024
const INPUT_SIZE: usize = 41024;
const HALF_DIMENSIONS: usize = 256;
const L2_SIZE: usize = 32;
const L3_SIZE: usize = 32;

// Quantization constants from Stockfish
const FV_SCALE: i32 = 16;
const WEIGHT_SCALE_BITS: i32 = 6;
const PS_W_PAWN: usize = 0;
const PS_B_PAWN: usize = 1 * 64;
const PS_W_KNIGHT: usize = 2 * 64;
const PS_B_KNIGHT: usize = 3 * 64;
const PS_W_BISHOP: usize = 4 * 64;
const PS_B_BISHOP: usize = 5 * 64;
const PS_W_ROOK: usize = 6 * 64;
const PS_B_ROOK: usize = 7 * 64;
const PS_W_QUEEN: usize = 8 * 64;
const PS_B_QUEEN: usize = 9 * 64;
const PS_END: usize = 10 * 64;

const KING_BUCKET_SIZE: usize = PS_END + 1;

pub static NETWORK: OnceLock<Network> = OnceLock::new();

#[repr(C, align(64))]
pub struct Network {
    pub ft_biases: Vec<i16>,        // HALF_DIMENSIONS
    pub ft_weights: Vec<i16>,       // INPUT_SIZE * HALF_DIMENSIONS
    pub l1_biases: Vec<i32>,        // L2_SIZE
    pub l1_weights: Vec<i8>,        // 512 * L2_SIZE
    pub l2_biases: Vec<i32>,        // L3_SIZE
    pub l2_weights: Vec<i8>,        // L2_SIZE * L3_SIZE
    pub l3_bias: i32,               // 1
    pub l3_weights: Vec<i8>,        // L3_SIZE
}

impl Network {
    pub fn load(path: &str) -> io::Result<Self> {
        let f = File::open(path)?;
        let mut reader = BufReader::new(f);

        let metadata = std::fs::metadata(path)?;
        let file_len = metadata.len() as usize;

        let mut version = [0u8; 4];
        reader.read_exact(&mut version)?;
        let version_num = u32::from_le_bytes(version);
        println!("info string NNUE version: 0x{:08X}", version_num);

        let mut hash = [0u8; 4];
        reader.read_exact(&mut hash)?;
        let hash_num = u32::from_le_bytes(hash);
        println!("info string NNUE hash: 0x{:08X}", hash_num);

        let mut desc_size_buf = [0u8; 4];
        reader.read_exact(&mut desc_size_buf)?;
        let desc_size = u32::from_le_bytes(desc_size_buf) as usize;
        
        let mut desc = vec![0u8; desc_size];
        reader.read_exact(&mut desc)?;
        let desc_str = String::from_utf8_lossy(&desc);
        println!("info string NNUE arch: {}", desc_str.trim_end_matches('\0'));

        let mut ft_hash = [0u8; 4];
        reader.read_exact(&mut ft_hash)?;
        println!("info string FT hash: 0x{:08X}", u32::from_le_bytes(ft_hash));

        let ft_biases = read_i16_vec(&mut reader, HALF_DIMENSIONS)?;
        println!("info string FT biases[0..8]: {:?}", &ft_biases[0..8]);

        let ft_weights = read_i16_vec(&mut reader, INPUT_SIZE * HALF_DIMENSIONS)?;
        println!("info string FT weights: {} values loaded", ft_weights.len());

        let mut net_hash = [0u8; 4];
        reader.read_exact(&mut net_hash)?;
        println!("info string Network hash: 0x{:08X}", u32::from_le_bytes(net_hash));

        let l1_biases = read_i32_vec(&mut reader, L2_SIZE)?;
        println!("info string L1 biases[0..8]: {:?}", &l1_biases[0..8.min(L2_SIZE)]);

        let l1_weights_raw = read_i8_vec(&mut reader, 512 * L2_SIZE)?;
        println!("info string L1 weights: {} values", l1_weights_raw.len());

        let l2_biases = read_i32_vec(&mut reader, L3_SIZE)?;
        let l2_weights_raw = read_i8_vec(&mut reader, L2_SIZE * L3_SIZE)?;

        let l3_bias = read_i32_vec(&mut reader, 1)?[0];
        let l3_weights = read_i8_vec(&mut reader, L3_SIZE)?;
        
        println!("info string L3 bias: {}", l3_bias);
        println!("info string L3 weights[0..8]: {:?}", &l3_weights[0..8.min(L3_SIZE)]);

        let pos = reader.stream_position()? as usize;
        println!("info string Read {} of {} bytes", pos, file_len);

        Ok(Self {
            ft_biases,
            ft_weights,
            l1_biases,
            l1_weights: l1_weights_raw,
            l2_biases,
            l2_weights: l2_weights_raw,
            l3_bias,
            l3_weights,
        })
    }
}

#[inline(always)]
pub fn is_enabled() -> bool {
    unsafe { CACHED_NNUE_ENABLED }
}

pub fn init_cpu_features() {
    if let Some(net) = NETWORK.get() {
        NNUE_ENABLED.store(true, Ordering::Relaxed);
        unsafe {
            NETWORK_PTR = net as *const Network;
            CACHED_NNUE_ENABLED = true;
        }
    }
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            USE_AVX2.store(true, Ordering::Relaxed);
            unsafe { CACHED_USE_AVX2 = true }
        }
    }
    CPU_FEATURES_CHECKED.store(true, Ordering::Relaxed);
}

#[inline(always)]
unsafe fn get_network() -> &'static Network {
    unsafe {
        &*NETWORK_PTR
    }
}

#[inline(always)]
fn use_avx2() -> bool {
    unsafe { CACHED_USE_AVX2 }
}

/// Get the piece-square index base for HalfKP
#[inline]
fn ps_index(pt: PieceType, color: Color) -> usize {
    match (pt, color) {
        (PieceType::Pawn, Color::White) => PS_W_PAWN,
        (PieceType::Pawn, Color::Black) => PS_B_PAWN,
        (PieceType::Knight, Color::White) => PS_W_KNIGHT,
        (PieceType::Knight, Color::Black) => PS_B_KNIGHT,
        (PieceType::Bishop, Color::White) => PS_W_BISHOP,
        (PieceType::Bishop, Color::Black) => PS_B_BISHOP,
        (PieceType::Rook, Color::White) => PS_W_ROOK,
        (PieceType::Rook, Color::Black) => PS_B_ROOK,
        (PieceType::Queen, Color::White) => PS_W_QUEEN,
        (PieceType::Queen, Color::Black) => PS_B_QUEEN,
        (PieceType::King, _) => usize::MAX, // Kings are not features
    }
}

/// Orient a square for the given perspective
/// White perspective: square as-is
/// Black perspective: rotate 180 degrees (sq ^ 63)
#[inline]
fn orient(perspective: Color, sq: Square) -> usize {
    if perspective == Color::White {
        sq as usize
    } else {
        (sq ^ 63) as usize
    }
}

/// Calculate HalfKP feature index
/// Index formula = 1 + orient(ksq) * PS_END + ps_index(piece) + orient(sq)
/// The +1 is for BONA_PIECE_ZERO at index 0
pub fn make_index(perspective: Color, king_sq: Square, piece_sq: Square, pt: PieceType, piece_color: Color) -> usize {
    if pt == PieceType::King {
        return usize::MAX;
    }
    
    let o_ksq = orient(perspective, king_sq);
    let o_psq = orient(perspective, piece_sq);
    
    let o_pc = if perspective == Color::White {
        piece_color
    } else {
        if piece_color == Color::White { Color::Black } else { Color::White }
    };
    
    let p_idx = ps_index(pt, o_pc);
    if p_idx == usize::MAX {
        return usize::MAX;
    }
    
    // Final index = 1 (skip BONA_PIECE_ZERO) + king_sq * 640 + piece_index
    o_ksq * KING_BUCKET_SIZE + 1 + p_idx + o_psq
}

/// Backwards compatibility for make_index
#[inline]
pub fn halfkp_index(king_sq: Square, piece_sq: Square, pt: PieceType, piece_color: Color, perspective: Color) -> usize {
    make_index(perspective, king_sq, piece_sq, pt, piece_color)
}

/// Add weights for a feature to accumulator
#[inline]
fn add_weights(acc: &mut Accumulator, index: usize, weights: &[i16]) {
    if index == usize::MAX || index >= INPUT_SIZE {
        return;
    }
    let offset = index * HALF_DIMENSIONS;
    
    #[cfg(target_arch = "x86_64")]
    {
        if use_avx2() {
            unsafe { add_weights_avx2(acc, &weights[offset..offset + HALF_DIMENSIONS]); }
            return;
        }
    }
    
    for i in 0..HALF_DIMENSIONS {
        acc.values[i] = acc.values[i].saturating_add(weights[offset + i]);
    }
}

/// Subtract weights for a feature from accumulator
#[inline]
fn sub_weights(acc: &mut Accumulator, index: usize, weights: &[i16]) {
    if index == usize::MAX || index >= INPUT_SIZE {
        return;
    }
    let offset = index * HALF_DIMENSIONS;
    
    #[cfg(target_arch = "x86_64")]
    {
        if use_avx2() {
            unsafe { sub_weights_avx2(acc, &weights[offset..offset + HALF_DIMENSIONS]); }
            return;
        }
    }
    
    for i in 0..HALF_DIMENSIONS {
        acc.values[i] = acc.values[i].saturating_sub(weights[offset + i]);
    }
}

/// Used for incremental updates during make_move
pub fn update_feature(acc: &mut Accumulator, index: usize, add: bool) {
    let net = match NETWORK.get() {
        Some(n) => n,
        None => return,
    };
    
    if add {
        add_weights(acc, index, &net.ft_weights);
    } else {
        sub_weights(acc, index, &net.ft_weights);
    }
}

/// Batch update features for better performance
pub fn update_feature_batch(acc: &mut Accumulator, updates: &[(usize, bool)]) {
    let net = unsafe { get_network() };

    #[cfg(target_arch = "x86_64")]
    {
        if use_avx2() {
            unsafe {
                update_feature_batch_avx2(acc, updates, &net.ft_weights);
            }
            return;
        }
    }

    for &(index, add) in updates {
        if index == usize::MAX || index >= INPUT_SIZE {
            continue;
        }
        let offset = index * HALF_DIMENSIONS;
        if add {
            for i in 0..HALF_DIMENSIONS {
                acc.values[i] = acc.values[i].saturating_add(net.ft_weights[offset + i]);
            }
        } else {
            for i in 0..HALF_DIMENSIONS {
                acc.values[i] = acc.values[i].saturating_sub(net.ft_weights[offset + i]);
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn update_feature_batch_avx2(
    acc: &mut Accumulator,
    updates: &[(usize, bool)],
    weights: &[i16],
) {
    use std::arch::x86_64::*;

    let acc_ptr = acc.values.as_mut_ptr();
    let w_ptr = weights.as_ptr();

    // Process 16 i16 values at a time (256 bits)
    for i in (0..HALF_DIMENSIONS).step_by(16) {
        let mut sum = unsafe {
        _mm256_loadu_si256(acc_ptr.add(i) as *const __m256i)
        };

        for &(index, add) in updates {
            if index == usize::MAX || index >= INPUT_SIZE {
                continue;
            }
            let offset = index * HALF_DIMENSIONS;
            let w = unsafe {
                _mm256_loadu_si256(w_ptr.add(offset + i) as *const __m256i)
            }; 

            if add {
                sum = _mm256_adds_epi16(sum, w);
            } else {
                sum = _mm256_subs_epi16(sum, w);
            }
        }

       unsafe { _mm256_storeu_si256(acc_ptr.add(i) as *mut __m256i, sum); }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn add_weights_avx2(acc: &mut Accumulator, weights: &[i16]) {
    use std::arch::x86_64::*;
    
    let acc_ptr = acc.values.as_mut_ptr();
    let w_ptr = weights.as_ptr();
    
    // Process 16 i16 values at a time (256 bits)
    for i in (0..HALF_DIMENSIONS).step_by(16) {
        unsafe {
            let a = _mm256_loadu_si256(acc_ptr.add(i) as *const __m256i);
            let w = _mm256_loadu_si256(w_ptr.add(i) as *const __m256i);
            let sum = _mm256_adds_epi16(a, w);
            _mm256_storeu_si256(acc_ptr.add(i) as *mut __m256i, sum);
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn sub_weights_avx2(acc: &mut Accumulator, weights: &[i16]) {
    use std::arch::x86_64::*;
    
    let acc_ptr = acc.values.as_mut_ptr();
    let w_ptr = weights.as_ptr();
    
    for i in (0..HALF_DIMENSIONS).step_by(16) {
        unsafe {
            let a = _mm256_loadu_si256(acc_ptr.add(i) as *const __m256i);
            let w = _mm256_loadu_si256(w_ptr.add(i) as *const __m256i);
            let diff = _mm256_subs_epi16(a, w);
            _mm256_storeu_si256(acc_ptr.add(i) as *mut __m256i, diff);
        }
    }
}

/// Refresh both accumulators from scratch
pub fn refresh_accumulator(board: &Board) -> [Accumulator; 2] {
    let net = match NETWORK.get() {
        Some(n) => n,
        None => return [Accumulator::default(); 2],
    };
    
    let mut accs = [Accumulator::default(); 2];
    
    // Get king squares
    let wk_bb = board.pieces[PieceType::King as usize][Color::White as usize];
    let bk_bb = board.pieces[PieceType::King as usize][Color::Black as usize];
    
    if wk_bb == 0 || bk_bb == 0 {
        return accs;
    }
    
    let wk_sq = wk_bb.trailing_zeros() as u8;
    let bk_sq = bk_bb.trailing_zeros() as u8;
    
    accs[0].values.copy_from_slice(&net.ft_biases);
    accs[1].values.copy_from_slice(&net.ft_biases);
    
    for pt_idx in 0..5 {
        let pt = PieceType::from(pt_idx);

        for color_idx in 0..2 {
            let pc = if color_idx == 0 { Color::White } else { Color::Black };
            let mut bb = board.pieces[pt_idx][color_idx];
            
            while bb != 0 {
                let sq = bb.trailing_zeros() as u8;
                bb &= bb - 1;
                
                // White's accumulator (uses white king position)
                let idx_w = make_index(Color::White, wk_sq, sq, pt, pc);
                add_weights(&mut accs[0], idx_w, &net.ft_weights);
                
                // Black's accumulator (uses black king position)
                let idx_b = make_index(Color::Black, bk_sq, sq, pt, pc);
                add_weights(&mut accs[1], idx_b, &net.ft_weights);
            }
        }
    }
    
    accs
}

/// Clipped ReLU: clamp to [0, 127] for i16 input
#[inline]
fn crelu_i16(x: i16) -> u8 {
    x.clamp(0, 127) as u8
}

/// Clipped ReLU for i32 after scaling
#[inline]
fn crelu_i32(x: i32) -> u8 {
    (x >> WEIGHT_SCALE_BITS).clamp(0, 127) as u8
}

/// Evaluate the current position using NNUE
pub fn evaluate(board: &Board) -> i32 {
    let net = match NETWORK.get() {
        Some(n) => n,
        None => return 0,
    };

    let (stm_acc, nstm_acc) = if board.side_to_move == Color::White {
        (&board.accumulator[0], &board.accumulator[1])
    } else {
        (&board.accumulator[1], &board.accumulator[0])
    };

    #[cfg(target_arch = "x86_64")]
    {
        if use_avx2() {
            return unsafe { evaluate_avx2(net, stm_acc, nstm_acc) };
        }
    }
    
    evaluate_scalar(net, stm_acc, nstm_acc)
}

fn evaluate_scalar(net: &Network, stm_acc: &Accumulator, nstm_acc: &Accumulator) -> i32 {
    // Build clipped input (512 u8 values)
    let mut input = [0u8; 512];
    for i in 0..HALF_DIMENSIONS {
        input[i] = crelu_i16(stm_acc.values[i]);
        input[HALF_DIMENSIONS + i] = crelu_i16(nstm_acc.values[i]);
    }

    // Layer 1: 512 -> 32
    let mut l1_out = [0i32; L2_SIZE];
    for i in 0..L2_SIZE {
        let mut sum = net.l1_biases[i];
        for j in 0..512 {
            sum += (input[j] as i32) * (net.l1_weights[i * 512 + j] as i32);
        }
        l1_out[i] = sum;
    }

    // Layer 2: 32 -> 32
    let mut l2_out = [0i32; L3_SIZE];
    for i in 0..L3_SIZE {
        let mut sum = net.l2_biases[i];
        for j in 0..L2_SIZE {
            let inp = crelu_i32(l1_out[j]) as i32;
            sum += inp * (net.l2_weights[i * L2_SIZE + j] as i32);
        }
        l2_out[i] = sum;
    }

    // Layer 3 (output): 32 -> 1
    let mut output = net.l3_bias;
    for j in 0..L3_SIZE {
        let inp = crelu_i32(l2_out[j]) as i32;
        output += inp * (net.l3_weights[j] as i32);
    }

    // Final scaling
    (output / FV_SCALE).clamp(-30000, 30000)
}

// #[cfg(target_arch = "x86_64")]
// #[target_feature(enable = "avx2")]
// unsafe fn evaluate_avx2(net: &Network, stm_acc: &Accumulator, nstm_acc: &Accumulator) -> i32 {
//     use std::arch::x86_64::*;
    
//     // Build clipped input vector (512 i8 values stored as u8)
//     let mut input = [0i8; 512];
    
//     unsafe {
//         let zero = _mm256_setzero_si256();
//         let max_val = _mm256_set1_epi16(127);
        
//         // Process STM accumulator
//         for i in (0..HALF_DIMENSIONS).step_by(16) {
//             let v = _mm256_loadu_si256(stm_acc.values.as_ptr().add(i) as *const __m256i);
//             let clamped = _mm256_min_epi16(_mm256_max_epi16(v, zero), max_val);
//             // Pack to bytes
//             let packed = _mm256_packs_epi16(clamped, zero);
//             let permuted = _mm256_permute4x64_epi64(packed, 0b11011000);
//             _mm_storeu_si128(
//                 input.as_mut_ptr().add(i) as *mut __m128i,
//                 _mm256_castsi256_si128(permuted)
//             );
//         }
        
//         // Process NSTM accumulator
//         for i in (0..HALF_DIMENSIONS).step_by(16) {
//             let v = _mm256_loadu_si256(nstm_acc.values.as_ptr().add(i) as *const __m256i);
//             let clamped = _mm256_min_epi16(_mm256_max_epi16(v, zero), max_val);
//             let packed = _mm256_packs_epi16(clamped, zero);
//             let permuted = _mm256_permute4x64_epi64(packed, 0b11011000);
//             _mm_storeu_si128(
//                 input.as_mut_ptr().add(HALF_DIMENSIONS + i) as *mut __m128i,
//                 _mm256_castsi256_si128(permuted)
//             );
//         }

//         // Layer 1: 512 -> 32 using AVX2 dot products
//         let mut l1_out = [0i32; L2_SIZE];
        
//         for i in 0..L2_SIZE {
//             let mut sum = _mm256_setzero_si256();
//             let weights_base = i * 512;
            
//             // Process 32 elements at a time
//             for j in (0..512).step_by(32) {
//                 let inp = _mm256_loadu_si256(input.as_ptr().add(j) as *const __m256i);
//                 let wgt = _mm256_loadu_si256(net.l1_weights.as_ptr().add(weights_base + j) as *const __m256i);
                
//                 // Multiply and add horizontally
//                 let product = _mm256_maddubs_epi16(inp, wgt);
//                 let product_32 = _mm256_madd_epi16(product, _mm256_set1_epi16(1));
//                 sum = _mm256_add_epi32(sum, product_32);
//             }
            
//             // Horizontal sum
//             let sum128 = _mm_add_epi32(
//                 _mm256_castsi256_si128(sum),
//                 _mm256_extracti128_si256(sum, 1)
//             );
//             let sum64 = _mm_add_epi32(sum128, _mm_srli_si128(sum128, 8));
//             let sum32 = _mm_add_epi32(sum64, _mm_srli_si128(sum64, 4));
            
//             l1_out[i] = net.l1_biases[i] + _mm_cvtsi128_si32(sum32);
//         }

//         // Layer 2: 32 -> 32 (smaller, use scalar)
//         let mut l2_out = [0i32; L3_SIZE];
//         for i in 0..L3_SIZE {
//             let mut sum = net.l2_biases[i];
//             for j in 0..L2_SIZE {
//                 let inp = (l1_out[j] >> WEIGHT_SCALE_BITS).clamp(0, 127);
//                 sum += inp * (net.l2_weights[i * L2_SIZE + j] as i32);
//             }
//             l2_out[i] = sum;
//         }

//         // Layer 3: 32 -> 1
//         let mut output = net.l3_bias;
//         for j in 0..L3_SIZE {
//             let inp = (l2_out[j] >> WEIGHT_SCALE_BITS).clamp(0, 127);
//             output += inp * (net.l3_weights[j] as i32);
//         }

//         (output / FV_SCALE).clamp(-30000, 30000)
//     }
// }

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn evaluate_avx2(net: &Network, stm_acc: &Accumulator, nstm_acc: &Accumulator) -> i32 {
    use std::arch::x86_64::*;

    #[repr(C, align(64))]
    struct AlignedInput {
        data: [u8; 512]
    }
    
    // Build clipped input vector (512 u8 values, all 0-127)
    let mut input = AlignedInput {
        data: [0u8; 512]
    };
    
    let zero = _mm256_setzero_si256();
    let max_val = _mm256_set1_epi16(127);
   unsafe { 
         // Process STM accumulator
        for i in (0..HALF_DIMENSIONS).step_by(16) {
            let v = _mm256_loadu_si256(stm_acc.values.as_ptr().add(i) as *const __m256i);
            let clamped = _mm256_min_epi16(_mm256_max_epi16(v, zero), max_val);
            let packed = _mm256_packus_epi16(clamped, zero);  // Use packus for unsigned
            let permuted = _mm256_permute4x64_epi64(packed, 0b11011000);
            _mm_storeu_si128(
                input.data.as_mut_ptr().add(i) as *mut __m128i,
                _mm256_castsi256_si128(permuted)
            );
        }

    }
    unsafe {
    // Process NSTM accumulator  
        for i in (0..HALF_DIMENSIONS).step_by(16) {
            let v = _mm256_loadu_si256(nstm_acc.values.as_ptr().add(i) as *const __m256i);
            let clamped = _mm256_min_epi16(_mm256_max_epi16(v, zero), max_val);
            let packed = _mm256_packus_epi16(clamped, zero);
            let permuted = _mm256_permute4x64_epi64(packed, 0b11011000);
            _mm_storeu_si128(
                input.data.as_mut_ptr().add(HALF_DIMENSIONS + i) as *mut __m128i,
                _mm256_castsi256_si128(permuted)
            );
        }
    }

    // Layer 1: 512 -> 32
    let mut l1_out = [0i32; L2_SIZE];
    
    for i in 0..L2_SIZE {
        let mut sum = _mm256_setzero_si256();
        let weights_base = i * 512;

        unsafe {
            for j in (0..512).step_by(32) {
                // input is u8 (0-127), weights are i8
                let inp = _mm256_loadu_si256(input.data.as_ptr().add(j) as *const __m256i);
                let wgt = _mm256_loadu_si256(net.l1_weights.as_ptr().add(weights_base + j) as *const __m256i);
                
                // maddubs: treats first arg as unsigned, second as signed - perfect!
                let product = _mm256_maddubs_epi16(inp, wgt);
                let product_32 = _mm256_madd_epi16(product, _mm256_set1_epi16(1));
                sum = _mm256_add_epi32(sum, product_32);
            }
        }
        
        let sum128 = _mm_add_epi32(
            _mm256_castsi256_si128(sum),
            _mm256_extracti128_si256(sum, 1)
        );
        let sum64 = _mm_add_epi32(sum128, _mm_srli_si128(sum128, 8));
        let sum32 = _mm_add_epi32(sum64, _mm_srli_si128(sum64, 4));
        
        l1_out[i] = net.l1_biases[i] + _mm_cvtsi128_si32(sum32);
    }

    // L2 and L3 (small, keep scalar)
    let mut l2_out = [0i32; L3_SIZE];
    for i in 0..L3_SIZE {
        let mut sum = net.l2_biases[i];
        for j in 0..L2_SIZE {
            let inp = (l1_out[j] >> WEIGHT_SCALE_BITS).clamp(0, 127);
            sum += inp * (net.l2_weights[i * L2_SIZE + j] as i32);
        }
        l2_out[i] = sum;
    }

    let mut output = net.l3_bias;
    for j in 0..L3_SIZE {
        let inp = (l2_out[j] >> WEIGHT_SCALE_BITS).clamp(0, 127);
        output += inp * (net.l3_weights[j] as i32);
    }

    (output / FV_SCALE).clamp(-30000, 30000)
}

pub fn add_piece(board: &mut Board, sq: Square, pt: PieceType, pc: Color) {
    let net = match NETWORK.get() {
        Some(n) => n,
        None => return,
    };
    
    if pt == PieceType::King {
        return;
    }
    
    let wk_sq = board.pieces[PieceType::King as usize][Color::White as usize].trailing_zeros() as u8;
    let bk_sq = board.pieces[PieceType::King as usize][Color::Black as usize].trailing_zeros() as u8;
    
    let idx_w = make_index(Color::White, wk_sq, sq, pt, pc);
    let idx_b = make_index(Color::Black, bk_sq, sq, pt, pc);
    
    add_weights(&mut board.accumulator[0], idx_w, &net.ft_weights);
    add_weights(&mut board.accumulator[1], idx_b, &net.ft_weights);
}

pub fn remove_piece(board: &mut Board, sq: Square, pt: PieceType, pc: Color) {
    let net = match NETWORK.get() {
        Some(n) => n,
        None => return,
    };
    
    if pt == PieceType::King {
        return;
    }
    
    let wk_sq = board.pieces[PieceType::King as usize][Color::White as usize].trailing_zeros() as u8;
    let bk_sq = board.pieces[PieceType::King as usize][Color::Black as usize].trailing_zeros() as u8;
    
    let idx_w = make_index(Color::White, wk_sq, sq, pt, pc);
    let idx_b = make_index(Color::Black, bk_sq, sq, pt, pc);
    
    sub_weights(&mut board.accumulator[0], idx_w, &net.ft_weights);
    sub_weights(&mut board.accumulator[1], idx_b, &net.ft_weights);
}



pub fn debug_eval(board: &Board) {
    let net = match NETWORK.get() {
        Some(n) => n,
        None => {
            println!("Network not loaded!");
            return;
        }
    };

    let wk = board.pieces[PieceType::King as usize][Color::White as usize].trailing_zeros();
    let bk = board.pieces[PieceType::King as usize][Color::Black as usize].trailing_zeros();
    
    println!("=== NNUE Debug ===");
    println!("White king sq: {}, Black king sq: {}", wk, bk);
    println!("Side to move: {:?}", board.side_to_move);
    
    let mut w_features = 0usize;
    let mut b_features = 0usize;
    
    for pt_idx in 0..5 {
        let pt = PieceType::from(pt_idx);
        for c in 0..2 {
            let pc = if c == 0 { Color::White } else { Color::Black };
            let count = board.pieces[pt_idx][c].count_ones() as usize;
            w_features += count;
            b_features += count;
        }
    }
    
    println!("Active features per perspective: {}", w_features);
    println!("Acc[0] (White) first 8: {:?}", &board.accumulator[0].values[0..8]);
    println!("Acc[1] (Black) first 8: {:?}", &board.accumulator[1].values[0..8]);
    
    // Check if accumulators look reasonable
    let sum_w: i64 = board.accumulator[0].values.iter().map(|&x| x as i64).sum();
    let sum_b: i64 = board.accumulator[1].values.iter().map(|&x| x as i64).sum();
    println!("Acc[0] sum: {}, Acc[1] sum: {}", sum_w, sum_b);
    
    let score = evaluate(board);
    println!("NNUE eval: {} cp", score);
    println!("==================");
}


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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_index_bounds() {
        for king_sq in 0..64u8 {
            for piece_sq in 0..64u8 {
                for pt in [PieceType::Pawn, PieceType::Knight, PieceType::Bishop, PieceType::Rook, PieceType::Queen] {
                    for piece_color in [Color::White, Color::Black] {
                        for perspective in [Color::White, Color::Black] {
                            let idx = make_index(perspective, king_sq, piece_sq, pt, piece_color);
                            assert!(idx < INPUT_SIZE, 
                                "Index {} >= {} for k={}, p={}, pt={:?}, pc={:?}, persp={:?}",
                                idx, INPUT_SIZE, king_sq, piece_sq, pt, piece_color, perspective);
                        }
                    }
                }
            }
        }
    }
    
    #[test]
    fn test_orient() {
        assert_eq!(orient(Color::White, 0), 0);
        assert_eq!(orient(Color::Black, 0), 63);
        assert_eq!(orient(Color::White, 63), 63);
        assert_eq!(orient(Color::Black, 63), 0);
        assert_eq!(orient(Color::White, 4), 4);
        assert_eq!(orient(Color::Black, 4), 59);
    }
    
    #[test]
    fn test_symmetric_positions() {
        let idx_w = make_index(Color::White, 4, 12, PieceType::Pawn, Color::White);
        let idx_b = make_index(Color::Black, 60, 52, PieceType::Pawn, Color::Black);
        assert_eq!(idx_w, idx_b, "Symmetric positions should have equal indices");
    }
    
    #[test]
    fn test_halfkp_index_alias() {
        let idx1 = make_index(Color::White, 4, 12, PieceType::Pawn, Color::White);
        let idx2 = halfkp_index(4, 12, PieceType::Pawn, Color::White, Color::White);
        assert_eq!(idx1, idx2);
    }
}