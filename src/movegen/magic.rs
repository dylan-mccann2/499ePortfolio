use crate::board::bitboard::*;
use lazy_static::lazy_static;

// Pre-computed magic numbers (from Stockfish)
// These are well-known constants that provide perfect hashing for slider attacks
const ROOK_MAGIC_NUMBERS: [u64; 64] = [
    0x0080001020400080, 0x0040001000200040, 0x0080081000200080, 0x0080040800100080,
    0x0080020400080080, 0x0080010200040080, 0x0080008001000200, 0x0080002040800100,
    0x0000800020400080, 0x0000400020005000, 0x0000801000200080, 0x0000800800100080,
    0x0000800400080080, 0x0000800200040080, 0x0000800100020080, 0x0000800040800100,
    0x0000208000400080, 0x0000404000201000, 0x0000808010002000, 0x0000808008001000,
    0x0000808004000800, 0x0000808002000400, 0x0000010100020004, 0x0000020000408104,
    0x0000208080004000, 0x0000200040005000, 0x0000100080200080, 0x0000080080100080,
    0x0000040080080080, 0x0000020080040080, 0x0000010080800200, 0x0000800080004100,
    0x0000204000800080, 0x0000200040401000, 0x0000100080802000, 0x0000080080801000,
    0x0000040080800800, 0x0000020080800400, 0x0000020001010004, 0x0000800040800100,
    0x0000204000808000, 0x0000200040008080, 0x0000100020008080, 0x0000080010008080,
    0x0000040008008080, 0x0000020004008080, 0x0000010002008080, 0x0000004081020004,
    0x0000204000800080, 0x0000200040008080, 0x0000100020008080, 0x0000080010008080,
    0x0000040008008080, 0x0000020004008080, 0x0000800100020080, 0x0000800041000080,
    0x00FFFCDDFCED714A, 0x007FFCDDFCED714A, 0x003FFFCDFFD88096, 0x0000040810002101,
    0x0001000204080011, 0x0001000204000801, 0x0001000082000401, 0x0001FFFAABFAD1A2,
];

const BISHOP_MAGIC_NUMBERS: [u64; 64] = [
    0x0002020202020200, 0x0002020202020000, 0x0004010202000000, 0x0004040080000000,
    0x0001104000000000, 0x0000821040000000, 0x0000410410400000, 0x0000104104104000,
    0x0000040404040400, 0x0000020202020200, 0x0000040102020000, 0x0000040400800000,
    0x0000011040000000, 0x0000008210400000, 0x0000004104104000, 0x0000002082082000,
    0x0004000808080800, 0x0002000404040400, 0x0001000202020200, 0x0000800802004000,
    0x0000800400A00000, 0x0000200100884000, 0x0000400082082000, 0x0000200041041000,
    0x0002080010101000, 0x0001040008080800, 0x0000208004010400, 0x0000404004010200,
    0x0000840000802000, 0x0000404002011000, 0x0000808001041000, 0x0000404000820800,
    0x0001041000202000, 0x0000820800101000, 0x0000104400080800, 0x0000020080080080,
    0x0000404040040100, 0x0000808100020100, 0x0001010100020800, 0x0000808080010400,
    0x0000820820004000, 0x0000410410002000, 0x0000082088001000, 0x0000002011000800,
    0x0000080100400400, 0x0001010101000200, 0x0002020202000400, 0x0001010101000200,
    0x0000410410400000, 0x0000208208200000, 0x0000002084100000, 0x0000000020880000,
    0x0000001002020000, 0x0000040408020000, 0x0004040404040000, 0x0002020202020000,
    0x0000104104104000, 0x0000002082082000, 0x0000000020841000, 0x0000000000208800,
    0x0000000010020200, 0x0000000404080200, 0x0000040404040400, 0x0002020202020200,
];

#[derive(Clone, Copy)]
enum Direction {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

const RAYS: [[Direction; 4]; 2] = [
    [Direction::North, Direction::South, Direction::East, Direction::West],
    [Direction::NorthEast, Direction::NorthWest, Direction::SouthEast, Direction::SouthWest],
];

lazy_static! {
    pub static ref ROOK_MAGICS: [Magic; 64] = {
        let mut magics = [Magic::empty(); 64];
        for sq in 0..64 {
            magics[sq as usize] = find_magic(sq, true);
        }
        magics
    };
    pub static ref BISHOP_MAGICS: [Magic; 64] = {
        let mut magics = [Magic::empty(); 64];
        for sq in 0..64 {
            magics[sq as usize] = find_magic(sq, false);
        }
        magics
    };
}

fn generate_ray(sq: u8, direction: Direction) -> Bitboard {
    let mut attacks = 0;
    let rank = get_rank(sq);
    let file = get_file(sq);

    let (rank_inc, file_inc) = match direction {
        Direction::North => (1, 0),
        Direction::South => (-1, 0),
        Direction::East => (0, 1),
        Direction::West => (0, -1),
        Direction::NorthEast => (1, 1),
        Direction::NorthWest => (1, -1),
        Direction::SouthEast => (-1, 1),
        Direction::SouthWest => (-1, -1),
    };

    let mut to_rank = rank as i8 + rank_inc;
    let mut to_file = file as i8 + file_inc;

    while to_rank >= 0 && to_rank < 8 && to_file >= 0 && to_file < 8 {
        let to_sq = (to_rank * 8 + to_file) as u8;
        attacks |= square_mask(to_sq);
        to_rank += rank_inc;
        to_file += file_inc;
    }
    attacks
}

fn generate_rook_mask(sq: u8) -> Bitboard {
    let mut mask = 0;
    let rank = get_rank(sq);
    let file = get_file(sq);

    for r in (rank + 1)..7 {
        mask |= square_mask(r * 8 + file);
    }
    for r in (1..rank).rev() {
        mask |= square_mask(r * 8 + file);
    }
    for f in (file + 1)..7 {
        mask |= square_mask(rank * 8 + f);
    }
    for f in (1..file).rev() {
        mask |= square_mask(rank * 8 + f);
    }
    mask
}

fn generate_bishop_mask(sq: u8) -> Bitboard {
    let mut mask = 0;
    let rank = get_rank(sq);
    let file = get_file(sq);

    for i in 1.. {
        if rank + i > 6 || file + i > 6 {
            break;
        }
        mask |= square_mask((rank + i) * 8 + (file + i));
    }
    for i in 1.. {
        if rank + i > 6 || file <= i {
            break;
        }
        mask |= square_mask((rank + i) * 8 + (file - i));
    }
    for i in 1.. {
        if rank <= i || file + i > 6 {
            break;
        }
        mask |= square_mask((rank - i) * 8 + (file + i));
    }
    for i in 1.. {
        if rank <= i || file <= i {
            break;
        }
        mask |= square_mask((rank - i) * 8 + (file - i));
    }
    mask
}

fn generate_rook_attacks(sq: u8, blockers: Bitboard) -> Bitboard {
    let mut attacks = 0;
    for &direction in &RAYS[0] {
        let ray = generate_ray(sq, direction);
        let intersection = ray & blockers;
        if intersection != 0 {
            let first_blocker_sq = if Direction::is_positive(&direction) {
                intersection.trailing_zeros() as u8
            } else {
                63 - intersection.leading_zeros() as u8
            };
            attacks |= ray ^ generate_ray(first_blocker_sq, direction);
        } else {
            attacks |= ray;
        }
    }
    attacks
}

fn generate_bishop_attacks(sq: u8, blockers: Bitboard) -> Bitboard {
    let mut attacks = 0;
    for &direction in &RAYS[1] {
        let ray = generate_ray(sq, direction);
        let intersection = ray & blockers;
        if intersection != 0 {
            let first_blocker_sq = if Direction::is_positive(&direction) {
                intersection.trailing_zeros() as u8
            } else {
                63 - intersection.leading_zeros() as u8
            };
            attacks |= ray ^ generate_ray(first_blocker_sq, direction);
        } else {
            attacks |= ray;
        }
    }
    attacks
}

impl Direction {
    fn is_positive(direction: &Direction) -> bool {
        matches!(direction, Direction::North | Direction::NorthEast | Direction::NorthWest | Direction::East)
    }
}

#[derive(Clone, Copy)]
pub struct Magic {
    pub magic: u64,
    pub mask: Bitboard,
    pub shift: u8,
    pub attacks: &'static [Bitboard],
}

impl Magic {
    const fn empty() -> Self {
        Self {
            magic: 0,
            mask: 0,
            shift: 0,
            attacks: &[],
        }
    }
}

fn find_magic(sq: u8, is_rook: bool) -> Magic {
    let mask = if is_rook {
        generate_rook_mask(sq)
    } else {
        generate_bishop_mask(sq)
    };
    let shift = 64 - popcount(mask);

    // Use pre-computed magic number
    let magic = if is_rook {
        ROOK_MAGIC_NUMBERS[sq as usize]
    } else {
        BISHOP_MAGIC_NUMBERS[sq as usize]
    };

    // Build the attack table
    let table_size = 1 << popcount(mask);
    let mut attack_table = vec![0u64; table_size];

    // Enumerate all blocker configurations using Carry-Rippler
    let mut b: u64 = 0;
    loop {
        let attack = if is_rook {
            generate_rook_attacks(sq, b)
        } else {
            generate_bishop_attacks(sq, b)
        };

        let index = (b.wrapping_mul(magic)) >> shift;
        attack_table[index as usize] = attack;

        b = b.wrapping_sub(mask) & mask;
        if b == 0 {
            break;
        }
    }

    let attacks_leaked = Box::leak(attack_table.into_boxed_slice());
    Magic {
        magic,
        mask,
        shift,
        attacks: attacks_leaked,
    }
}