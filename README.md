# UPLOADER19

Send or receive data via serial port at 9600 8N1.

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
  -h, --help             Print help
```

### Examples

Send file:
```
UPLOADER19 -m s -f data.76 -p COM4 -d 0.01
```

Receive data (and send data):
```
UPLOADER19 -m r -p COM4
```

Interactive mode (no arguments):
```
UPLOADER19
```

## Build

```bash
cargo build --release
```

## Requirements

Serial port hardware. 9600 baud, 8 data bits, no parity, 1 stop bit.