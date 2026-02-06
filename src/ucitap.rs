use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    engine: String,
    logfile: String,
}

fn main() {
    // Parse command line arguments for --config
    let args: Vec<String> = std::env::args().collect();
    let config_file = parse_config_arg(&args).unwrap_or_else(|| "config.json".to_string());

    // Load config file
    let cfg_data = fs::read_to_string(&config_file)
        .expect(&format!("failed to read {}", config_file));
    let cfg: Config = serde_json::from_str(&cfg_data)
        .expect("failed to parse config");

    // Open log file (append, create if missing)
    let logfile = OpenOptions::new()
        .create(true)
        .append(true)
        .open(cfg.logfile)
        .expect("failed to open logfile");
    let logfile = Arc::new(Mutex::new(logfile));

    // Spawn engine with platform-specific settings
    let mut cmd = Command::new(cfg.engine);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    // Windows-specific: hide console window
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().expect("failed to spawn engine");

    let mut engine_stdin = child.stdin.take().expect("engine stdin");
    let mut engine_stdout = child.stdout.take().expect("engine stdout");

    // Thread: stdin -> engine stdin
    let log_in = Arc::clone(&logfile);
    let stdin_thread = thread::spawn(move || {
        let mut stdin = io::stdin();
        let mut buf = [0u8; 4096];
        loop {
            let n = match stdin.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            if engine_stdin.write_all(&buf[..n]).is_err() {
                break;
            }
            let _ = engine_stdin.flush();
            if let Ok(mut log) = log_in.lock() {
                let _ = log.write_all(&buf[..n]);
                let _ = log.flush();
            }
        }
    });

    // Thread: engine stdout -> stdout
    let log_out = Arc::clone(&logfile);
    let stdout_thread = thread::spawn(move || {
        let mut stdout = io::stdout();
        let mut buf = [0u8; 4096];
        loop {
            let n = match engine_stdout.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            let _ = stdout.write_all(&buf[..n]);
            let _ = stdout.flush();
            if let Ok(mut log) = log_out.lock() {
                let _ = log.write_all(&buf[..n]);
                let _ = log.flush();
            }
        }
    });

    let _ = stdin_thread.join();
    let _ = stdout_thread.join();
    let _ = child.wait();
}

// Parse --config argument from command line
fn parse_config_arg(args: &[String]) -> Option<String> {
    for i in 0..args.len() {
        if args[i] == "--config" && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
    }
    None
}