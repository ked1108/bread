---
title: Writing a Chess Engine
date: 2025-11-16
tags:
  - chess
  - engine
  - cpp
slug: chess
---

# Building a Bitboard Chess Engine from Scratch in C++

My friend I often play chess together. While we were on equal footing in the beginning with a more or less equal score, she started to get better and better at it. Soon I realised, so what if I can't beat her? I can build something which is capable of beating her on my behalf. This was my main motivation to build my own Chess Engine because, if I have made the engine myself it does not count as cheating.     
Creating a chess engine is one of those projects that combines algorithm design, optimization, and game theory in a uniquely satisfying way. Over the past few weeks, I built a fully functional chess engine from scratch using C++ and Raylib, complete with legal move generation, an AI opponent, and a graphical interface. I wanted to share some of the things I learnt along the way.

## Why Bitboards?

When I started researching chess engines, I quickly discovered that a lot of implementations use **bitboards** rather than the intuitive 8×8 array representation which was also my initial choice. A bitboard represents the chess board as a 64-bit integer, where each bit corresponds to a square.

```cpp
// White pawns on the starting position
U64 white_pawns = 0x000000000000FF00ULL;
```

This might seem cryptic at first, but bitboards enable lightning-fast operations using bitwise arithmetic. Want to check if a square is occupied? Just test a single bit. Need to find all attacked squares? Apply shift operations and masks.

### The Bitboard Representation

My engine uses 12 bitboards—one for each piece type and color:

```cpp
struct Board {
    U64 pieces[6][2];      // [piece_type][color]
    U64 occupancies[3];    // [WHITE, BLACK, BOTH]
    int side;              // Whose turn
    int enpassant;         // En passant square
    int castle;            // Castling rights
    U64 hash;              // Zobrist hash
};
```

These are some bit manipulation macros which make working with these bitboards simpler:

```cpp
#define set_bit(b, i)   ((b) |= (1ULL << (i)))
#define get_bit(b, i)   ((b) & (1ULL << (i)))
#define clear_bit(b, i) ((b) &= ~(1ULL << (i)))
```

## Move Generation: The Heart of the Engine

Move generation is where bitboards truly shine. The engine needs to generate legal moves incredibly fast. In a typical position, it explores tens of thousands of positions per second during search.

### Leaping Pieces

For knights and kings, I pre-computed attack tables at startup. A knight on e4 can reach eight squares, and we can calculate this once and store it:

```cpp
U64 generate_knight_attacks(int square) {
    U64 attacks = 0ULL;
    U64 knights = 0ULL;
    set_bit(knights, square);
    
    // Apply knight move patterns with file masks to prevent wrapping
    attacks |= (knights << 17) & ~FILE_A;  // Up-up-right
    attacks |= (knights << 15) & ~FILE_H;  // Up-up-left
    attacks |= (knights << 10) & ~FILE_AB; // Up-right-right
    // ... etc
    
    return attacks;
}
```

The file masks (`~FILE_A`, `~FILE_H`) are crucial—they prevent moves from wrapping around the board edges, which was actually a bug I had to debug where knights could illegally jump from g1 to a3 :P

### Sliding Pieces: Magic Bitboards

Sliding pieces (bishops, rooks, queens) were more complex. I implemented **magic bitboards**, the de facto standard used in engines which I got to know from the Chess Programming Wiki, the holy grail of Chess Programming. The algorithm works like this:

1. Mask the relevant occupancy bits (ignore edges)
2. Multiply by a pre-computed "magic number"
3. Shift right to get an index
4. Look up the attack bitboard in a table

```cpp
inline U64 get_rook_attacks(int square, U64 occupancy) {
    occupancy &= ROOK_MAGICS[square].mask;
    occupancy *= ROOK_MAGICS[square].magic;
    occupancy >>= ROOK_MAGICS[square].shift;
    return ROOK_MAGICS[square].attacks[occupancy];
}
```

Finding the magic numbers requires brute force—trying random 64-bit numbers until one works for all possible occupancy combinations. This takes 10-30 seconds at startup, but provides O(1) attack generation during gameplay.

### Legal Move Generation

Generating pseudo-legal moves is straightforward, but we need to filter out moves that leave our king in check:

```cpp
void generate_legal_moves(Board& board, MoveList& list) {
    MoveList pseudo_legal;
    generate_moves(board, pseudo_legal);
    
    list.clear();
    int our_side = board.side;
    
    for (int i = 0; i < pseudo_legal.count; i++) {
        Board temp = board;
        temp.make_move(pseudo_legal.moves[i]);
        
        // Check if our king is under attack after the move
        int king_square = get_LSB(temp.pieces[KING][our_side]);
        if (!temp.is_square_attacked(king_square, !our_side)) {
            list.add(pseudo_legal.moves[i]);
        }
    }
}
```

## The AI: Negamax with Alpha-Beta Pruning

For the AI opponent, I implemented the **negamax algorithm** with alpha-beta pruning. Negamax is an elegant variation of minimax that treats both players symmetrically by negating the evaluation at each level:

```cpp
int negamax(Board& board, int depth, int alpha, int beta) {
    if (depth == 0) {
        return evaluate_position(board);
    }
    
    MoveList moves;
    generate_legal_moves(board, moves);
    
    // Checkmate or stalemate
    if (moves.count == 0) {
        return board.in_check() ? (-CHECKMATE_SCORE - depth) : 0;
    }
    
    order_moves(board, moves);  // Critical for performance
    
    int max_score = -INFINITY_SCORE;
    
    for (int i = 0; i < moves.count; i++) {
        Board temp = board;
        temp.make_move(moves.moves[i]);
        
        int score = -negamax(temp, depth - 1, -beta, -alpha);
        
        max_score = std::max(max_score, score);
        alpha = std::max(alpha, score);
        
        if (alpha >= beta) break;  // Beta cutoff
    }
    
    return max_score;
}
```

### Evaluation Function

The evaluation uses piece-square tables—bonuses for pieces on good squares. For example, knights are valued more in the center:

```cpp
const int KNIGHT_TABLE[64] = {
    -50,-40,-30,-30,-30,-30,-40,-50,
    -40,-20,  0,  0,  0,  0,-20,-40,
    -30,  0, 10, 15, 15, 10,  0,-30,
    -30,  5, 15, 20, 20, 15,  5,-30,
    // ... etc
};
```

### Move Ordering

Alpha-beta pruning's effectiveness depends heavily on move ordering. I implemented MVV-LVA (Most Valuable Victim - Least Valuable Attacker) for captures:

```cpp
int score_move(Board& board, Move move) {
    int score = 0;
    
    if (is_capture(move)) {
        score = 10 * victim_value - attacker_value;
    }
    
    if (is_promotion(move)) {
        score += 800;
    }
    
    return score;
}
```

Good move ordering improves search speed by 5-10x through better pruning.

## Zobrist Hashing for Repetition Detection

Chess has a threefold repetition rule—if the same position occurs three times, it's a draw. Rather than storing entire board positions, I used **Zobrist hashing**:

```cpp
U64 compute_hash(Board& board) {
    U64 hash = 0ULL;
    
    for each piece on the board {
        hash ^= ZOBRIST_PIECES[piece][color][square];
    }
    
    hash ^= ZOBRIST_CASTLE[board.castle];
    if (board.enpassant != -1) hash ^= ZOBRIST_ENPASSANT[board.enpassant];
    if (board.side == BLACK) hash ^= ZOBRIST_SIDE;
    
    return hash;
}
```

Each position gets a unique 64-bit fingerprint. Checking for repetition becomes a simple integer comparison instead of comparing entire board states.

## The GUI: Raylib Integration

For the interface, I used Raylib—a simple, lightweight graphics library. The key was separating the engine logic from the UI. I implemented drag-and-drop piece movement with legal move highlighting, and added an evaluation bar showing who's winning.

## Special Moves

Implementing special chess rules required careful attention:

### Castling

```cpp
if (flags == 2) { // Kingside castle
    if (side == WHITE) {
        clear_bit(pieces[ROOK][WHITE], h1);
        set_bit(pieces[ROOK][WHITE], f1);
    } else {
        clear_bit(pieces[ROOK][BLACK], h8);
        set_bit(pieces[ROOK][BLACK], f8);
    }
}
```

### En Passant

The trickiest part was that the captured pawn isn't on the destination square:

```cpp
if (flags == 5) { // En passant
    int captured_square = side == WHITE ? (to - 8) : (to + 8);
    clear_bit(pieces[PAWN][!side], captured_square);
}
```

### Pawn Promotion

I added a dialog overlay for promotion piece selection, displaying all four options (Queen, Rook, Bishop, Knight) with piece sprites from the spritesheet.

## Debugging Challenges

### Knight Wrapping

Knights were making illegal moves like g1 to a3. The problem was incorrect file masks in the attack generation—I needed to mask out the source file, not the destination file, when shifting bits.

### Repetition False Positives

Initially, position history was stored in the Board struct itself, so every temporary board copy during AI search incremented the same counter. The fix was tracking history separately in the main game loop, only for actual moves.

## Performance

The engine searches about 50,000-100,000 positions per second at depth 4 on my machine. Not Stockfish-level (which does millions), but respectable for a from-scratch implementation. The perft test verifies correctness:

```
perft(1) = 20
perft(2) = 400
perft(3) = 8,902
perft(4) = 197,281
perft(5) = 4,865,609  ✓
```

## What I Learned

1. **Bitboards are fast but unintuitive**: It took time to think in bits rather than squares
2. **Move ordering matters enormously**: Good ordering makes alpha-beta 10x faster
3. **Bugs are subtle**: Edge wrapping, hash collisions, and repetition detection all had tricky edge cases
4. **Separation of concerns is crucial**: Keeping engine logic separate from UI made debugging much easier
5. **Testing is essential**: Perft testing caught bugs I never would have found through play testing

## Future Improvements

The engine is functional but could be much stronger:

- **Transposition tables** to cache evaluated positions
- **Iterative deepening** for better time management
- **Quiescence search** to avoid horizon effects
- **Opening book** for better early game play
- **Endgame tablebases** for perfect endgame play

## Conclusion

Building a chess engine was one of the most rewarding programming projects I've undertaken. It combines low-level optimization (bit manipulation), high-level algorithms (alpha-beta pruning), and domain knowledge (chess tactics). 

The full source code is available on [GitHub](https://github.com/ked1108/BitBoardChess). If you're interested in chess programming, I highly recommend the [Chess Programming Wiki](https://www.chessprogramming.org/), it's an incredible resource.
***

**Tags:** #chess #cpp #ai #gamedev #algorithms
