use std::io::{self, BufRead, Write, Read};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use std::fs::{File, OpenOptions};
use std::io::BufReader;
use std::sync::{Mutex, OnceLock};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal;

/// UNIVAC Uploader - Send or receive data via serial port (A sequel to UPLOADER11 written in C)

static LOG_FILE: OnceLock<Option<Mutex<File>>> = OnceLock::new();

/// Log file path win: %LOCALAPPDATA%\UPLOADER19\uploader19.log
/// unix ~/.local/share/UPLOADER19/uploader19.log, otherwise pwd
fn log_path() -> PathBuf {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"));

    let mut dir = base.unwrap_or_else(|| PathBuf::from("."));
    dir.push("UPLOADER19");
    let _ = std::fs::create_dir_all(&dir);
    dir.push("uploader19.log");
    dir
}

/// Open (append) the global log file exactly once.
fn init_log() {
    LOG_FILE.get_or_init(|| {
        let path = log_path();
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => Some(Mutex::new(f)),
            Err(e) => {
                eprintln!("WARNING: could not open log file {}: {}", path.display(), e);
                None
            }
        }
    });
}

/// Buffer for reconstructing received-datastream lines in the log.
static RECV_BUF: OnceLock<Mutex<String>> = OnceLock::new();

fn log_recv_byte(byte: u8) {
    let buf = RECV_BUF.get_or_init(|| Mutex::new(String::new()));
    if let Ok(mut s) = buf.lock() {
        if byte == 0x0A {
            let line = std::mem::take(&mut *s);
            log_line(&format!("[RECV] {}", line));
        } else {
            s.push(byte as char);
        }
    }
}

/// Flush any partial received-datastream line still held in the buffer.
fn flush_recv_buf() {
    if let Some(buf) = RECV_BUF.get() {
        if let Ok(mut s) = buf.lock() {
            if !s.is_empty() {
                let line = std::mem::take(&mut *s);
                log_line(&format!("[RECV] {}", line));
            }
        }
    }
}

/// Append a line to the log file (best effort; ignored if the log is unavailable).
fn log_line(line: &str) {
    if let Some(Some(lock)) = LOG_FILE.get() {
        if let Ok(mut f) = lock.lock() {
            let _ = writeln!(f, "{}", line);
            let _ = f.flush();
        }
    }
}

/// Print to stdout AND mirror to the log file.
macro_rules! log_out {
    ($($arg:tt)*) => {{
        let s = format!($($arg)*);
        println!("{}", s);
        log_line(&s);
    }};
}

/// Print to stderr AND mirror to the log file.
macro_rules! log_err {
    ($($arg:tt)*) => {{
        let s = format!($($arg)*);
        eprintln!("{}", s);
        log_line(&s);
    }};
}

#[derive(Parser, Debug)]
#[command(name = "UPLOADER19")]
#[command(about = "Send or receive data via serial port at 9600 8N1", long_about = None)]
struct Args {
    /// Mode: send (s) or receive (r)
    #[arg(short, long, value_name = "MODE")]
    mode: Option<String>,

    /// File to send (required for send mode)
    #[arg(short, long, value_name = "FILE")]
    file: Option<String>,

    /// Serial port (e.g., COM4, 4, ttyUSB0)
    #[arg(short, long, value_name = "PORT")]
    port: Option<String>,

    /// Delay in seconds between bytes (send mode only)
    #[arg(short, long, value_name = "SECONDS")]
    delay: Option<f64>,

    /// Show the raw datastream instead of a progress bar (send mode)
    #[arg(short, long)]
    stream: bool,
}

fn main() {
    let args = Args::parse();

    // Open the everlasting log and record how this invocation was launched.
    init_log();
    log_line("========================================================================");
    let cli: Vec<String> = std::env::args().collect();
    log_line(&format!("[RUN] UPLOADER19 started; command line: {}", cli.join(" ")));

    // On the classic Windows console (conhost), ANSI escape sequences are not
    // processed by default. Ask crossterm to enable virtual-terminal handling
    // so anything that does emit escapes behaves; the progress bar itself only
    // uses '\r' and plain ASCII, so it works even if this is unavailable.
    #[cfg(windows)]
    let _ = crossterm::ansi_support::supports_ansi();

    // ── Prompt: send or receive ──────────────────────────────────────────────
    let mode = if let Some(m) = args.mode {
        match m.to_uppercase().as_str() {
            "S" | "SEND" => "SEND",
            "R" | "RECEIVE" => "RECEIVE",
            _ => {
                log_err!("Invalid mode. Use 's' or 'send' for Send, 'r' or 'receive' for Receive.");
                std::process::exit(1);
            }
        }
    } else {
        loop {
            let response = prompt("SEND DATA OR RECEIVE DATA (S/R)").to_uppercase();
            match response.as_str() {
                "S" | "SEND" => break "SEND",
                "R" | "RECEIVE" => break "RECEIVE",
                _ => log_err!("Please enter 'S' for Send or 'R' for Receive."),
            }
        }
    };

    if mode == "SEND" {
        send_data(args.file, args.port, args.delay, args.stream);
    } else {
        receive_data(args.port);
    }
}

fn send_data(file_arg: Option<String>, port_arg: Option<String>, delay_arg: Option<f64>, stream: bool) {
    // ── Prompt: file name ────────────────────────────────────────────────────
    let filename = file_arg.unwrap_or_else(|| prompt("TYPE NAME OF PROGRAM FILE.ext"));
    let path = Path::new(&filename);
    if !path.exists() {
        log_err!("ERROR: File '{}' not found.", filename);
        std::process::exit(1);
    }

    // ── Prompt: serial port ──────────────────────────────────────────────────
    #[cfg(target_os = "windows")]
    let port_prompt = "COM PORT NUMBER (1-17)";
    #[cfg(target_os = "linux")]
    let port_prompt = "Serial port (e.g., ttyUSB0, ttyACM0, ttyS0)";
    #[cfg(target_os = "macos")]
    let port_prompt = "Serial port (e.g., cu.usbserial-XXXXX)";
    
    let port_name = get_port_name(&port_arg.unwrap_or_else(|| prompt(port_prompt)));

    // ── Prompt: delay in seconds ─────────────────────────────────────────────
    let delay_secs: f64 = if let Some(d) = delay_arg {
        if d >= 0.0 {
            d
        } else {
            log_err!("Delay must be non-negative.");
            std::process::exit(1);
        }
    } else {
        loop {
            let s = prompt("DELAY in seconds");
            match s.trim().parse::<f64>() {
                Ok(d) if d >= 0.0 => break d,
                _ => log_err!("Please enter a non-negative number (e.g. 0.00001)."),
            }
        }
    };

    let delay = Duration::from_secs_f64(delay_secs);

    // ── Open serial port at 9600 8N1 ─────────────────────────────────────────
    let mut port = serialport::new(&port_name, 9_600)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::None)
        .timeout(Duration::from_millis(10))
        .open()
        .unwrap_or_else(|e| {
            log_err!("ERROR opening {}: {}", port_name, e);
            std::process::exit(1);
        });

    log_out!("Opened {} at 9600 8N1. Uploading '{}'...", port_name, filename);
    log_line(&format!(
        "[SEND] file='{}' port='{}' delay={}s display={}",
        filename, port_name, delay_secs, if stream { "stream" } else { "progress-bar" }
    ));

    // First pass: read the whole file and collect the valid byte values so we
    // know the total up front for the progress bar.
    let file = File::open(path).expect("Cannot open file");
    let reader = BufReader::new(file);
    let mut values: Vec<u8> = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let line = line.expect("I/O error reading file");
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match trimmed.parse::<u8>() {
            Ok(v) => values.push(v),
            Err(_) => {
                log_err!("WARNING: Skipping non-numeric value '{}' on line {}", trimmed, line_num + 1);
            }
        }
    }

    let total = values.len();
    let mut count: u32 = 0;

    // Second pass: send each byte. The raw datastream value is always written to
    // the log; on-screen it is shown either as the raw stream (--stream) or as
    // an in-place progress bar (default).
    for (i, value) in values.iter().enumerate() {
        // Send the byte, retrying transient timeouts. A stalled output buffer
        // (e.g. os error 121 "semaphore timeout", or a WouldBlock/TimedOut)
        // is usually recoverable — the driver just needs a moment to drain.
        write_byte_with_retry(&mut *port, *value, i + 1, total);

        port.flush().unwrap_or_else(|e| {
            log_err!("\nWARNING: flush error: {}", e);
        });

        count += 1;

        if stream {
            // Raw datastream view: echo each value on its own line, like the
            // original program. log_out! also mirrors it to the file.
            log_out!("{}", value);
        } else {
            // Progress-bar view: console shows the bar; the file still records
            // every value so the log is a complete datastream either way.
            log_line(&value.to_string());
            draw_progress(i + 1, total);
        }

        if delay.as_nanos() > 0 {
            thread::sleep(delay);
        }
    }

    if !stream {
        // Move off the progress-bar line before the summary.
        println!();
    }
    log_out!("Done. Sent {} values to {}.", count, port_name);
}

fn receive_data(port_arg: Option<String>) {
    // ── Prompt: serial port ──────────────────────────────────────────────────
    #[cfg(target_os = "windows")]
    let port_prompt = "COM PORT NUMBER (1-17)";
    #[cfg(target_os = "linux")]
    let port_prompt = "Serial port (e.g., ttyUSB0, ttyACM0, ttyS0)";
    #[cfg(target_os = "macos")]
    let port_prompt = "Serial port (e.g., cu.usbserial-XXXXX)";

    let port_name = get_port_name(&port_arg.unwrap_or_else(|| prompt(port_prompt)));

    // ── Open serial port at 9600 8N1 ─────────────────────────────────────────
    let mut port = serialport::new(&port_name, 9_600)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::None)
        .timeout(Duration::from_millis(10))
        .open()
        .unwrap_or_else(|e| {
            log_err!("ERROR opening {}: {}", port_name, e);
            std::process::exit(1);
        });

    log_out!("Opened {} at 9600 8N1. Interactive terminal (send & receive).", port_name);
    log_out!("Type to send data. Press Ctrl+C to stop.");
    println!();
    log_line(&format!("[RECEIVE] port='{}'", port_name));

    // Enable raw mode so keypresses are sent immediately (no Enter needed).
    terminal::enable_raw_mode().unwrap_or_else(|e| {
        log_err!("ERROR enabling raw terminal mode: {}", e);
        std::process::exit(1);
    });

    let mut count: u32 = 0;
    let mut buffer = [0u8; 1];
    let mut loop_count: u64 = 0;
    let mut poll_true_count: u64 = 0;
    let mut read_ok_count: u64 = 0;
    let mut read_timeout_count: u64 = 0;
    let mut read_err_count: u64 = 0;

    eprintln!("[DEBUG] raw mode enabled, entering main loop");
    log_line("[DEBUG] raw mode enabled, entering main loop");

    let mut running = true;
    while running {
        loop_count += 1;

        // Print debug stats every 1000 loops
        if loop_count % 5000 == 0 {
            let msg = format!(
                "[DEBUG] loops={} polls_true={} read_ok={} read_timeout={} read_err={} bytes={}",
                loop_count, poll_true_count, read_ok_count, read_timeout_count, read_err_count, count
            );
            eprintln!("\r\n{}", msg);
            log_line(&msg);
        }

        // ── Check for keyboard input to send ─────────────────────────────────
        match event::poll(Duration::from_millis(1)) {
            Ok(true) => {
                poll_true_count += 1;
                match event::read() {
                    Ok(Event::Key(key_event)) => {
                        let msg = format!(
                            "[DEBUG] key event: code={:?} kind={:?} modifiers={:?}",
                            key_event.code, key_event.kind, key_event.modifiers
                        );
                        eprintln!("\r\n{}", msg);
                        log_line(&msg);

                        if key_event.kind == KeyEventKind::Press {
                            // Ctrl+C to exit
                            if key_event.modifiers.contains(KeyModifiers::CONTROL)
                                && key_event.code == KeyCode::Char('c')
                            {
                                eprintln!("\r\n[DEBUG] Ctrl+C detected, exiting");
                                log_line("[DEBUG] Ctrl+C detected, exiting");
                                let _ = terminal::disable_raw_mode();
                                running = false;
                            } else {
                                // Convert key event to byte(s) and send to serial port
                                let bytes: Vec<u8> = match key_event.code {
                                    KeyCode::Char(c) => {
                                        let mut buf = [0u8; 4];
                                        let s = c.encode_utf8(&mut buf);
                                        s.as_bytes().to_vec()
                                    }
                                    KeyCode::Enter => vec![0x0D],
                                    KeyCode::Backspace => vec![0x08],
                                    KeyCode::Tab => vec![0x09],
                                    KeyCode::Esc => vec![0x1B],
                                    _ => vec![],
                                };

                                eprintln!("[DEBUG] sending {} bytes to serial: {:?}", bytes.len(), bytes);
                                log_line(&format!("[DEBUG] sending {} bytes to serial: {:?}", bytes.len(), bytes));

                                for byte in &bytes {
                                    match port.write_all(&[*byte]) {
                                        Ok(_) => {
                                            eprintln!("[DEBUG] wrote byte 0x{:02X} ok", byte);
                                            log_line(&format!("[DEBUG] wrote byte 0x{:02X} ok", byte));
                                        }
                                        Err(e) => {
                                            let _ = terminal::disable_raw_mode();
                                            eprintln!("\r\n[ERROR] writing to serial port: {}", e);
                                            log_line(&format!("[ERROR] writing to serial port: {}", e));
                                        }
                                    }
                                }
                                if !bytes.is_empty() {
                                    match port.flush() {
                                        Ok(_) => {
                                            eprintln!("[DEBUG] flush ok");
                                            log_line("[DEBUG] flush ok");
                                        }
                                        Err(e) => {
                                            eprintln!("[DEBUG] flush error: {}", e);
                                            log_line(&format!("[DEBUG] flush error: {}", e));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(other_event) => {
                        eprintln!("\r\n[DEBUG] non-key event: {:?}", other_event);
                        log_line(&format!("[DEBUG] non-key event: {:?}", other_event));
                    }
                    Err(e) => {
                        eprintln!("\r\n[DEBUG] event::read() error: {}", e);
                        log_line(&format!("[DEBUG] event::read() error: {}", e));
                    }
                }
            }
            Ok(false) => {
                // No input pending, that's fine
            }
            Err(e) => {
                eprintln!("\r\n[DEBUG] event::poll() error: {}", e);
                log_line(&format!("[DEBUG] event::poll() error: {}", e));
            }
        }

        // ── Read incoming data from serial port ──────────────────────────────
        match port.read(&mut buffer) {
            Ok(bytes_read) if bytes_read > 0 => {
                let byte = buffer[0];
                read_ok_count += 1;

                // Filter out form feed (0x0C) and carriage return (0x0D);
                // print printable ASCII and line feed immediately.
                if byte == 0x0C || byte == 0x0D {
                    // skip
                } else if byte >= 0x20 || byte == 0x0A {
                    let _ = io::stdout().write_all(&[byte]);
                    let _ = io::stdout().flush();
                    // Mirror the raw received byte to the log.
                    log_recv_byte(byte);
                }

                count += bytes_read as u32;
            }
            Ok(_) => {
                read_timeout_count += 1;
            }
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut
                || e.kind() == io::ErrorKind::WouldBlock => {
                read_timeout_count += 1;
            }
            Err(e) => {
                read_err_count += 1;
                let msg = format!(
                    "[DEBUG] serial read error #{}: {} (kind: {:?})",
                    read_err_count, e, e.kind()
                );
                eprintln!("\r\n{}", msg);
                log_line(&msg);
                // Don't exit on first error, keep trying
                if read_err_count > 10 {
                    let _ = terminal::disable_raw_mode();
                    eprintln!("\r\n[ERROR] Too many serial read errors, exiting");
                    log_line("[ERROR] Too many serial read errors, exiting");
                    running = false;
                }
            }
        }
    }

    // Flush any partial received line still buffered, then announce the total.
    flush_recv_buf();
    println!();
    log_out!("Done. Received {} bytes from {}.", count, port_name);
}

/// Write a single byte to the serial port, retrying transient stalls.
///
/// Windows raises os error 121 ("The semaphore timeout period has expired.")
/// when the driver's transmit buffer stays full past its internal timeout —
/// commonly a flow-control stall on a USB-serial adapter. Such a stall is
/// almost always transient, so we back off briefly and retry rather than
/// aborting the whole upload on the first hiccup.
fn write_byte_with_retry(port: &mut dyn serialport::SerialPort, value: u8, done: usize, total: usize) {
    const MAX_RETRIES: u32 = 50;

    let mut attempt = 0;
    loop {
        match port.write_all(&[value]) {
            Ok(()) => return,
            Err(e) => {
                let transient = e.kind() == io::ErrorKind::TimedOut
                    || e.kind() == io::ErrorKind::WouldBlock
                    || e.raw_os_error() == Some(121);

                if transient && attempt < MAX_RETRIES {
                    attempt += 1;
                    // Overwrite the progress line with spaces (no ANSI escape,
                    // so it works on the classic Windows conhost), print the
                    // retry note, then redraw the bar.
                    let msg = format!(
                        "WARNING: serial write stalled at byte {}/{} ({}); retry {}/{}...",
                        done, total, e, attempt, MAX_RETRIES
                    );
                    eprint!("\r{}\r", " ".repeat(72));
                    eprintln!("{}", msg);
                    log_line(&msg);
                    draw_progress(done.saturating_sub(1), total);
                    thread::sleep(Duration::from_millis(100));
                } else {
                    eprintln!("\nERROR writing to serial port: {}", e);
                    log_line(&format!("[ERROR] writing to serial port: {}", e));
                    std::process::exit(1);
                }
            }
        }
    }
}

/// Draw a single-line progress bar in place using a carriage return.
fn draw_progress(done: usize, total: usize) {
    const WIDTH: usize = 40;

    if total == 0 {
        return;
    }

    let fraction = done as f64 / total as f64;
    let filled = (fraction * WIDTH as f64).round() as usize;
    let filled = filled.min(WIDTH);
    let percent = (fraction * 100.0).round() as u32;

    // Use plain ASCII so the bar renders on any console code page (the classic
    // Windows conhost may show '?' for Unicode block glyphs). Trailing spaces
    // pad the line so a shorter redraw fully overwrites a longer previous one,
    // which avoids relying on an ANSI clear-line escape.
    let bar: String = "#".repeat(filled) + &"-".repeat(WIDTH - filled);
    print!("\r[{}] {:3}%  {}/{}   ", bar, percent, done, total);
    let _ = io::stdout().flush();
}

/// Print a prompt and read a line from stdin.
fn prompt(msg: &str) -> String {
    print!("{}: ", msg);
    io::stdout().flush().expect("flush stdout");
    let mut buf = String::new();
    io::stdin()
        .lock()
        .read_line(&mut buf)
        .expect("read_line failed");
    let response = buf.trim_end_matches(['\r', '\n']).to_string();
    log_line(&format!("[PROMPT] {}: {}", msg, response));
    response
}

/// Build the platform-specific port name.
#[cfg(target_os = "windows")]
fn get_port_name(input: &str) -> String {
    // On Windows, accept either a number (1-17) or a full COM port name
    if let Ok(port_num) = input.trim().parse::<u8>() {
        if (1..=17).contains(&port_num) {
            // Windows needs \\.\COM10 and above for two-digit ports
            return if port_num >= 10 {
                format!("\\\\.\\COM{}", port_num)
            } else {
                format!("COM{}", port_num)
            };
        }
    }
    // If not a number, assume it's already a full port name
    input.trim().to_string()
}

#[cfg(target_os = "linux")]
fn get_port_name(input: &str) -> String {
    let trimmed = input.trim();
    // If user provided just the device name (e.g., "ttyUSB0"), prepend /dev/
    if !trimmed.starts_with('/') {
        format!("/dev/{}", trimmed)
    } else {
        trimmed.to_string()
    }
}

#[cfg(target_os = "macos")]
fn get_port_name(input: &str) -> String {
    let trimmed = input.trim();
    // If user provided just the device name (e.g., "cu.usbserial-1234"), prepend /dev/
    if !trimmed.starts_with('/') {
        format!("/dev/{}", trimmed)
    } else {
        trimmed.to_string()
    }
}

/// Mimic the original "Press any key to continue" pause.
fn press_any_key() {
}