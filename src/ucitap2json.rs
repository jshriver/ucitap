use clap::Parser;
use regex::Regex;
use serde::Serialize;
use shakmaty::{Chess, Position, Square, Role};
use shakmaty::fen::Fen;
use shakmaty::san::San;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use zstd::stream::write::Encoder;
use anyhow::Result;

#[derive(Parser)]
struct Args {
    /// Input UCI log file
    #[arg(short = 'l', long = "log")]
    log: String,

    /// Compress output to .zst
    #[arg(short = 'c')]
    compress: bool,
}

#[derive(Serialize)]
struct Payload {
    engine: String,
    fen: String,
    ply: Option<u32>,
    score: Option<i32>,
    mate: Option<i32>,
    nodes: Option<u64>,
    nps: Option<u64>,
    time: Option<u64>,
    pv: Option<String>,
}

fn trim_fen(fen: &str) -> String {
    let parts: Vec<&str> = fen.split_whitespace().collect();
    if parts.len() >= 4 {
        parts[..4].join(" ")
    } else {
        fen.to_string()
    }
}

fn parse_uci_move(uci: &str, board: &Chess) -> Option<shakmaty::Move> {
    if uci.len() < 4 {
        return None;
    }
    
    let from = uci[0..2].parse::<Square>().ok()?;
    let to = uci[2..4].parse::<Square>().ok()?;
    
    // Check for promotion
    let promotion = if uci.len() == 5 {
        match &uci[4..5] {
            "q" => Some(Role::Queen),
            "r" => Some(Role::Rook),
            "b" => Some(Role::Bishop),
            "n" => Some(Role::Knight),
            _ => None,
        }
    } else {
        None
    };
    
    // Generate legal moves and find the matching one
    for m in board.legal_moves() {
        if m.from() == Some(from) && m.to() == to {
            if let Some(promo) = promotion {
                if m.promotion() == Some(promo) {
                    return Some(m);
                }
            } else if m.promotion().is_none() {
                return Some(m);
            }
        }
    }
    
    None
}

fn convert_pv_to_san(pv: &str, initial_board: &Chess) -> String {
    let mut board = initial_board.clone();
    let mut san_moves = Vec::new();
    
    for uci in pv.split_whitespace() {
        if let Some(chess_move) = parse_uci_move(uci, &board) {
            let san = San::from_move(&board, chess_move);
            san_moves.push(san.to_string());
            board.play_unchecked(chess_move);
        } else {
            // If we can't parse a move, stop here
            break;
        }
    }
    
    san_moves.join(" ")
}

fn apply_moves(mut board: Chess, moves_str: &str) -> Chess {
    for m in moves_str.split_whitespace() {
        if let Some(chess_move) = parse_uci_move(m, &board) {
            board.play_unchecked(chess_move);
        }
    }
    board
}

fn main() -> Result<()> {
    let args = Args::parse();

    let input = File::open(&args.log)?;
    let reader = BufReader::new(input);

    let base = Path::new(&args.log)
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let json_path = format!("{base}.json");
    let final_path = if args.compress {
        format!("{base}.zst")
    } else {
        json_path.clone()
    };

    println!("📖 Parsing UCI log: {}", args.log);

    let re_info = Regex::new(r"info ").unwrap();
    let re_pv = Regex::new(r"(^| )pv (.*)$").unwrap();

    let mut board = Chess::default();
    let mut engine = String::new();

    let mut fen: Option<String> = None;
    let mut ply: Option<u32> = None;
    let mut score: Option<i32> = None;
    let mut mate: Option<i32> = None;
    let mut nodes: Option<u64> = None;
    let mut nps: Option<u64> = None;
    let mut time: Option<u64> = None;
    let mut pv: Option<String> = None;

    let mut results: Vec<Payload> = Vec::new();
    let mut line_count: u64 = 0;

    for line in reader.lines() {
        let line = line?;
        line_count += 1;

        if line_count % 100_000 == 0 {
            println!("📖 Parsed {} lines…", line_count);
        }

        match line.as_str() {
            "uci" | "ucinewgame" => {
                board = Chess::default();
                fen = None;
                ply = None;
                score = None;
                mate = None;
                nodes = None;
                nps = None;
                time = None;
                pv = None;
                continue;
            }
            _ => {}
        }

        if let Some(rest) = line.strip_prefix("id name ") {
            engine = rest.to_string();
            continue;
        }

        if let Some(rest) = line.strip_prefix("position ") {
            if rest.starts_with("startpos") {
                board = Chess::default();
                if let Some(moves) = rest.strip_prefix("startpos moves ") {
                    board = apply_moves(board, moves);
                }
                let f = Fen::from_position(&board, shakmaty::EnPassantMode::Legal);
                fen = Some(trim_fen(&f.to_string()));
            } else if let Some(fen_part) = rest.strip_prefix("fen ") {
                // Split on " moves " to handle both FEN and subsequent moves
                if let Some(moves_idx) = fen_part.find(" moves ") {
                    let fen_str = &fen_part[..moves_idx];
                    let moves_str = &fen_part[moves_idx + 7..]; // Skip " moves "
                    
                    if let Ok(parsed_fen) = fen_str.parse::<Fen>() {
                        if let Ok(pos) = parsed_fen.into_position(shakmaty::CastlingMode::Standard) {
                            board = pos;
                            board = apply_moves(board, moves_str);
                        }
                    }
                } else {
                    // Just FEN, no moves
                    if let Ok(parsed_fen) = fen_part.parse::<Fen>() {
                        if let Ok(pos) = parsed_fen.into_position(shakmaty::CastlingMode::Standard) {
                            board = pos;
                        }
                    }
                }
                let f = Fen::from_position(&board, shakmaty::EnPassantMode::Legal);
                fen = Some(trim_fen(&f.to_string()));
            }
            continue;
        }

        if re_info.is_match(&line) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let mut i = 0;
            while i < parts.len() {
                match parts[i] {
                    "depth" => ply = parts.get(i + 1).and_then(|v| v.parse().ok()),
                    "cp" => score = parts.get(i + 1).and_then(|v| v.parse().ok()),
                    "mate" => mate = parts.get(i + 1).and_then(|v| v.parse().ok()),
                    "nodes" => nodes = parts.get(i + 1).and_then(|v| v.parse().ok()),
                    "nps" => nps = parts.get(i + 1).and_then(|v| v.parse().ok()),
                    "time" => time = parts.get(i + 1).and_then(|v| v.parse().ok()),
                    _ => {}
                }
                i += 1;
            }

            if let Some(cap) = re_pv.captures(&line) {
                let uci_pv = cap[2].trim();
                // Convert UCI PV to SAN
                let san_pv = convert_pv_to_san(uci_pv, &board);
                pv = Some(san_pv);
            }

            continue;
        }

        if line.starts_with("bestmove") {
            if let Some(fen_val) = &fen {
                results.push(Payload {
                    engine: engine.clone(),
                    fen: fen_val.clone(),
                    ply,
                    score,
                    mate,
                    nodes,
                    nps,
                    time,
                    pv: pv.clone(),
                });
            }

            ply = None;
            score = None;
            mate = None;
            nodes = None;
            nps = None;
            time = None;
            pv = None;
        }
    }

    println!("✅ Parsing complete — {} positions captured", results.len());
    println!("🧠 Serializing JSON objects…");

    let json_data = serde_json::to_vec_pretty(&results)?;

    println!("✅ JSON serialization complete");

    if args.compress {
        println!("🗜️  Compressing JSON (max level)…");
        let file = File::create(&final_path)?;
        let mut encoder = Encoder::new(file, 22)?;
        encoder.write_all(&json_data)?;
        encoder.finish()?;
        println!("💾 Writing output file: {}", final_path);
        println!("🎉 Done! Wrote {} positions", results.len());
    } else {
        println!("💾 Writing output file: {}", final_path);
        std::fs::write(&final_path, json_data)?;
        println!("🎉 Done! Wrote {} positions", results.len());
    }

    Ok(())
}