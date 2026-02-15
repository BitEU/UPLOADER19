use std::io::{self, BufRead, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;
use std::fs::File;
use std::io::BufReader;

fn main() {
    // ── Prompt: file name ────────────────────────────────────────────────────
    let filename = prompt("TYPE NAME OF PROGRAM FILE.ext");
    let path = Path::new(&filename);
    if !path.exists() {
        eprintln!("ERROR: File '{}' not found.", filename);
        std::process::exit(1);
    }

    // ── Prompt: COM port ─────────────────────────────────────────────────────
    let port_num: u8 = loop {
        let s = prompt("COM PORT NUMBER (1-17)");
        match s.trim().parse::<u8>() {
            Ok(n) if (1..=17).contains(&n) => break n,
            _ => eprintln!("Please enter a number between 1 and 17."),
        }
    };

    // ── Prompt: delay in seconds ─────────────────────────────────────────────
    let delay_secs: f64 = loop {
        let s = prompt("DELAY in seconds");
        match s.trim().parse::<f64>() {
            Ok(d) if d >= 0.0 => break d,
            _ => eprintln!("Please enter a non-negative number (e.g. 0.00001)."),
        }
    };

    let delay = Duration::from_secs_f64(delay_secs);

    // ── Build port name ───────────────────────────────────────────────────────
    // Windows needs \\.\COM10 and above for two-digit ports.
    let port_name = if port_num >= 10 {
        format!("\\\\.\\COM{}", port_num)
    } else {
        format!("COM{}", port_num)
    };

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

/// Mimic the original "Press any key to continue" pause.
fn press_any_key() {
    print!("\nPress any key to continue...");
    io::stdout().flush().expect("flush stdout");
    // On Windows the terminal will normally close; just wait for Enter.
    let mut buf = String::new();
    let _ = io::stdin().lock().read_line(&mut buf);
}