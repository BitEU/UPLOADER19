# UPLOADER19

Send or receive data via serial port at 9600 8N1. A sequel to UPLOADER11 (originally written in C), used to upload programs to a UNIVAC.

## Download

[Latest builds](../../actions) - Click latest workflow → Artifacts

- `UPLOADER19-linux-x86_64` - Linux
- `UPLOADER19-macos-aarch64` - Mac M1/M2/M3
- `UPLOADER19-windows-x86_64.exe` - Windows

## Install

### Windows
```
UPLOADER19.exe --help
```

### macOS/Linux
```bash
chmod +x UPLOADER19
./UPLOADER19 --help
```

## Usage

```
UPLOADER19 [OPTIONS]

Options:
  -m, --mode <MODE>      Mode: send (s) or receive (r)
  -f, --file <FILE>      File to send (required for send mode)
  -p, --port <PORT>      Serial port (e.g., COM4, 4, ttyUSB0)
  -d, --delay <SECONDS>  Delay in seconds between bytes (send mode only)
  -s, --stream           Show the raw datastream instead of a progress bar (send mode)
  -h, --help             Print help
```

### Examples

Send a file (shows a progress bar):
```
UPLOADER19 -m s -f data.76 -p COM4 -d 0.01
```

Receive data (you can also type to send):
```
UPLOADER19 -m r -p COM4
```

Fully interactive:
```
UPLOADER19
```

## Modes

### Send mode

Reads the input file, sends each value as one byte over the serial port at 9600 8N1,
and reports progress.

### Receive mode

Opens the port and runs an **interactive bidirectional terminal**:

- Incoming serial data is printed to the screen as it arrives (form feed `0x0C` and
  carriage return `0x0D` are filtered out).
- **You can type to send** — keystrokes are transmitted immediately (raw mode, no Enter
  required). Enter sends `0x0D`, Backspace `0x08`, Tab `0x09`, Esc `0x1B`.
- Press **Ctrl+C** to stop.

Periodic `[DEBUG]` statistics (loop/poll/read counts) are printed to help diagnose
serial issues.

## Serial port naming

- **Windows** — accept either a bare number `1`–`17` or a full name like `COM4`.
  Ports `10` and above are automatically addressed as `\\.\COM10` etc.
- **Linux** — a bare device name like `ttyUSB0` is expanded to `/dev/ttyUSB0`
  (examples: `ttyUSB0`, `ttyACM0`, `ttyS0`).
- **macOS** — a bare name like `cu.usbserial-XXXX` is expanded to `/dev/cu.usbserial-XXXX`.

## Logging

Log files can be found in:

- **Windows:** `%LOCALAPPDATA%\UPLOADER19\uploader19.log`
- **macOS/Linux:** `~/.local/share/UPLOADER19/uploader19.log`

## Build

```bash
cargo build --release
```

## Requirements

Serial port hardware. 9600 baud, 8 data bits, no parity, 1 stop bit (9600 8N1).