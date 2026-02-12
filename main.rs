use macroquad::prelude::*;
use macroquad::audio::{Sound, load_sound_from_bytes, play_sound_once};
use std::cmp::{max, min};
use std::ops::Not;
use std::collections::HashMap;
use std::time::Instant;
use std::process;

//الكود الكامل لي لعبة ضاما موجود في الاسفل//

// --- EMBEDDED ASSETS ---
const BYTES_BG: &[u8] = include_bytes!("../assets/wood background.png");
const BYTES_BG_SETTINGS: &[u8] = include_bytes!("../assets/ST.png"); // الخلفية الجديدة
const BYTES_UI: &[u8] = include_bytes!("../assets/wood ui.png");
const BYTES_PB: &[u8] = include_bytes!("../assets/pb.png");
const BYTES_PW: &[u8] = include_bytes!("../assets/pw.png");
const BYTES_KING: &[u8] = include_bytes!("../assets/king.png");
const BYTES_SND_GO: &[u8] = include_bytes!("../assets/go.wav");
const BYTES_SND_CAPTURE: &[u8] = include_bytes!("../assets/capture.wav");

// --- CONSTANTS ---
const BOARD_SIZE: usize = 8;
const SQUARE: f32 = 80.0;
const BOARD_PIXELS: f32 = BOARD_SIZE as f32 * SQUARE;
const WINDOW_W: f32 = 1000.0;
const WINDOW_H: f32 = 760.0;

// --- BITMASKS ---
const NOT_A_FILE: Bitboard = 0xfefefefefefefefe;
const NOT_H_FILE: Bitboard = 0x7f7f7f7f7f7f7f7f;
const WHITE_PROMOTION_RANK: Bitboard = 0xFF00000000000000;
const BLACK_PROMOTION_RANK: Bitboard = 0x00000000000000FF;
const DARK_SQUARES: Bitboard = 0xAA55AA55AA55AA55;
const CENTER_MASK: Bitboard = 0x00003C3C3C3C0000;
const DOG_HOLES: Bitboard = 0x8000000000000001 | 0x0100000000000080; 
const WHITE_BRIDGE: Bitboard = 0x000000000000000A; 
const BLACK_BRIDGE: Bitboard = 0x5000000000000000;

// Strong Corners (a7/b8 and g1/h2)
const CORNER_TL: Bitboard = (1 << 57) | (1 << 48); 
const CORNER_BR: Bitboard = (1 << 15) | (1 << 6);

// --- ENUMS ---
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum PieceColor { White, Black }

impl Not for PieceColor {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            PieceColor::White => PieceColor::Black,
            PieceColor::Black => PieceColor::White,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum PieceKind { Man, King }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Piece { kind: PieceKind, color: PieceColor }

// --- BITBOARD ---
type Bitboard = u64;

// --- STATE ---
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BitboardState {
    white_men: Bitboard,
    black_men: Bitboard,
    white_kings: Bitboard,
    black_kings: Bitboard,
    turn: PieceColor,
}

impl BitboardState {
    fn new() -> Self {
        Self {
            white_men: 0x000000000055AA55,
            black_men: 0xAA55AA0000000000,
            white_kings: 0,
            black_kings: 0,
            turn: PieceColor::White,
        }
    }

    fn white_pieces(&self) -> Bitboard { self.white_men | self.white_kings }
    fn black_pieces(&self) -> Bitboard { self.black_men | self.black_kings }
    fn all_pieces(&self) -> Bitboard { self.white_pieces() | self.black_pieces() }

    fn get_piece_at_bit(&self, bit: Bitboard) -> Option<Piece> {
        if (self.white_men & bit) != 0 { return Some(Piece { kind: PieceKind::Man, color: PieceColor::White }); }
        if (self.black_men & bit) != 0 { return Some(Piece { kind: PieceKind::Man, color: PieceColor::Black }); }
        if (self.white_kings & bit) != 0 { return Some(Piece { kind: PieceKind::King, color: PieceColor::White }); }
        if (self.black_kings & bit) != 0 { return Some(Piece { kind: PieceKind::King, color: PieceColor::Black }); }
        None
    }

    fn get_piece_at(&self, r: i32, c: i32) -> Option<Piece> {
        let bit_index = (7 - r) * 8 + c;
        if bit_index < 0 || bit_index > 63 { return None; }
        let bit = 1u64 << bit_index;
        self.get_piece_at_bit(bit)
    }

    fn apply_move(&self, m: &Move) -> Self {
        let mut next = *self;
        let from_piece = next.get_piece_at_bit(m.from).unwrap();
        
        if from_piece.color == PieceColor::White {
            next.white_men &= !m.from; next.white_kings &= !m.from;
        } else {
            next.black_men &= !m.from; next.black_kings &= !m.from;
        }

        let mut is_king = from_piece.kind == PieceKind::King;
        if !is_king {
            if from_piece.color == PieceColor::White && (m.to & WHITE_PROMOTION_RANK) != 0 { is_king = true; }
            if from_piece.color == PieceColor::Black && (m.to & BLACK_PROMOTION_RANK) != 0 { is_king = true; }
        }

        if from_piece.color == PieceColor::White {
            if is_king { next.white_kings |= m.to; } else { next.white_men |= m.to; }
        } else {
            if is_king { next.black_kings |= m.to; } else { next.black_men |= m.to; }
        }

        if m.captures != 0 {
            next.white_men &= !m.captures; next.white_kings &= !m.captures;
            next.black_men &= !m.captures; next.black_kings &= !m.captures;
        }

        next.turn = !next.turn;
        next
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Copy)]
struct Move {
    from: Bitboard,
    to: Bitboard,
    captures: Bitboard,
}

// --- ASSETS STRUCT ---
struct GameAssets {
    tex_bg: Texture2D,
    tex_bg_settings: Texture2D,
    tex_ui: Texture2D,
    tex_pb: Texture2D,
    tex_pw: Texture2D,
    tex_king: Texture2D,
    snd_go: Sound,
    snd_capture: Sound,
}

impl GameAssets {
    async fn load() -> Self {
        let tex_bg = Texture2D::from_file_with_format(BYTES_BG, Some(ImageFormat::Png));
        let tex_bg_settings = Texture2D::from_file_with_format(BYTES_BG_SETTINGS, Some(ImageFormat::Png));
        let tex_ui = Texture2D::from_file_with_format(BYTES_UI, Some(ImageFormat::Png));
        let tex_pb = Texture2D::from_file_with_format(BYTES_PB, Some(ImageFormat::Png));
        let tex_pw = Texture2D::from_file_with_format(BYTES_PW, Some(ImageFormat::Png));
        let tex_king = Texture2D::from_file_with_format(BYTES_KING, Some(ImageFormat::Png));
        
        let snd_go = load_sound_from_bytes(BYTES_SND_GO).await.unwrap();
        let snd_capture = load_sound_from_bytes(BYTES_SND_CAPTURE).await.unwrap();

        Self { tex_bg, tex_bg_settings, tex_ui, tex_pb, tex_pw, tex_king, snd_go, snd_capture }
    }
}

// --- ANIMATION STATE ---
struct Animation {
    piece: Piece,
    start_pos: Vec2,
    end_pos: Vec2,
    move_data: Move,
    progress: f32, // 0.0 to 1.0
}

// --- GAME STRUCT ---
#[derive(Clone, PartialEq, Eq, Copy)]
enum Mode { Menu, Settings, SideSelection, PvP, PvA, AvA }

struct Game {
    board_state: BitboardState,
    legal_moves: Vec<Move>,
    history: Vec<BitboardState>,
    mode: Mode,
    ai_white: bool,
    ai_black: bool,
    ai_should_move: bool,
    last_move: Option<Move>,
    is_mid_capture: bool,
    position_counts: HashMap<BitboardState, usize>,
    
    // Settings
    force_max_capture: bool,
    ai_time_limit: f32, // Seconds

    // UI & Interaction
    dragged_piece_idx: Option<usize>, 
    selected_square_idx: Option<usize>, 
    animation: Option<Animation>,
    
    half_move_clock: i32,
    full_move_number: i32,
    flipped: bool,
    game_over_printed: bool,
}

impl Game {
    fn new() -> Self {
        let mut g = Self {
            board_state: BitboardState::new(),
            legal_moves: vec![],
            history: vec![],
            mode: Mode::Menu,
            ai_white: false,
            ai_black: false,
            ai_should_move: false,
            last_move: None,
            is_mid_capture: false,
            position_counts: HashMap::new(),
            
            force_max_capture: true, // Default: True
            ai_time_limit: 0.8,      // Default: 0.8s

            dragged_piece_idx: None,
            selected_square_idx: None,
            animation: None,
            half_move_clock: 0,
            full_move_number: 1,
            flipped: false,
            game_over_printed: false,
        };
        g.reset_board();
        g
    }

    fn reset_board(&mut self) {
        self.board_state = BitboardState::new();
        self.legal_moves.clear();
        self.history.clear();
        self.last_move = None;
        self.is_mid_capture = false;
        self.position_counts.clear();
        self.position_counts.insert(self.board_state, 1);
        self.ai_should_move = false;
        self.half_move_clock = 0;
        self.full_move_number = 1;
        self.game_over_printed = false;
        self.dragged_piece_idx = None;
        self.selected_square_idx = None;
        self.animation = None;
    }

    fn undo(&mut self) {
        if let Some(removed_state) = self.history.pop() {
            if let Some(count) = self.position_counts.get_mut(&removed_state) {
                *count -= 1;
                if *count == 0 { self.position_counts.remove(&removed_state); }
            }
            if self.board_state.turn == PieceColor::White { self.full_move_number -= 1; }
        }
        if let Some(s) = self.history.last().copied() {
            self.board_state = s;
        } else {
            self.board_state = BitboardState::new();
            self.full_move_number = 1;
        }
        self.legal_moves.clear();
        self.last_move = None;
        self.is_mid_capture = false;
        self.game_over_printed = false;
        self.dragged_piece_idx = None;
        self.selected_square_idx = None;
        self.animation = None;
    }

    fn start_move_animation(&mut self, m: Move) {
        let piece = self.board_state.get_piece_at_bit(m.from).unwrap();
        
        let f = m.from.trailing_zeros();
        let t = m.to.trailing_zeros();
        let (fr, fc) = (7 - f / 8, f % 8);
        let (tr, tc) = (7 - t / 8, t % 8);
        
        let ox = (WINDOW_W - BOARD_PIXELS) / 2.0;
        let oy = (WINDOW_H - BOARD_PIXELS) / 2.0;

        let start_pos = if self.flipped {
            vec2(ox + (7 - fc) as f32 * SQUARE, oy + (7 - fr) as f32 * SQUARE)
        } else {
            vec2(ox + fc as f32 * SQUARE, oy + fr as f32 * SQUARE)
        };

        let end_pos = if self.flipped {
            vec2(ox + (7 - tc) as f32 * SQUARE, oy + (7 - tr) as f32 * SQUARE)
        } else {
            vec2(ox + tc as f32 * SQUARE, oy + tr as f32 * SQUARE)
        };

        self.animation = Some(Animation {
            piece,
            start_pos,
            end_pos,
            move_data: m,
            progress: 0.0,
        });
    }

    fn update_animation(&mut self, assets: &GameAssets) {
        if let Some(anim) = &mut self.animation {
            let dt = get_frame_time().min(0.05); 
            anim.progress += 5.0 * dt; 
            
            if anim.progress >= 1.0 {
                let m = anim.move_data;
                self.finalize_move(&m, assets);
                self.animation = None;
            }
        }
    }

    fn finalize_move(&mut self, m: &Move, assets: &GameAssets) {
        let piece = self.board_state.get_piece_at_bit(m.from).unwrap();
        let is_capture = m.captures != 0;
        let is_man = piece.kind == PieceKind::Man;

        if is_capture || is_man {
            self.half_move_clock = 0;
        } else {
            self.half_move_clock += 1;
        }

        if self.board_state.turn == PieceColor::Black {
            self.full_move_number += 1;
        }

        self.history.push(self.board_state);
        self.board_state = self.board_state.apply_move(m);
        self.last_move = Some(m.clone());
        self.is_mid_capture = false;
        self.dragged_piece_idx = None;
        self.selected_square_idx = None;
        
        *self.position_counts.entry(self.board_state).or_insert(0) += 1;

        if m.captures != 0 {
            play_sound_once(&assets.snd_capture);
        } else {
            play_sound_once(&assets.snd_go);
        }

        if m.captures != 0 {
             let next_captures = generate_captures_from_pos(&self.board_state, m.to, self.force_max_capture);
             if !next_captures.is_empty() {
                 self.board_state.turn = !self.board_state.turn;
                 self.selected_square_idx = Some(m.to.trailing_zeros() as usize);
                 self.legal_moves = next_captures;
                 self.is_mid_capture = true;
             }
        }
    }

    fn is_game_over(&self) -> Option<Option<PieceColor>> {
        let white_can_move = !generate_all_moves(&self.board_state, self.force_max_capture).is_empty() && self.board_state.turn == PieceColor::White;
        let black_can_move = !generate_all_moves(&self.board_state, self.force_max_capture).is_empty() && self.board_state.turn == PieceColor::Black;

        if self.board_state.white_pieces() == 0 { return Some(Some(PieceColor::Black)); }
        if self.board_state.black_pieces() == 0 { return Some(Some(PieceColor::White)); }
        
        if !white_can_move && self.board_state.turn == PieceColor::White { return Some(Some(PieceColor::Black)); }
        if !black_can_move && self.board_state.turn == PieceColor::Black { return Some(Some(PieceColor::White)); }

        if let Some(&count) = self.position_counts.get(&self.board_state) {
            if count >= 3 { return Some(None); }
        }

        if self.half_move_clock >= 100 {
            return Some(None);
        }

        None
    }
}

// --- MOVE GENERATION ---
fn generate_all_moves(state: &BitboardState, force_max: bool) -> Vec<Move> {
    let captures = generate_captures(state, force_max);
    if !captures.is_empty() { return captures; }
    generate_simple_moves(state)
}

fn generate_simple_moves(state: &BitboardState) -> Vec<Move> {
    let mut moves = Vec::with_capacity(32);
    let (men, kings) = match state.turn {
        PieceColor::White => (state.white_men, state.white_kings),
        PieceColor::Black => (state.black_men, state.black_kings),
    };
    let empty = !state.all_pieces() & DARK_SQUARES;
    
    if state.turn == PieceColor::White {
        let mut to = (men << 9) & empty & NOT_A_FILE;
        while to != 0 { let t = 1u64 << to.trailing_zeros(); moves.push(Move { from: t >> 9, to: t, captures: 0 }); to &= to - 1; }
        to = (men << 7) & empty & NOT_H_FILE;
        while to != 0 { let t = 1u64 << to.trailing_zeros(); moves.push(Move { from: t >> 7, to: t, captures: 0 }); to &= to - 1; }
    } else {
        let mut to = (men >> 9) & empty & NOT_H_FILE;
        while to != 0 { let t = 1u64 << to.trailing_zeros(); moves.push(Move { from: t << 9, to: t, captures: 0 }); to &= to - 1; }
        to = (men >> 7) & empty & NOT_A_FILE;
        while to != 0 { let t = 1u64 << to.trailing_zeros(); moves.push(Move { from: t << 7, to: t, captures: 0 }); to &= to - 1; }
    }

    let mut k = kings;
    while k != 0 {
        let from = 1u64 << k.trailing_zeros();
        let dirs = [9, 7, -9, -7];
        for &d in &dirs {
            let to = if d > 0 { from << d } else { from >> -d };
            let mask = if d == 9 || d == -7 { NOT_A_FILE } else { NOT_H_FILE };
            if (to & empty & mask) != 0 {
                moves.push(Move { from, to, captures: 0 });
            }
        }
        k &= k - 1;
    }
    moves
}

fn generate_captures(state: &BitboardState, force_max: bool) -> Vec<Move> {
    let mut moves = Vec::new();
    let pieces = if state.turn == PieceColor::White { state.white_pieces() } else { state.black_pieces() };
    let mut p = pieces;
    while p != 0 {
        let from = 1u64 << p.trailing_zeros();
        find_capture_paths_recursive(state, from, from, 0, &mut moves);
        p &= p - 1;
    }
    if moves.is_empty() { return vec![]; }
    
    if force_max {
        let max_cap = moves.iter().map(|m| m.captures.count_ones()).max().unwrap_or(0);
        moves.into_iter().filter(|m| m.captures.count_ones() == max_cap).collect()
    } else {
        moves
    }
}

fn generate_captures_from_pos(state: &BitboardState, pos: Bitboard, force_max: bool) -> Vec<Move> {
    let mut moves = Vec::new();
    find_capture_paths_recursive(state, pos, pos, 0, &mut moves);
    if moves.is_empty() { return vec![]; }
    
    if force_max {
        let max_cap = moves.iter().map(|m| m.captures.count_ones()).max().unwrap_or(0);
        moves.into_iter().filter(|m| m.captures.count_ones() == max_cap).collect()
    } else {
        moves
    }
}

fn find_capture_paths_recursive(state: &BitboardState, start: Bitboard, current: Bitboard, captured: Bitboard, paths: &mut Vec<Move>) {
    let mut found = false;
    let is_king = (state.white_kings | state.black_kings) & start != 0;
    let color = if (state.white_pieces() & start) != 0 { PieceColor::White } else { PieceColor::Black };
    let opp = if color == PieceColor::White { state.black_pieces() } else { state.white_pieces() };
    let valid_opp = opp & !captured;

    let dirs = if is_king { vec![9, 7, -9, -7] } else if color == PieceColor::White { vec![9, 7] } else { vec![-9, -7] };

    for &d in &dirs {
        let jump_over = if d > 0 { current << d } else { current >> -d };
        let land = if d > 0 { jump_over << d } else { jump_over >> -d };
        let mask1 = if d == 9 || d == -7 { NOT_A_FILE } else { NOT_H_FILE };
        let mask2 = mask1;

        if (jump_over & valid_opp & mask1) != 0 && (land & !state.all_pieces() & DARK_SQUARES & mask2) != 0 {
            found = true;
            find_capture_paths_recursive(state, start, land, captured | jump_over, paths);
        }
    }

    if !found && captured != 0 {
        paths.push(Move { from: start, to: current, captures: captured });
    }
}

// --- EVALUATION & AI ---
fn evaluate_board(state: &BitboardState) -> i32 {
    let mut score = 0;
    
    let wm = state.white_men.count_ones() as i32;
    let bm = state.black_men.count_ones() as i32;
    let wk = state.white_kings.count_ones() as i32;
    let bk = state.black_kings.count_ones() as i32;

    // Material Score
    score += 100 * (wm - bm);
    score += 350 * (wk - bk); 

    let w_total = wm + wk;
    let b_total = bm + bk;
    
    // Trading Incentive (Reward exchanging when ahead)
    if w_total > b_total {
        score += (24 - b_total) * 10; 
    } else if b_total > w_total {
        score -= (24 - w_total) * 10;
    }

    // Endgame Corner Control
    if w_total + b_total <= 5 {
        if (state.white_kings & CORNER_TL) != 0 { score += 30; }
        if (state.white_kings & CORNER_BR) != 0 { score += 30; }
        if (state.black_kings & CORNER_TL) != 0 { score -= 30; }
        if (state.black_kings & CORNER_BR) != 0 { score -= 30; }
    }

    // Aggression / Distance Score (King Endgames Only)
    let is_king_endgame = (wm == 0 && bm == 0) || (w_total + b_total <= 6);
    
    if is_king_endgame && w_total != b_total {
        let mut w_positions = Vec::with_capacity(12);
        let mut b_positions = Vec::with_capacity(12);
        
        let mut wp = state.white_pieces();
        while wp != 0 {
            let idx = wp.trailing_zeros();
            w_positions.push((7 - (idx as i32 / 8), idx as i32 % 8)); 
            wp &= wp - 1;
        }
        
        let mut bp = state.black_pieces();
        while bp != 0 {
            let idx = bp.trailing_zeros();
            b_positions.push((7 - (idx as i32 / 8), idx as i32 % 8));
            bp &= bp - 1;
        }

        if w_total > b_total && !b_positions.is_empty() {
            for &(wr, wc) in &w_positions {
                let mut min_dist = 100;
                for &(br, bc) in &b_positions {
                    let dist = max((wr - br).abs(), (wc - bc).abs());
                    if dist < min_dist { min_dist = dist; }
                }
                if min_dist < 10 {
                    score += (10 - min_dist) * 10;
                }
            }
        }
        else if b_total > w_total && !w_positions.is_empty() {
            for &(br, bc) in &b_positions {
                let mut min_dist = 100;
                for &(wr, wc) in &w_positions {
                    let dist = max((br - wr).abs(), (bc - wc).abs());
                    if dist < min_dist { min_dist = dist; }
                }
                if min_dist < 10 {
                    score -= (10 - min_dist) * 10;
                }
            }
        }
    }

    let empty = !state.all_pieces() & DARK_SQUARES;
    
    // Mobility
    let wm_mob = ((state.white_men << 9) & empty & NOT_A_FILE).count_ones() + ((state.white_men << 7) & empty & NOT_H_FILE).count_ones();
    let bm_mob = ((state.black_men >> 9) & empty & NOT_H_FILE).count_ones() + ((state.black_men >> 7) & empty & NOT_A_FILE).count_ones();
    score += 5 * (wm_mob as i32 - bm_mob as i32);

    // Center Control
    let w_center = (state.white_pieces() & CENTER_MASK).count_ones() as i32;
    let b_center = (state.black_pieces() & CENTER_MASK).count_ones() as i32;
    score += 8 * (w_center - b_center);

    // Structure
    if (state.white_men & WHITE_BRIDGE) == WHITE_BRIDGE { score += 30; }
    if (state.black_men & BLACK_BRIDGE) == BLACK_BRIDGE { score -= 30; }
    let w_dog = (state.white_men & DOG_HOLES).count_ones() as i32;
    let b_dog = (state.black_men & DOG_HOLES).count_ones() as i32;
    score -= 25 * (w_dog - b_dog);

    // Runaway Checkers
    let mut w_men = state.white_men;
    while w_men != 0 {
        let idx = w_men.trailing_zeros();
        let rank = idx / 8;
        score += rank as i32 * 4; 
        if rank >= 4 {
            let mut mask = 0u64;
            for r in (rank+1)..8 { mask |= 0xFF << (r*8); }
            if (state.black_pieces() & mask) == 0 { score += 100; }
        }
        w_men &= w_men - 1;
    }
    let mut b_men = state.black_men;
    while b_men != 0 {
        let idx = b_men.trailing_zeros();
        let rank = 7 - (idx / 8);
        score -= rank as i32 * 4;
        if rank >= 4 {
            let mut mask = 0u64;
            for r in 0..(7-rank) { mask |= 0xFF << (r*8); }
            if (state.white_pieces() & mask) == 0 { score -= 100; }
        }
        b_men &= b_men - 1;
    }
    score
}

fn quiescence(state: &BitboardState, mut alpha: i32, mut beta: i32, maximizing: bool, start_time: Instant, nodes: &mut u64, time_limit_ms: u128, force_max: bool) -> Option<i32> {
    *nodes += 1;
    if (*nodes & 2047) == 0 && start_time.elapsed().as_millis() > time_limit_ms { return None; }
    let stand_pat = evaluate_board(state);
    if maximizing {
        if stand_pat >= beta { return Some(beta); }
        if stand_pat > alpha { alpha = stand_pat; }
    } else {
        if stand_pat <= alpha { return Some(alpha); }
        if stand_pat < beta { beta = stand_pat; }
    }
    let captures = generate_captures(state, force_max);
    let moves_to_search = if !captures.is_empty() { captures } else {
        let mut quiet = generate_simple_moves(state);
        quiet.retain(|m| {
            if state.turn == PieceColor::White { (m.to & WHITE_PROMOTION_RANK) != 0 } else { (m.to & BLACK_PROMOTION_RANK) != 0 }
        });
        quiet
    };
    if !moves_to_search.is_empty() {
        let mut best_val = if maximizing { -i32::MAX } else { i32::MAX };
        for m in moves_to_search {
            let next_state = state.apply_move(&m);
            let val = quiescence(&next_state, alpha, beta, !maximizing, start_time, nodes, time_limit_ms, force_max)?;
            if maximizing {
                best_val = max(best_val, val);
                alpha = max(alpha, val);
                if beta <= alpha { break; }
            } else {
                best_val = min(best_val, val);
                beta = min(beta, val);
                if beta <= alpha { break; }
            }
        }
        return Some(best_val);
    }
    Some(stand_pat)
}

fn minimax(game: &Game, state: BitboardState, depth: i32, mut alpha: i32, mut beta: i32, maximizing: bool, start_time: Instant, nodes: &mut u64, time_limit_ms: u128) -> Option<i32> {
    *nodes += 1;
    if (*nodes & 2047) == 0 && start_time.elapsed().as_millis() > time_limit_ms { return None; }
    if depth == 0 { return quiescence(&state, alpha, beta, maximizing, start_time, nodes, time_limit_ms, game.force_max_capture); }
    let mut moves = generate_all_moves(&state, game.force_max_capture);
    if moves.is_empty() { return Some(if maximizing { -100000 + (20 - depth) } else { 100000 - (20 - depth) }); }
    moves.sort_by(|a, b| {
        let cap_a = a.captures.count_ones();
        let cap_b = b.captures.count_ones();
        if cap_a != cap_b { return cap_b.cmp(&cap_a); }
        let is_prom_a = (a.to & (WHITE_PROMOTION_RANK | BLACK_PROMOTION_RANK)) != 0;
        let is_prom_b = (b.to & (WHITE_PROMOTION_RANK | BLACK_PROMOTION_RANK)) != 0;
        if is_prom_a && !is_prom_b { return std::cmp::Ordering::Less; }
        if !is_prom_a && is_prom_b { return std::cmp::Ordering::Greater; }
        std::cmp::Ordering::Equal
    });
    if maximizing {
        let mut max_eval = -i32::MAX;
        for m in moves {
            let next_state = state.apply_move(&m);
            if let Some(&count) = game.position_counts.get(&next_state) {
                if count >= 1 { max_eval = max(max_eval, 0); alpha = max(alpha, 0); if beta <= alpha { break; } continue; }
            }
            let eval = minimax(game, next_state, depth - 1, alpha, beta, false, start_time, nodes, time_limit_ms)?;
            max_eval = max(max_eval, eval);
            alpha = max(alpha, eval);
            if beta <= alpha { break; }
        }
        Some(max_eval)
    } else {
        let mut min_eval = i32::MAX;
        for m in moves {
            let next_state = state.apply_move(&m);
            if let Some(&count) = game.position_counts.get(&next_state) {
                if count >= 1 { min_eval = min(min_eval, 0); beta = min(beta, 0); if beta <= alpha { break; } continue; }
            }
            let eval = minimax(game, next_state, depth - 1, alpha, beta, true, start_time, nodes, time_limit_ms)?;
            min_eval = min(min_eval, eval);
            beta = min(beta, eval);
            if beta <= alpha { break; }
        }
        Some(min_eval)
    }
}

fn find_best_move(game: &mut Game) -> Option<Move> {
    let mut moves = generate_all_moves(&game.board_state, game.force_max_capture);
    if moves.is_empty() { return None; }
    if moves.len() == 1 { return Some(moves[0].clone()); }
    let start_time = Instant::now();
    let maximizing = game.board_state.turn == PieceColor::White;
    let mut best_move_global = moves[0].clone();
    let mut _total_nodes = 0u64;
    
    // Convert seconds to ms
    let time_limit_ms = (game.ai_time_limit * 1000.0) as u128;

    moves.sort_by(|a, b| b.captures.count_ones().cmp(&a.captures.count_ones()));
    for depth in 1..=20 {
        let mut current_depth_best_move = None;
        let mut best_val = if maximizing { -i32::MAX } else { i32::MAX };
        let mut alpha = -i32::MAX;
        let mut beta = i32::MAX;
        let mut time_out = false;
        let mut nodes_at_depth = 0;
        for m in &moves {
            let next_state = game.board_state.apply_move(m);
            let mut is_repetition = false;
            if let Some(&count) = game.position_counts.get(&next_state) { if count >= 1 { is_repetition = true; } }
            let val_opt = if is_repetition { Some(0) } else { minimax(game, next_state, depth - 1, alpha, beta, !maximizing, start_time, &mut nodes_at_depth, time_limit_ms) };
            match val_opt {
                Some(val) => {
                    if maximizing {
                        if val > best_val { best_val = val; current_depth_best_move = Some(m.clone()); }
                        alpha = max(alpha, val);
                    } else {
                        if val < best_val { best_val = val; current_depth_best_move = Some(m.clone()); }
                        beta = min(beta, val);
                    }
                },
                None => { time_out = true; break; }
            }
        }
        if time_out { break; }
        _total_nodes += nodes_at_depth;
        if let Some(bm) = current_depth_best_move {
            best_move_global = bm;
            if best_val.abs() > 90000 { break; }
        }
    }
    Some(best_move_global)
}

// --- GUI & MAIN ---

fn window_conf() -> Conf { Conf { window_title: "Checkers: Deluxe Edition".to_owned(), window_width: WINDOW_W as i32, window_height: WINDOW_H as i32, ..Default::default() } }

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = Game::new();
    let assets = GameAssets::load().await;

    loop {
        match game.mode {
            Mode::Menu => {
                draw_texture_ex(&assets.tex_bg, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: Some(vec2(WINDOW_W, WINDOW_H)), ..Default::default() });
                draw_menu(&mut game, &assets).await
            },
            Mode::Settings => {
                draw_texture_ex(&assets.tex_bg_settings, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: Some(vec2(WINDOW_W, WINDOW_H)), ..Default::default() });
                draw_settings(&mut game, &assets).await
            },
            Mode::SideSelection => {
                draw_texture_ex(&assets.tex_bg, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: Some(vec2(WINDOW_W, WINDOW_H)), ..Default::default() });
                draw_side_selection(&mut game, &assets).await
            },
            Mode::PvP | Mode::PvA | Mode::AvA => {
                draw_texture_ex(&assets.tex_bg, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: Some(vec2(WINDOW_W, WINDOW_H)), ..Default::default() });
                update_game(&mut game, &assets);
                draw_game(&mut game, &assets).await;
            },
        }
        next_frame().await
    }
}

async fn draw_menu(game: &mut Game, assets: &GameAssets) {
    let cx = WINDOW_W / 2.0;
    draw_text_ex("CHECKERS", cx - 140.0, 150.0, TextParams { font_size: 70, color: GOLD, ..Default::default() });
    
    let btn_h = 60.0;
    let btn_w = 300.0;
    let start_y = 280.0;

    let buttons = [
        ("Player vs Player", Mode::PvP),
        ("Play vs AI", Mode::SideSelection),
        ("AI vs AI", Mode::AvA),
        ("Settings", Mode::Settings),
    ];

    for (i, (text, mode)) in buttons.iter().enumerate() {
        let y = start_y + i as f32 * (btn_h + 20.0);
        let rect = Rect::new(cx - btn_w / 2.0, y, btn_w, btn_h);
        let hovered = rect.contains(mouse_position().into());
        
        draw_texture_ex(&assets.tex_ui, rect.x, rect.y, if hovered { WHITE } else { LIGHTGRAY }, DrawTextureParams {
            dest_size: Some(vec2(rect.w, rect.h)),
            ..Default::default()
        });
        
        let text_dim = measure_text(text, None, 30, 1.0);
        draw_text(text, rect.x + (rect.w - text_dim.width) / 2.0, rect.y + 40.0, 30.0, WHITE);

        if is_mouse_button_pressed(MouseButton::Left) && hovered {
            if *mode == Mode::AvA {
                game.mode = Mode::AvA;
                game.reset_board();
                game.ai_white = true;
                game.ai_black = true;
            } else if *mode == Mode::PvP {
                game.mode = Mode::PvP;
                game.reset_board();
                game.ai_white = false;
                game.ai_black = false;
            } else if *mode == Mode::Settings {
                game.mode = Mode::Settings;
            } else {
                game.mode = Mode::SideSelection;
            }
        }
    }
}

async fn draw_settings(game: &mut Game, assets: &GameAssets) {
    let cx = WINDOW_W / 2.0;
    draw_text_ex("SETTINGS", cx - 120.0, 100.0, TextParams { font_size: 60, color: WHITE, ..Default::default() });

    let start_y = 200.0;
    
    // 1. Force Max Capture Toggle
    let toggle_text = format!("Force Max Capture: {}", if game.force_max_capture { "ON" } else { "OFF" });
    let toggle_rect = Rect::new(cx - 200.0, start_y, 400.0, 60.0);
    let t_hover = toggle_rect.contains(mouse_position().into());
    
    draw_texture_ex(&assets.tex_ui, toggle_rect.x, toggle_rect.y, if t_hover { WHITE } else { LIGHTGRAY }, DrawTextureParams { dest_size: Some(vec2(toggle_rect.w, toggle_rect.h)), ..Default::default() });
    let t_dim = measure_text(&toggle_text, None, 30, 1.0);
    draw_text(&toggle_text, toggle_rect.x + (toggle_rect.w - t_dim.width)/2.0, toggle_rect.y + 40.0, 30.0, WHITE);
    
    if is_mouse_button_pressed(MouseButton::Left) && t_hover {
        game.force_max_capture = !game.force_max_capture;
    }

    // 2. AI Time Limit
    let time_y = start_y + 100.0;
    let time_text = format!("AI Time: {:.1} s", game.ai_time_limit);
    draw_text(&time_text, cx - 100.0, time_y + 40.0, 40.0, WHITE);

    // Minus Button
    let minus_rect = Rect::new(cx - 180.0, time_y, 60.0, 60.0);
    let m_hover = minus_rect.contains(mouse_position().into());
    draw_texture_ex(&assets.tex_ui, minus_rect.x, minus_rect.y, if m_hover { WHITE } else { LIGHTGRAY }, DrawTextureParams { dest_size: Some(vec2(minus_rect.w, minus_rect.h)), ..Default::default() });
    draw_text("-", minus_rect.x + 22.0, minus_rect.y + 40.0, 40.0, WHITE);
    if is_mouse_button_pressed(MouseButton::Left) && m_hover {
        game.ai_time_limit = (game.ai_time_limit - 0.1).max(0.1);
    }

    // Plus Button
    let plus_rect = Rect::new(cx + 120.0, time_y, 60.0, 60.0);
    let p_hover = plus_rect.contains(mouse_position().into());
    draw_texture_ex(&assets.tex_ui, plus_rect.x, plus_rect.y, if p_hover { WHITE } else { LIGHTGRAY }, DrawTextureParams { dest_size: Some(vec2(plus_rect.w, plus_rect.h)), ..Default::default() });
    draw_text("+", plus_rect.x + 18.0, plus_rect.y + 40.0, 40.0, WHITE);
    if is_mouse_button_pressed(MouseButton::Left) && p_hover {
        game.ai_time_limit = (game.ai_time_limit + 0.1).min(3.0);
    }

    // 3. Bottom Buttons (Menu & Quit)
    let btn_w = 120.0;
    let btn_h = 40.0;
    let bottom_y = WINDOW_H - 150.0;

    // Menu Button (Red)
    let menu_rect = Rect::new(cx - btn_w/2.0, bottom_y, btn_w, btn_h);
    let menu_hover = menu_rect.contains(mouse_position().into());
    draw_rectangle(menu_rect.x, menu_rect.y, menu_rect.w, menu_rect.h, if menu_hover { RED } else { MAROON });
    draw_rectangle_lines(menu_rect.x, menu_rect.y, menu_rect.w, menu_rect.h, 2.0, WHITE);
    draw_text("Menu", menu_rect.x + 30.0, menu_rect.y + 28.0, 24.0, WHITE);
    
    if is_mouse_button_pressed(MouseButton::Left) && menu_hover {
        game.mode = Mode::Menu;
    }

    // Quit Button (Red)
    let quit_rect = Rect::new(cx - btn_w/2.0, bottom_y + 60.0, btn_w, btn_h);
    let quit_hover = quit_rect.contains(mouse_position().into());
    draw_rectangle(quit_rect.x, quit_rect.y, quit_rect.w, quit_rect.h, if quit_hover { RED } else { MAROON });
    draw_rectangle_lines(quit_rect.x, quit_rect.y, quit_rect.w, quit_rect.h, 2.0, WHITE);
    draw_text("Quit", quit_rect.x + 35.0, quit_rect.y + 28.0, 24.0, WHITE);

    if is_mouse_button_pressed(MouseButton::Left) && quit_hover {
        process::exit(0);
    }
}

async fn draw_side_selection(game: &mut Game, assets: &GameAssets) {
    let cx = WINDOW_W / 2.0;
    draw_text_ex("Choose Your Side", cx - 160.0, 150.0, TextParams { font_size: 50, color: WHITE, ..Default::default() });

    let btn_w = 200.0;
    let btn_h = 80.0;
    let y = 300.0;

    // White Button
    let white_rect = Rect::new(cx - btn_w - 20.0, y, btn_w, btn_h);
    let w_hover = white_rect.contains(mouse_position().into());
    draw_texture_ex(&assets.tex_ui, white_rect.x, white_rect.y, if w_hover { WHITE } else { LIGHTGRAY }, DrawTextureParams { dest_size: Some(vec2(btn_w, btn_h)), ..Default::default() });
    draw_text("WHITE", white_rect.x + 50.0, white_rect.y + 50.0, 40.0, WHITE);

    // Black Button
    let black_rect = Rect::new(cx + 20.0, y, btn_w, btn_h);
    let b_hover = black_rect.contains(mouse_position().into());
    draw_texture_ex(&assets.tex_ui, black_rect.x, black_rect.y, if b_hover { WHITE } else { LIGHTGRAY }, DrawTextureParams { dest_size: Some(vec2(btn_w, btn_h)), ..Default::default() });
    draw_text("BLACK", black_rect.x + 50.0, black_rect.y + 50.0, 40.0, WHITE);

    if is_mouse_button_pressed(MouseButton::Left) {
        if w_hover {
            game.mode = Mode::PvA;
            game.reset_board();
            game.ai_white = false;
            game.ai_black = true;
            game.flipped = false;
        } else if b_hover {
            game.mode = Mode::PvA;
            game.reset_board();
            game.ai_white = true;
            game.ai_black = false;
            game.flipped = true;
        }
    }
    
    // Back Button
    let back_rect = Rect::new(cx - 60.0, 500.0, 120.0, 40.0);
    draw_texture_ex(&assets.tex_ui, back_rect.x, back_rect.y, LIGHTGRAY, DrawTextureParams { dest_size: Some(vec2(back_rect.w, back_rect.h)), ..Default::default() });
    draw_text("Back", back_rect.x + 30.0, back_rect.y + 28.0, 24.0, WHITE);
    if is_mouse_button_pressed(MouseButton::Left) && back_rect.contains(mouse_position().into()) {
        game.mode = Mode::Menu;
    }
}

fn update_game(game: &mut Game, assets: &GameAssets) {
    // 1. Update Animation
    if game.animation.is_some() {
        game.update_animation(assets);
        return; // Block other updates while animating
    }

    // 2. Check Game Over
    if game.is_game_over().is_some() { return; }

    // 3. AI Logic
    let is_ai_turn = (game.ai_white && game.board_state.turn == PieceColor::White) || (game.ai_black && game.board_state.turn == PieceColor::Black);
    
    if is_ai_turn {
        if !game.ai_should_move {
            game.ai_should_move = true;
        } else {
            if let Some(best) = find_best_move(game) {
                game.start_move_animation(best);
            }
            game.ai_should_move = false;
        }
        return; // Return immediately to let the animation start rendering in the next frame
    }

    // 4. Player Input (Drag & Drop + Click)
    let ox = (WINDOW_W - BOARD_PIXELS) / 2.0;
    let oy = (WINDOW_H - BOARD_PIXELS) / 2.0;
    let (mx, my) = mouse_position();
    let flipped = game.flipped; 

    let get_board_pos = |mx: f32, my: f32, is_flipped: bool| -> Option<(i32, i32)> {
        if mx >= ox && mx < ox + BOARD_PIXELS && my >= oy && my < oy + BOARD_PIXELS {
            let mut c = ((mx - ox) / SQUARE) as i32;
            let mut r = ((my - oy) / SQUARE) as i32;
            if is_flipped { c = 7 - c; r = 7 - r; }
            Some((r, c))
        } else {
            None
        }
    };

    if is_mouse_button_pressed(MouseButton::Left) {
        if let Some((r, c)) = get_board_pos(mx, my, flipped) {
            let clicked_idx = (7 - r) * 8 + c;
            let clicked_bit = 1u64 << clicked_idx;

            // Check if clicking a valid move target (Click-to-Move)
            let mut moved = false;
            if let Some(sel_idx) = game.selected_square_idx {
                let move_to_apply = game.legal_moves.iter()
                    .find(|m| m.from.trailing_zeros() as usize == sel_idx && m.to == clicked_bit)
                    .cloned();

                if let Some(m) = move_to_apply {
                    // Use start_move_animation instead of finalize_move for clicks
                    game.start_move_animation(m);
                    moved = true;
                }
            }

            if !moved && !game.is_mid_capture {
                if let Some(p) = game.board_state.get_piece_at(r, c) {
                    if p.color == game.board_state.turn {
                        game.selected_square_idx = Some(clicked_idx as usize);
                        game.dragged_piece_idx = Some(clicked_idx as usize); 
                        game.legal_moves = generate_all_moves(&game.board_state, game.force_max_capture);
                    } else {
                        game.selected_square_idx = None;
                    }
                } else {
                    game.selected_square_idx = None;
                }
            }
        }
    }

    if is_mouse_button_released(MouseButton::Left) {
        if let Some(drag_idx) = game.dragged_piece_idx {
            if let Some((r, c)) = get_board_pos(mx, my, flipped) {
                let target_idx = (7 - r) * 8 + c;
                let target_bit = 1u64 << target_idx;
                
                let move_to_apply = game.legal_moves.iter()
                    .find(|m| m.from.trailing_zeros() as usize == drag_idx && m.to == target_bit)
                    .cloned();

                if let Some(m) = move_to_apply {
                    // For drag and drop, we finalize immediately (snap)
                    game.finalize_move(&m, assets);
                }
            }
            game.dragged_piece_idx = None; 
        }
    }
}

async fn draw_game(game: &mut Game, assets: &GameAssets) {
    let ox = (WINDOW_W - BOARD_PIXELS) / 2.0;
    let oy = (WINDOW_H - BOARD_PIXELS) / 2.0;

    // Draw Board Background
    draw_rectangle(ox - 5.0, oy - 5.0, BOARD_PIXELS + 10.0, BOARD_PIXELS + 10.0, Color::from_rgba(60, 40, 10, 255));

    // Draw Squares
    for r in 0..BOARD_SIZE {
        for c in 0..BOARD_SIZE {
            let draw_r = if game.flipped { 7 - r } else { r };
            let draw_c = if game.flipped { 7 - c } else { c };
            let x = ox + draw_c as f32 * SQUARE;
            let y = oy + draw_r as f32 * SQUARE;
            
            let dark = (r + c) % 2 == 1;
            let color = if dark { Color::from_rgba(100, 50, 10, 200) } else { Color::from_rgba(220, 190, 150, 200) };
            draw_rectangle(x, y, SQUARE, SQUARE, color);
            
            // Coordinates (Hidden if flipped)
            if !game.flipped {
                if c == 0 { 
                    let label = format!("{}", 8-r);
                    draw_text(&label, x - 15.0, y + 20.0, 15.0, WHITE); 
                }
                if r == 7 { 
                    let label = format!("{}", (b'a' + c as u8) as char);
                    draw_text(&label, x + SQUARE - 15.0, y + SQUARE + 15.0, 15.0, WHITE); 
                }
            }
        }
    }

    // Highlight Last Move
    if let Some(m) = &game.last_move {
        let f = m.from.trailing_zeros();
        let t = m.to.trailing_zeros();
        for &idx in &[f, t] {
            let (r, c) = (7 - idx / 8, idx % 8);
            let (dr, dc) = if game.flipped { (7-r, 7-c) } else { (r, c) };
            draw_rectangle(ox + dc as f32 * SQUARE, oy + dr as f32 * SQUARE, SQUARE, SQUARE, Color::from_rgba(255, 255, 0, 80));
        }
    }

    // Highlight Selected & Legal Moves
    if let Some(sel_idx) = game.selected_square_idx {
        let (r, c) = (7 - sel_idx as i32 / 8, sel_idx as i32 % 8);
        let (dr, dc) = if game.flipped { (7-r, 7-c) } else { (r, c) };
        draw_rectangle_lines(ox + dc as f32 * SQUARE, oy + dr as f32 * SQUARE, SQUARE, SQUARE, 3.0, RED);

        for m in &game.legal_moves {
            if m.from.trailing_zeros() as usize == sel_idx {
                let t = m.to.trailing_zeros();
                let (tr, tc) = (7 - t / 8, t % 8);
                let (dtr, dtc) = if game.flipped { (7-tr, 7-tc) } else { (tr, tc) };
                draw_circle(ox + dtc as f32 * SQUARE + SQUARE/2.0, oy + dtr as f32 * SQUARE + SQUARE/2.0, 10.0, Color::from_rgba(0, 255, 0, 150));
            }
        }
    }

    // Helper to draw a piece
    let draw_piece = |x: f32, y: f32, p: Piece| {
        let tex = match p.color {
            PieceColor::White => &assets.tex_pw,
            PieceColor::Black => &assets.tex_pb,
        };
        
        // FIX: Make white pieces 5% taller (1.2x height)
        let (w_scale, h_scale) = if p.color == PieceColor::White { (0.95, 1.05) } else { (1.0, 1.0) };
        
        let base_size = SQUARE * 0.9;
        let w = base_size * w_scale;
        let h = base_size * h_scale;
        
        // Center the piece in the square
        let x_off = (SQUARE - w) / 2.0;
        let y_off = (SQUARE - h) / 2.0;

        draw_texture_ex(tex, x + x_off, y + y_off, WHITE, DrawTextureParams {
            dest_size: Some(vec2(w, h)),
            ..Default::default()
        });

        if p.kind == PieceKind::King {
            let k_size = base_size * 0.6;
            let k_offset_x = (SQUARE - k_size) / 2.0;
            let k_offset_y = (SQUARE - k_size) / 2.0;
            draw_texture_ex(&assets.tex_king, x + k_offset_x, y + k_offset_y, WHITE, DrawTextureParams {
                dest_size: Some(vec2(k_size, k_size)),
                ..Default::default()
            });
        }
    };

    // Draw Static Pieces
    for r in 0..BOARD_SIZE {
        for c in 0..BOARD_SIZE {
            let idx = (7 - r) * 8 + c;
            
            if game.dragged_piece_idx == Some(idx) { continue; }
            if let Some(anim) = &game.animation {
                if anim.move_data.from.trailing_zeros() as usize == idx { continue; }
            }

            if let Some(p) = game.board_state.get_piece_at(r as i32, c as i32) {
                let draw_r = if game.flipped { 7 - r } else { r };
                let draw_c = if game.flipped { 7 - c } else { c };
                let x = ox + draw_c as f32 * SQUARE;
                let y = oy + draw_r as f32 * SQUARE;
                draw_piece(x, y, p);
            }
        }
    }

    // Draw Animated Piece
    if let Some(anim) = &game.animation {
        let curr_pos = anim.start_pos.lerp(anim.end_pos, anim.progress);
        draw_piece(curr_pos.x, curr_pos.y, anim.piece);
    }

    // Draw Dragged Piece
    if let Some(idx) = game.dragged_piece_idx {
        if let Some(p) = game.board_state.get_piece_at_bit(1u64 << idx) {
            let (mx, my) = mouse_position();
            draw_piece(mx - SQUARE/2.0, my - SQUARE/2.0, p);
        }
    }

    // UI Buttons
    let btn_w = 120.0;
    let btn_h = 40.0;
    
    let draw_btn = |text: &str, x: f32, y: f32| -> bool {
        let rect = Rect::new(x, y, btn_w, btn_h);
        let hovered = rect.contains(mouse_position().into());
        draw_texture_ex(&assets.tex_ui, rect.x, rect.y, if hovered { WHITE } else { LIGHTGRAY }, DrawTextureParams { dest_size: Some(vec2(rect.w, rect.h)), ..Default::default() });
        draw_text(text, rect.x + 20.0, rect.y + 28.0, 24.0, WHITE);
        is_mouse_button_pressed(MouseButton::Left) && hovered
    };

    if draw_btn("MENU", 20.0, WINDOW_H - 60.0) { game.mode = Mode::Menu; }
    if draw_btn("UNDO", 160.0, WINDOW_H - 60.0) { game.undo(); }
    if draw_btn("FLIP", 300.0, WINDOW_H - 60.0) { game.flipped = !game.flipped; }

    // Game Over Overlay
    if let Some(winner) = game.is_game_over() {
        if !game.game_over_printed {
            game.game_over_printed = true;
        }
        let text = match winner {
            Some(PieceColor::White) => "WHITE WINS!",
            Some(PieceColor::Black) => "BLACK WINS!",
            None => "DRAW!",
        };
        let dim = measure_text(text, None, 60, 1.0);
        draw_rectangle(WINDOW_W/2.0 - dim.width/2.0 - 20.0, WINDOW_H/2.0 - 40.0, dim.width + 40.0, 80.0, BLACK);
        draw_text(text, WINDOW_W/2.0 - dim.width/2.0, WINDOW_H/2.0 + 15.0, 60.0, GOLD);
    }
}