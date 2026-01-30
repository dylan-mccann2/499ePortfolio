/// Board Struct
/// bitboard implementation

use std::fmt;
use crate::board::bitboard::*;
use crate::board::zobrist::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
  // One bitboard per piece type 
  pub piece_bb: [Bitboard; 7],
  // one bitboard per color
  pub color_bb: [Bitboard; 2],

  //occupancy bitboards
  occupied: Bitboard,

  //state
  side_to_move: Color,
  castling_rights: CastlingRights,
  en_passant_square: Option<u8>,
  halfmove_clock: u8,
  fullmove_number: u16,

  //Zobrist hash for transposition table
  hash: u64,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
  White = 0,
  Black = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CastlingRights {
  white_kingside: bool,
  white_queenside: bool,
  black_kingside: bool,
  black_queenside: bool,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceType {
  None = 0,
  Pawn = 1,
  Knight = 2,
  Bishop = 3,
  Rook = 4,
  Queen = 5,
  King = 6,
}

impl From<u8> for PieceType {
  fn from(n: u8) -> Self {
    match n {
      1 => PieceType::Pawn,
      2 => PieceType::Knight,
      3 => PieceType::Bishop,
      4 => PieceType::Rook,
      5 => PieceType::Queen,
      6 => PieceType::King,
      _ => PieceType::None,
    }
  }
}


#[derive(Debug)]
pub enum FenError {
    InvalidFormat,
    InvalidPiece,
    InvalidColor,
    InvalidSquare,
    InvalidRankLength,
    ParseIntError,
}

impl fmt::Display for FenError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      FenError::InvalidFormat => write!(f, "Invalid FEN format"),
      FenError::InvalidPiece => write!(f, "Invalid piece character"),
      FenError::InvalidColor => write!(f, "Invalid color"),
      FenError::InvalidSquare => write!(f, "Invalid square"),
      FenError::InvalidRankLength => write!(f, "Invalid rank length"),
      FenError::ParseIntError => write!(f, "Failed to parse integer"),
    }
  }
}

impl Board {
    
  pub fn new() -> Self{
    let board = Board {
      piece_bb: [EMPTY; 7],
      color_bb: [EMPTY; 2],

      occupied: EMPTY,
      side_to_move: Color::White,
      castling_rights: CastlingRights {
        white_kingside: true,
        white_queenside: true,
        black_kingside: true,
        black_queenside: true,
      },
      en_passant_square: None,
      halfmove_clock: 0,
      fullmove_number: 1,
      hash:0,
    };
    board
  }

  pub fn startpos() -> Self {
    //starting position
    let mut b = Board::new();
    b.setup();
    b.side_to_move = Color::White;
    b.castling_rights = CastlingRights {
      white_kingside: true,
      white_queenside: true,
      black_kingside: true,
      black_queenside: true,
    };
    b.en_passant_square = None;
    b.halfmove_clock = 0;
    b.fullmove_number = 1;
    b.calc_hash();
    b
  }

  pub fn from_fen(fen: &str) -> Result<Self, FenError> {
    //create empty board
    let mut board = Board {
      piece_bb: [EMPTY; 7],
      color_bb: [EMPTY; 2], 
      occupied: EMPTY,
      side_to_move: Color::White,
      castling_rights: CastlingRights {
        white_kingside: false,
        white_queenside: false,
        black_kingside: false,
        black_queenside: false,
      },
      en_passant_square: None,
      halfmove_clock: 0,
      fullmove_number: 1,
      hash:0,
    };

    //split fen into sections
    let slices: Vec<&str> = fen.split_whitespace().collect();

    if slices.len() != 6 {
      return Err(FenError::InvalidFormat);
    }

    // board section
    board.parse_bboards(slices[0])?;
    //side to move
    match slices[1] {
      "w" => board.side_to_move = Color::White,
      "b" => board.side_to_move = Color::Black,
      _ => return Err(FenError::InvalidColor),  
    }
    //castling rights
    for ch in slices[2].chars() {
      match ch {
        'K' => board.castling_rights.white_kingside = true,
        'Q' => board.castling_rights.white_queenside = true,
        'k' => board.castling_rights.black_kingside = true,
        'q' => board.castling_rights.black_queenside = true,
        '-' => {},
        _ => return Err(FenError::InvalidFormat),
      }
    }

    //en passant square
    if slices[3] != "-" {
      board.en_passant_square = algebraic_to_square(slices[3]);
    }

    //halfmove clock
    board.halfmove_clock = slices[4].parse().map_err(|_| FenError::ParseIntError)?;

    //fullmove number
    board.fullmove_number = slices[5].parse().map_err(|_| FenError::ParseIntError)?;

    //calculate hash
    board.calc_hash();

    Ok(board)
  }

  fn parse_bboards(&mut self, board: &str) -> Result<(), FenError>{
    // splits ranks by '/'
    let ranks: Vec<&str> = board.split('/').collect();
    
    if ranks.len() != 8 {
      return Err(FenError::InvalidFormat);
    }

    for (rank_idx, rank) in ranks.iter().enumerate() {
      let actual_rank = 7 - rank_idx; // FEN starts from rank 8 down to rank 1
      let mut file_idx = 0;
      
      for ch in rank.chars() {
        if file_idx >= 8 {
          return Err(FenError::InvalidRankLength);
        }
        
        if ch.is_ascii_digit() {
          let skip = ch.to_digit(10).unwrap() as usize;
          file_idx += skip;
        } else {  
          let square_idx = actual_rank * 8 + file_idx;
          self.parse_and_place_piece(ch, square_idx as u8)?;
          file_idx += 1;
        }
      }
      
      if file_idx != 8 {
        return Err(FenError::InvalidRankLength);
      }
    }
    Ok(())
  }

  fn parse_and_place_piece(&mut self, ch: char, sq: u8) -> Result<(), FenError> {
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
      _ => return Err(FenError::InvalidPiece),
    };

    self.place_piece(sq, piece_type, color);
    Ok(())
  }

  #[inline(always)]
  pub fn place_piece(&mut self, square: u8, piece_type: PieceType, color: Color) {
    let mask = square_mask(square);
    
    self.piece_bb[piece_type as usize] |= mask;
    self.color_bb[color as usize] |= mask;
    self.occupied |= mask;
    
    //self.squares[square as usize] = ((color as u8) << 3) | piece_type as u8;
  }

  pub fn setup(&mut self){
    self.piece_bb[PieceType::Pawn as usize] = PAWNS_START;
    self.piece_bb[PieceType::Knight as usize] = KNIGHTS_START;
    self.piece_bb[PieceType::Bishop as usize] = BISHOPS_START;
    self.piece_bb[PieceType::Rook as usize] = ROOKS_START;
    self.piece_bb[PieceType::Queen as usize] = QUEENS_START;
    self.piece_bb[PieceType::King as usize] = KINGS_START;

    self.color_bb[Color::White as usize] = WHITE_START;
    self.color_bb[Color::Black as usize] = BLACK_START;
    self.occupied = self.color_bb[0] | self.color_bb[1];
  }


  fn calc_hash(&mut self) {
    //calculate zobrist hash for current position
    self.hash = 0;
    for square in 0..64 {
      for piece_type in 1..=6u8 {
        let idx = piece_type as usize;
        if has_bit(self.piece_bb[idx], square) {
          let color = if has_bit(self.color_bb[Color::White as usize], square) {
            Color::White
          } else {
            Color::Black
          };
          self.hash ^= zobrist_keys()[idx][color as usize][square as usize];
        }
      }
    }

    //side to move
    if self.side_to_move == Color::Black {
      self.hash ^= zobrist_side_to_move();
    }

    //castling rights
    if self.castling_rights.white_kingside {
      self.hash ^= zobrist_castling_rights()[0];
    }
    if self.castling_rights.white_queenside {
      self.hash ^= zobrist_castling_rights()[1];
    }
    if self.castling_rights.black_kingside {
      self.hash ^= zobrist_castling_rights()[2];
    }
    if self.castling_rights.black_queenside {
      self.hash ^= zobrist_castling_rights()[3];
    }

    //en passant square
    if let Some(sq) = self.en_passant_square {
      self.hash ^= zobrist_en_passant()[sq as usize];
    }
  }

}


  #[cfg(test)]
  mod tests {
    use super::*;

    #[test]
    fn new_board_is_empty() {
      let b = Board::new();
      for &bb in &b.piece_bb {
        assert_eq!(bb, EMPTY);
      }
      assert_eq!(b.color_bb[Color::White as usize], EMPTY);
      assert_eq!(b.color_bb[Color::Black as usize], EMPTY);
      assert_eq!(b.occupied, EMPTY);
      assert_eq!(b.hash, 0);
      assert_eq!(b.side_to_move, Color::White);
    }

    #[test]
    fn startpos_and_fen_hash_match() {
      let b1 = Board::startpos();
      let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
      let b2 = Board::from_fen(fen).expect("from_fen failed");

      assert_eq!(b1.piece_bb, b2.piece_bb);
      assert_eq!(b1.color_bb, b2.color_bb);
      assert_eq!(b1.occupied, b2.occupied);
      assert_eq!(b1.hash, b2.hash);
    }

    #[test]
    fn hash_changes_with_side_to_move_and_ep_and_castling() {
      let b = Board::startpos();

      let mut b_side = b;
      b_side.side_to_move = Color::Black;
      b_side.calc_hash();
      assert_ne!(b.hash, b_side.hash);

      let mut b_ep = b;
      b_ep.en_passant_square = Some(16);
      b_ep.calc_hash();
      assert_ne!(b.hash, b_ep.hash);

      let mut b_castle = b;
      b_castle.castling_rights.white_kingside = false;
      b_castle.castling_rights.white_queenside = false;
      b_castle.castling_rights.black_kingside = false;
      b_castle.castling_rights.black_queenside = false;
      b_castle.calc_hash();
      assert_ne!(b.hash, b_castle.hash);
    }

    #[test]
    fn from_fen_invalid() {
      assert!(Board::from_fen("invalid fen").is_err());
    }

    #[test]
    fn zobrist_different_positions_have_unique_hashes() {
      // Various positions should all have different hashes
      let positions = [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",      // startpos
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",   // 1.e4
        "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2", // 1.e4 e5
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq c6 0 2", // 1.e4 c5
        "r1bqkbnr/pppppppp/2n5/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 1 2",  // 1.e4 Nc6
      ];

      let hashes: Vec<u64> = positions
        .iter()
        .map(|fen| Board::from_fen(fen).unwrap().hash)
        .collect();

      // All hashes should be unique
      for i in 0..hashes.len() {
        for j in (i+1)..hashes.len() {
          assert_ne!(hashes[i], hashes[j],
            "Hash collision between position {} and {}", i, j);
        }
      }

      // All hashes should be non-zero (positions have pieces)
      for (i, &h) in hashes.iter().enumerate() {
        assert_ne!(h, 0, "Position {} has zero hash", i);
      }
    }

    #[test]
    fn zobrist_hash_is_deterministic() {
      let fen = "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4";
      let b1 = Board::from_fen(fen).unwrap();
      let b2 = Board::from_fen(fen).unwrap();
      assert_eq!(b1.hash, b2.hash);
    }

    #[test]
    fn zobrist_keys_are_nonzero() {
      // Verify key quality
      let keys = zobrist_keys();
      for piece in 1..=6 {
        for color in 0..2 {
          for sq in 0..64 {
            assert_ne!(keys[piece][color][sq], 0,
              "Zero key at piece={}, color={}, sq={}", piece, color, sq);
          }
        }
      }
      assert_ne!(zobrist_side_to_move(), 0);
      for i in 0..4 {
        assert_ne!(zobrist_castling_rights()[i], 0);
      }
    }
  }


