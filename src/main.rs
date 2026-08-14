use std::io::{self, BufRead, Write, Read};
use std::path::Path;
use std::thread;
use std::time::Duration;
use std::fs::File;
use std::io::BufReader;
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal;

/// UNIVAC Uploader - Send or receive data via serial port (A sequel to UPLOADER11 written in C)
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
}

fn main() {
    let args = Args::parse();

    // ── Prompt: send or receive ──────────────────────────────────────────────
    let mode = if let Some(m) = args.mode {
        match m.to_uppercase().as_str() {
            "S" | "SEND" => "SEND",
            "R" | "RECEIVE" => "RECEIVE",
            _ => {
                eprintln!("Invalid mode. Use 's' or 'send' for Send, 'r' or 'receive' for Receive.");
                std::process::exit(1);
            }
        }
    } else {
        loop {
            let response = prompt("SEND DATA OR RECEIVE DATA (S/R)").to_uppercase();
            match response.as_str() {
                "S" | "SEND" => break "SEND",
                "R" | "RECEIVE" => break "RECEIVE",
                _ => eprintln!("Please enter 'S' for Send or 'R' for Receive."),
            }
        }
    };

    if mode == "SEND" {
        send_data(args.file, args.port, args.delay);
    } else {
        receive_data(args.port);
    }
}

fn send_data(file_arg: Option<String>, port_arg: Option<String>, delay_arg: Option<f64>) {
    // ── Prompt: file name ────────────────────────────────────────────────────
    let filename = file_arg.unwrap_or_else(|| prompt("TYPE NAME OF PROGRAM FILE.ext"));
    let path = Path::new(&filename);
    if !path.exists() {
        eprintln!("ERROR: File '{}' not found.", filename);
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
            eprintln!("Delay must be non-negative.");
            std::process::exit(1);
        }
    } else {
        loop {
            let s = prompt("DELAY in seconds");
            match s.trim().parse::<f64>() {
                Ok(d) if d >= 0.0 => break d,
                _ => eprintln!("Please enter a non-negative number (e.g. 0.00001)."),
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
            eprintln!("ERROR opening {}: {}", port_name, e);
            std::process::exit(1);
        });

    println!("Opened {} at 9600 8N1. Uploading '{}'...", port_name, filename);

    // The original program printed each value to the console as it was sent.
    let file = File::open(path).expect("Cannot open file");
    let reader = BufReader::new(file);
    let mut count: u32 = 0;

    for (line_num, line) in reader.lines().enumerate() {
        let line = line.expect("I/O error reading file");
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value: u8 = match trimmed.parse::<u8>() {
            Ok(v) => v,
            Err(_) => {
                eprintln!("WARNING: Skipping non-numeric value '{}' on line {}", trimmed, line_num + 1);
                continue;
            }
        };

        // Echo value to console, matching the original program's output
        println!("{}", value);

        // Send the byte
        port.write_all(&[value]).unwrap_or_else(|e| {
            eprintln!("ERROR writing to serial port: {}", e);
            std::process::exit(1);
        });
        port.flush().unwrap_or_else(|e| {
            eprintln!("WARNING: flush error: {}", e);
        });

        count += 1;

        if delay.as_nanos() > 0 {
            thread::sleep(delay);
        }
    }

    println!("\nDone. Sent {} values to {}.", count, port_name);
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
            eprintln!("ERROR opening {}: {}", port_name, e);
            std::process::exit(1);
        });

    println!("Opened {} at 9600 8N1. Interactive terminal (send & receive).", port_name);
    println!("Type to send data. Press Ctrl+C to stop.\n");

    // Enable raw mode so keypresses are sent immediately (no Enter needed).
    terminal::enable_raw_mode().unwrap_or_else(|e| {
        eprintln!("ERROR enabling raw terminal mode: {}", e);
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

    let mut running = true;
    while running {
        loop_count += 1;

        // Print debug stats every 1000 loops
        if loop_count % 5000 == 0 {
            eprintln!(
                "\r\n[DEBUG] loops={} polls_true={} read_ok={} read_timeout={} read_err={} bytes={}",
                loop_count, poll_true_count, read_ok_count, read_timeout_count, read_err_count, count
            );
        }

        // ── Check for keyboard input to send ─────────────────────────────────
        match event::poll(Duration::from_millis(1)) {
            Ok(true) => {
                poll_true_count += 1;
                match event::read() {
                    Ok(Event::Key(key_event)) => {
                        eprintln!(
                            "\r\n[DEBUG] key event: code={:?} kind={:?} modifiers={:?}",
                            key_event.code, key_event.kind, key_event.modifiers
                        );

                        if key_event.kind == KeyEventKind::Press {
                            // Ctrl+C to exit
                            if key_event.modifiers.contains(KeyModifiers::CONTROL)
                                && key_event.code == KeyCode::Char('c')
                            {
                                eprintln!("\r\n[DEBUG] Ctrl+C detected, exiting");
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

                                for byte in &bytes {
                                    match port.write_all(&[*byte]) {
                                        Ok(_) => eprintln!("[DEBUG] wrote byte 0x{:02X} ok", byte),
                                        Err(e) => {
                                            let _ = terminal::disable_raw_mode();
                                            eprintln!("\r\n[ERROR] writing to serial port: {}", e);
                                        }
                                    }
                                }
                                if !bytes.is_empty() {
                                    match port.flush() {
                                        Ok(_) => eprintln!("[DEBUG] flush ok"),
                                        Err(e) => eprintln!("[DEBUG] flush error: {}", e),
                                    }
                                }
                            }
                        }
                    }
                    Ok(other_event) => {
                        eprintln!("\r\n[DEBUG] non-key event: {:?}", other_event);
                    }
                    Err(e) => {
                        eprintln!("\r\n[DEBUG] event::read() error: {}", e);
                    }
                }
            }
            Ok(false) => {
                // No input pending, that's fine
            }
            Err(e) => {
                eprintln!("\r\n[DEBUG] event::poll() error: {}", e);
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
                eprintln!(
                    "\r\n[DEBUG] serial read error #{}: {} (kind: {:?})",
                    read_err_count, e, e.kind()
                );
                // Don't exit on first error, keep trying
                if read_err_count > 10 {
                    let _ = terminal::disable_raw_mode();
                    eprintln!("\r\n[ERROR] Too many serial read errors, exiting");
                    running = false;
                }
            }
        }
    }

    println!("\nDone. Received {} bytes from {}.", count, port_name);
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
    buf.trim_end_matches(['\r', '\n']).to_string()
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