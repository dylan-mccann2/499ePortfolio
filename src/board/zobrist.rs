// Deterministic Zobrist key generation at runtime.
// Uses a simple SplitMix64 PRNG with a fixed seed so values are stable across runs.

use std::sync::OnceLock;

fn splitmix64(state: &mut u64) -> u64 {
	*state = state.wrapping_add(0x9E3779B97F4A7C15);
	let mut z = *state;
	z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
	z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
	z ^ (z >> 31)
}

fn gen_zobrist_keys() -> [[[u64; 64]; 2]; 7] {
	let mut state: u64 = 0xDEADBEEFDEADBEEF;
	let mut keys = [[[0u64; 64]; 2]; 7];
	for piece in 0..7 {
		for color in 0..2 {
			for sq in 0..64 {
				keys[piece][color][sq] = splitmix64(&mut state);
			}
		}
	}
	keys
}

fn gen_castling_rights() -> [u64; 4] {
	let mut state: u64 = 0xC0FFEEC0FFEE;
	let mut out = [0u64; 4];
	for i in 0..4 { out[i] = splitmix64(&mut state); }
	out
}

fn gen_en_passant() -> [u64; 64] {
	let mut state: u64 = 0xFEEDFACECAFEBABE;
	let mut out = [0u64; 64];
	for i in 0..64 { out[i] = splitmix64(&mut state); }
	out
}

static ZOBRIST_KEYS_LOCK: OnceLock<[[[u64; 64]; 2]; 7]> = OnceLock::new();
static ZOBRIST_SIDE_LOCK: OnceLock<u64> = OnceLock::new();
static ZOBRIST_CASTLING_LOCK: OnceLock<[u64; 4]> = OnceLock::new();
static ZOBRIST_EP_LOCK: OnceLock<[u64; 64]> = OnceLock::new();

pub fn zobrist_keys() -> &'static [[[u64; 64]; 2]; 7] {
	ZOBRIST_KEYS_LOCK.get_or_init(|| gen_zobrist_keys())
}

pub fn zobrist_side_to_move() -> u64 {
	*ZOBRIST_SIDE_LOCK.get_or_init(|| splitmix64(&mut 0x12345678ABCDEFu64))
}

pub fn zobrist_castling_rights() -> &'static [u64; 4] {
	ZOBRIST_CASTLING_LOCK.get_or_init(|| gen_castling_rights())
}

pub fn zobrist_en_passant() -> &'static [u64; 64] {
	ZOBRIST_EP_LOCK.get_or_init(|| gen_en_passant())
}
