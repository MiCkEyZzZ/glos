## GLOS Replayer — Quick Test Guide

This section demonstrates how to record a test signal and replay it over
UDP using the built-in developer listener.

---

### 1. Record a test file (if not already created)

```bash
cargo run -p glos-recorder --release -- \
  --device sim --freq 1602MHz --rate 2MHz \
  --output signal.glos --duration 5
```

---

### 2. Start UDP debug listener (separate terminal)

```bash
./tools/udp_server.py
```

Expected output:

```
Listening UDP on 127.0.0.1:5555
```

---

### 3. Replay in real time (1× speed)

```bash
cargo run -p glos-replayer --release -- \
  --input signal.glos \
  --output udp://127.0.0.1:5555
```

---

### 4. Replay at 2× speed

```bash
cargo run -p glos-replayer --release -- \
  --input signal.glos \
  --output udp://127.0.0.1:5555 \
  --speed 2.0
```

---

### 5. Slow playback with looping

```bash
cargo run -p glos-replayer --release -- \
  --input signal.glos \
  --output udp://127.0.0.1:5555 \
  --speed 0.5 --loop
```

---

### 6. Replay LZ4-compressed file

```bash
cargo run -p glos-replayer --release -- \
  --input signal_lz4.glos \
  --output udp://127.0.0.1:5555 \
  --speed 10.0
```

---

### 7. Run unit and integration tests

```bash
cargo test -p glos-replayer
cargo test -p glos-replayer -- --nocapture
```

---

### Example Listener Output

```
packet=1 bytes=1472 total=1472
0000a13f0020ab34...

packet=2 bytes=1472 total=2944
0000a1400020ab91...
```

The listener displays:

- packet counter
- UDP payload size
- total received bytes
- hex preview of packet data

---

### Notes

The Python UDP listener is provided to ensure consistent debugging across
platforms without relying on system-specific tools such as:

- `nc`
- `socat`
- `xxd`

See `tools/README.md` for details.
