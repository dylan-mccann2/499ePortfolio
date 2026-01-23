/// Board Struct
/// bitboard implementation

#[derive(Clone, Copy Debug, PartialEq, Eq)]
pub struct Board {
  // One bitboard per piece type 
  pub piece_bb: [Bitboard; 7]
  // one bitboard per color
  pub color_bb: [Bitboard; 2]

  //occupancy bitboards
  all_pieces: Bitboard,

  //state
  side_to_move: Color,
  castling_rights: CastlingRights,
  en_passant_square: Option<Square>,
  halfmove_clock: u8,
  fullmove_number: u16,

  //Zobrist hash for transposition table
  hash: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
  White,
  Black,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Square(u8);

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
    let mut board = Board {
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
    pos.setup();
    pos.calc_hash();
    pos
  }

  pub fn startpos() -> Self {
    //starting position
    Self {
      side_to_move: Color::White,
      castling_rights: CastlingRights {
        white_kingside: true,
        white_queenside: true,
        black_kingside: true,
        black_queenside: true,
      },
      en_passant_square: None,
      halfmove_clock: 0,

    }
  }

  pub fn from_fen(fen: &str) -> Result<Self, FenError> {
    //split fen into sections
    let slices: Vec<&str> = fen.split_whitespace().collect();

    if slices.len != 6 {
      return Err(FenError::InvalidFormat);
    }

    // board section
    

      

      
  }

  fn parse_bboards(board: &str) -> Result<[Bitboard; 10], FenError>{
    let white_bb: u8 = 7;
    let black_bb: u8 = 8;
    let occupied: u8 = 9;
    
    let bbs: [Bitboard; 10];

    let ranks: Vec<&str> = position.split('/').collect();
    
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
          let piece = parse_piece(ch)?;
          let square_idx = actual_rank * 8 + file_idx;
          squares[square_idx] = Some(piece);
          file_idx += 1;
        }
      }
      
      if file_idx != 8 {
        return Err(FenError::InvalidRankLength);
      }
    }


  }

  fn parse_piece(ch: char) -> Result<Piece, FenError> {
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
    
    Ok(Piece { color, piece_type })
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
    piece_bb[PieceType::Pawn] = PAWNS_START;
    piece_bb[PieceType::Knight] = KNIGHTS_START_START;
    piece_bb[PieceType::Bishop] = BISHOPS_START;
    piece_bb[PieceType::Rook] = ROOKS_START;
    piece_bb[PieceType::Queen] = QUEENS_START;
    piece_bb[PieceType::King] = KINGS_START;

    color_bb[Color::White] = WHTIE_START;
    color_bb[Color::Black] = BLACK_START;
  }
}


