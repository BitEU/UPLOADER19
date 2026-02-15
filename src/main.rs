use std::io::{self, BufRead, Write, Read};
use std::path::Path;
use std::thread;
use std::time::Duration;
use std::fs::File;
use std::io::BufReader;
use clap::Parser;

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
    press_any_key();
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
        .timeout(Duration::from_millis(100))
        .open()
        .unwrap_or_else(|e| {
            eprintln!("ERROR opening {}: {}", port_name, e);
            std::process::exit(1);
        });

    println!("Opened {} at 9600 8N1. Listening for data...", port_name);
    println!("Press Ctrl+C to stop receiving.\n");

    let mut count: u32 = 0;

    // Read one byte at a time and flush stdout immediately so there is
    // effectively no user-space buffering of received characters.
    let mut buffer = [0u8; 1];

    loop {
        match port.read(&mut buffer) {
            Ok(bytes_read) if bytes_read > 0 => {
                let byte = buffer[0];

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
                // No data available; yield briefly.
                thread::sleep(Duration::from_millis(10));
            }
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                // Timeout is normal when no data, just continue
                continue;
            }
            Err(e) => {
                eprintln!("\nERROR reading from serial port: {}", e);
                break;
            }
        }
    }

    println!("\nDone. Received {} bytes from {}.", count, port_name);
    press_any_key();
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
    print!("\nPress any key to continue...");
    io::stdout().flush().expect("flush stdout");
    // On Windows the terminal will normally close; just wait for Enter.
    let mut buf = String::new();
    let _ = io::stdin().lock().read_line(&mut buf);
}