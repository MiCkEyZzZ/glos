# GLOS Developer Tools

This directory contains auxiliary developer tools used for debugging, testing,
and inspecting GLOS data streams.

These tools are **not part of the production binaries** and are intended only for
development and diagnostics.

## UDP Debug Listener

`udp_server.py` is a minimal UDP listener used to inspect packets produced by **glos-replayer**.

It allows developers to verify:

- UDP transmission works correctly
- packet sizes
- packet rate
- byte throughput
- raw payload preview (hex)

This tool replaces ad-hoc utilities like `nc`, `socat`, or `xxd` and works
consistently across platforms.

### Requirements

- Python 3.8+

No external dependencies are required.

### Running

From the repository root:

```bash
./tools/udp_server.py
```

or:

```bash
python3 tools/udp_server.py
```

Default address:

```text
127.0.0.1:5555
```

---

### Example Usage

Start the listener:

```bash
./tools/udp_server.py
```

In another terminal run the replayer:

```bash
cargo run -p glos-replayer --release -- \
  --input signal.glos \
  --output udp://127.0.0.1:5555
```

Example output:

```text
Listening UDP on 127.0.0.1:5555
packets=50 bytes=2531494 rate=0.10 MB/s
packets=100 bytes=5001000 rate=0.20 MB/s
packets=150 bytes=7532494 rate=0.30 MB/s
packets=200 bytes=10002000 rate=0.39 MB/s
packets=250 bytes=12533494 rate=0.49 MB/s
packets=300 bytes=15003000 rate=0.58 MB/s
packets=350 bytes=17534494 rate=0.67 MB/s
packets=400 bytes=20004000 rate=0.75 MB/s
...
```

### Output Description

| Field    | Description               |
| -------- | ------------------------- |
| `packet` | Sequential packet number  |
| `bytes`  | UDP payload size          |
| `total`  | Total bytes received      |
| hex dump | First 32 bytes of payload |

### Why this tool exists

Different systems ship incompatible versions of networking tools:

- `nc` (multiple variants)
- `socat`
- `xxd`

This script provides a **deterministic and portable debugging method** for
GLOS UDP streams.

### Scope

This tool is intended for:

- local development
- debugging
- integration testing
- protocol inspection

It is **not** intended for benchmarking or production monitoring.

### Future Improvements (optional)

- packet rate (pps) display
- bitrate statistics
- GLOS packet header parsing
- jitter analysis

## Contributing

Developer tools should remain:

- dependency-free
- simple
- cross-platform
- optional for users

Avoid adding heavy frameworks or external Python packages.
