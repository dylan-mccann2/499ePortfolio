// Board representation and logic.

// Declare the sub-modules found in the src/board/ directory
pub mod bitboard;
pub mod zobrist;

// Re-export the important parts of the sub-modules for a cleaner public API.
// This allows other parts of the crate to use `chess::board::Bitboard` 
pub use bitboard::*;
pub use zobrist::*;

use std::fmt;

/// Board Struct
/// uses bitboard implementation
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

impl Color {
    pub fn opposite(&self) -> Self {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CastlingRights {
  pub white_kingside: bool,
  pub white_queenside: bool,
  pub black_kingside: bool,
  pub black_queenside: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Unmove {
  pub mv: crate::movegen::Move,
  pub captured: PieceType,
  pub captured_sq: u8,
  pub piece_moved: PieceType,
  pub prev_castling: CastlingRights,
  pub prev_ep: Option<u8>,
  pub prev_halfmove: u8,
  pub prev_hash: u64,
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
    
  pub fn get_occupied(&self) -> Bitboard {
    self.occupied
  }

  pub fn get_color_bb(&self, color: Color) -> Bitboard {
    self.color_bb[color as usize]
  }

  pub fn get_piece_bb(&self, piece_type: PieceType) -> Bitboard {
    self.piece_bb[piece_type as usize]
  }

  pub fn get_side_to_move(&self) -> Color {
    self.side_to_move
  }

  pub fn get_en_passant_square(&self) -> Option<u8> {
    self.en_passant_square
  }

  pub fn get_castling_rights(&self) -> CastlingRights {
    self.castling_rights
  }

  pub fn get_hash(&self) -> u64 {
    self.hash
  }

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


  pub fn piece_at(&self, sq: u8) -> PieceType {
    let mask = square_mask(sq);
    for piece_type in 1..=6u8 {
      if self.piece_bb[piece_type as usize] & mask != 0 {
        return PieceType::from(piece_type);
      }
    }
    PieceType::None
  }

  pub fn remove_piece(&mut self, sq: u8, piece_type: PieceType, color: Color) {
    let mask = square_mask(sq);
    self.piece_bb[piece_type as usize] &= !mask;
    self.color_bb[color as usize] &= !mask;
    self.occupied &= !mask;
  }

  pub fn make_move(&mut self, mv: &crate::movegen::Move) -> Unmove {
    let us = self.side_to_move;
    let them = us.opposite();
    let from_mask = square_mask(mv.from);
    let to_mask = square_mask(mv.to);

    // Store state for unmake
    let prev_castling = self.castling_rights;
    let prev_ep = self.en_passant_square;
    let prev_halfmove = self.halfmove_clock;
    let prev_hash = self.hash;

    // Find the piece type being moved
    let piece_type = self.piece_at(mv.from);

    // Check for capture (including en passant)
    let mut captured = self.piece_at(mv.to);
    let mut captured_sq = mv.to;

    // Handle en passant capture
    if piece_type == PieceType::Pawn {
      if let Some(ep_sq) = self.en_passant_square {
        if mv.to == ep_sq {
          captured_sq = if us == Color::White { ep_sq - 8 } else { ep_sq + 8 };
          captured = PieceType::Pawn;
          // Update hash for captured pawn
          self.hash ^= zobrist_keys()[PieceType::Pawn as usize][them as usize][captured_sq as usize];
          self.remove_piece(captured_sq, PieceType::Pawn, them);
        }
      }
    }

    // Remove captured piece (non-ep)
    if captured != PieceType::None && captured_sq == mv.to {
      // Update hash for captured piece
      self.hash ^= zobrist_keys()[captured as usize][them as usize][mv.to as usize];
      self.remove_piece(mv.to, captured, them);
    }

    // Update hash: remove piece from source square
    self.hash ^= zobrist_keys()[piece_type as usize][us as usize][mv.from as usize];

    // Move the piece
    self.piece_bb[piece_type as usize] &= !from_mask;
    self.piece_bb[piece_type as usize] |= to_mask;
    self.color_bb[us as usize] &= !from_mask;
    self.color_bb[us as usize] |= to_mask;

    // Handle promotion
    let final_piece = if let Some(promo_piece) = mv.promotion {
      self.piece_bb[PieceType::Pawn as usize] &= !to_mask;
      self.piece_bb[promo_piece as usize] |= to_mask;
      promo_piece
    } else {
      piece_type
    };

    // Update hash: add piece to destination square
    self.hash ^= zobrist_keys()[final_piece as usize][us as usize][mv.to as usize];

    // Handle castling
    if piece_type == PieceType::King {
      let from_file = get_file(mv.from);
      let to_file = get_file(mv.to);
      if from_file == 4 && to_file == 6 {
        // Kingside castle
        let rook_from = if us == Color::White { 7 } else { 63 };
        let rook_to = if us == Color::White { 5 } else { 61 };
        let rook_from_mask = square_mask(rook_from);
        let rook_to_mask = square_mask(rook_to);
        // Update hash for rook movement
        self.hash ^= zobrist_keys()[PieceType::Rook as usize][us as usize][rook_from as usize];
        self.hash ^= zobrist_keys()[PieceType::Rook as usize][us as usize][rook_to as usize];
        self.piece_bb[PieceType::Rook as usize] &= !rook_from_mask;
        self.piece_bb[PieceType::Rook as usize] |= rook_to_mask;
        self.color_bb[us as usize] &= !rook_from_mask;
        self.color_bb[us as usize] |= rook_to_mask;
      } else if from_file == 4 && to_file == 2 {
        // Queenside castle
        let rook_from = if us == Color::White { 0 } else { 56 };
        let rook_to = if us == Color::White { 3 } else { 59 };
        let rook_from_mask = square_mask(rook_from);
        let rook_to_mask = square_mask(rook_to);
        // Update hash for rook movement
        self.hash ^= zobrist_keys()[PieceType::Rook as usize][us as usize][rook_from as usize];
        self.hash ^= zobrist_keys()[PieceType::Rook as usize][us as usize][rook_to as usize];
        self.piece_bb[PieceType::Rook as usize] &= !rook_from_mask;
        self.piece_bb[PieceType::Rook as usize] |= rook_to_mask;
        self.color_bb[us as usize] &= !rook_from_mask;
        self.color_bb[us as usize] |= rook_to_mask;
      }
    }

    // Update castling rights hash (remove old rights)
    if prev_castling.white_kingside { self.hash ^= zobrist_castling_rights()[0]; }
    if prev_castling.white_queenside { self.hash ^= zobrist_castling_rights()[1]; }
    if prev_castling.black_kingside { self.hash ^= zobrist_castling_rights()[2]; }
    if prev_castling.black_queenside { self.hash ^= zobrist_castling_rights()[3]; }

    // Update castling rights
    if piece_type == PieceType::King {
      if us == Color::White {
        self.castling_rights.white_kingside = false;
        self.castling_rights.white_queenside = false;
      } else {
        self.castling_rights.black_kingside = false;
        self.castling_rights.black_queenside = false;
      }
    }
    if piece_type == PieceType::Rook {
      match mv.from {
        0 => self.castling_rights.white_queenside = false,
        7 => self.castling_rights.white_kingside = false,
        56 => self.castling_rights.black_queenside = false,
        63 => self.castling_rights.black_kingside = false,
        _ => {}
      }
    }
    // Rook captured
    match mv.to {
      0 => self.castling_rights.white_queenside = false,
      7 => self.castling_rights.white_kingside = false,
      56 => self.castling_rights.black_queenside = false,
      63 => self.castling_rights.black_kingside = false,
      _ => {}
    }

    // Update castling rights hash (add new rights)
    if self.castling_rights.white_kingside { self.hash ^= zobrist_castling_rights()[0]; }
    if self.castling_rights.white_queenside { self.hash ^= zobrist_castling_rights()[1]; }
    if self.castling_rights.black_kingside { self.hash ^= zobrist_castling_rights()[2]; }
    if self.castling_rights.black_queenside { self.hash ^= zobrist_castling_rights()[3]; }

    // Update en passant hash (remove old)
    if let Some(ep_sq) = prev_ep {
      self.hash ^= zobrist_en_passant()[ep_sq as usize];
    }

    // Update en passant square
    self.en_passant_square = None;
    if piece_type == PieceType::Pawn {
      let from_rank = get_rank(mv.from);
      let to_rank = get_rank(mv.to);
      if (from_rank == 1 && to_rank == 3) || (from_rank == 6 && to_rank == 4) {
        self.en_passant_square = Some(if us == Color::White { mv.from + 8 } else { mv.from - 8 });
      }
    }

    // Update en passant hash (add new)
    if let Some(ep_sq) = self.en_passant_square {
      self.hash ^= zobrist_en_passant()[ep_sq as usize];
    }

    // Update halfmove clock
    if piece_type == PieceType::Pawn || captured != PieceType::None {
      self.halfmove_clock = 0;
    } else {
      self.halfmove_clock += 1;
    }

    // Update fullmove number
    if us == Color::Black {
      self.fullmove_number += 1;
    }

    // Update occupied
    self.occupied = self.color_bb[0] | self.color_bb[1];

    // Switch side to move and update hash
    self.side_to_move = them;
    self.hash ^= zobrist_side_to_move();

    Unmove {
      mv: *mv,
      captured,
      captured_sq,
      piece_moved: piece_type,
      prev_castling,
      prev_ep,
      prev_halfmove,
      prev_hash,
    }
  }

  pub fn unmake_move(&mut self, unmove: &Unmove) {
    let them = self.side_to_move; // After make_move, side_to_move is the opponent
    let us = them.opposite();
    let mv = &unmove.mv;
    let from_mask = square_mask(mv.from);
    let to_mask = square_mask(mv.to);

    // Restore side to move first
    self.side_to_move = us;

    // Determine the piece that was moved (accounting for promotion)
    let piece_on_to = if unmove.mv.promotion.is_some() {
      unmove.mv.promotion.unwrap()
    } else {
      unmove.piece_moved
    };

    // Move the piece back
    self.piece_bb[piece_on_to as usize] &= !to_mask;
    self.piece_bb[unmove.piece_moved as usize] |= from_mask;
    self.color_bb[us as usize] &= !to_mask;
    self.color_bb[us as usize] |= from_mask;

    // Restore captured piece
    if unmove.captured != PieceType::None {
      self.place_piece(unmove.captured_sq, unmove.captured, them);
    }

    // Handle castling (move rook back)
    if unmove.piece_moved == PieceType::King {
      let from_file = get_file(mv.from);
      let to_file = get_file(mv.to);
      if from_file == 4 && to_file == 6 {
        // Kingside castle - move rook back
        let rook_from = if us == Color::White { 7 } else { 63 };
        let rook_to = if us == Color::White { 5 } else { 61 };
        let rook_from_mask = square_mask(rook_from);
        let rook_to_mask = square_mask(rook_to);
        self.piece_bb[PieceType::Rook as usize] &= !rook_to_mask;
        self.piece_bb[PieceType::Rook as usize] |= rook_from_mask;
        self.color_bb[us as usize] &= !rook_to_mask;
        self.color_bb[us as usize] |= rook_from_mask;
      } else if from_file == 4 && to_file == 2 {
        // Queenside castle - move rook back
        let rook_from = if us == Color::White { 0 } else { 56 };
        let rook_to = if us == Color::White { 3 } else { 59 };
        let rook_from_mask = square_mask(rook_from);
        let rook_to_mask = square_mask(rook_to);
        self.piece_bb[PieceType::Rook as usize] &= !rook_to_mask;
        self.piece_bb[PieceType::Rook as usize] |= rook_from_mask;
        self.color_bb[us as usize] &= !rook_to_mask;
        self.color_bb[us as usize] |= rook_from_mask;
      }
    }

    // Restore state
    self.castling_rights = unmove.prev_castling;
    self.en_passant_square = unmove.prev_ep;
    self.halfmove_clock = unmove.prev_halfmove;
    self.hash = unmove.prev_hash;

    // Restore fullmove number
    if us == Color::Black {
      self.fullmove_number -= 1;
    }

    // Update occupied
    self.occupied = self.color_bb[0] | self.color_bb[1];
  }

  /// Returns the FEN string for the current position.
  pub fn to_fen(&self) -> String {
    let mut fen = String::with_capacity(64);

    // 1. Piece placement (rank 8 down to rank 1)
    for rank in (0..8).rev() {
      let mut empty = 0u8;
      for file in 0..8u8 {
        let sq = rank * 8 + file;
        let piece = self.piece_at(sq);
        if piece == PieceType::None {
          empty += 1;
        } else {
          if empty > 0 {
            fen.push((b'0' + empty) as char);
            empty = 0;
          }
          let is_white = (self.color_bb[Color::White as usize] & square_mask(sq)) != 0;
          let ch = match piece {
            PieceType::Pawn   => 'p',
            PieceType::Knight => 'n',
            PieceType::Bishop => 'b',
            PieceType::Rook   => 'r',
            PieceType::Queen  => 'q',
            PieceType::King   => 'k',
            _ => unreachable!(),
          };
          fen.push(if is_white { ch.to_ascii_uppercase() } else { ch });
        }
      }
      if empty > 0 {
        fen.push((b'0' + empty) as char);
      }
      if rank > 0 {
        fen.push('/');
      }
    }

    // 2. Side to move
    fen.push(' ');
    fen.push(if self.side_to_move == Color::White { 'w' } else { 'b' });

    // 3. Castling rights
    fen.push(' ');
    let cr = &self.castling_rights;
    if !cr.white_kingside && !cr.white_queenside && !cr.black_kingside && !cr.black_queenside {
      fen.push('-');
    } else {
      if cr.white_kingside  { fen.push('K'); }
      if cr.white_queenside { fen.push('Q'); }
      if cr.black_kingside  { fen.push('k'); }
      if cr.black_queenside { fen.push('q'); }
    }

    // 4. En passant square
    fen.push(' ');
    if let Some(ep) = self.en_passant_square {
      fen.push((b'a' + get_file(ep)) as char);
      fen.push((b'1' + get_rank(ep)) as char);
    } else {
      fen.push('-');
    }

    // 5. Halfmove clock
    fen.push(' ');
    fen.push_str(&self.halfmove_clock.to_string());

    // 6. Fullmove number
    fen.push(' ');
    fen.push_str(&self.fullmove_number.to_string());

    fen
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
    fn to_fen_startpos() {
      let b = Board::startpos();
      assert_eq!(
        b.to_fen(),
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
      );
    }

    #[test]
    fn to_fen_roundtrip() {
      let fens = [
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2",
        "r1bqk2r/ppppbppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
        "8/8/8/8/8/8/8/4K2R w K - 0 1",
        "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
        "3k4/4P3/8/8/8/8/8/4K3 w - - 0 1",
      ];
      for fen in fens {
        let board = Board::from_fen(fen).expect(&format!("failed to parse: {}", fen));
        assert_eq!(board.to_fen(), fen);
      }
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