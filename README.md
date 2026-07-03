# urlgrep

Streams URLs from stdin, fetches them concurrently, and outputs only those whose responses match a regex.
Uses the full Chrome TLS/HTTP2 fingerprint to avoid blocking.

## Install

```bash
cargo install --git https://github.com/ogpourya/urlgrep.git
```

Or build from source:

```bash
git clone https://github.com/ogpourya/urlgrep.git
cd urlgrep
cargo build --release
cp target/release/urlgrep ~/.local/bin/
```

## Usage

```bash
cat urls.txt | urlgrep "pattern" [options]
```

## Options

- `pattern`: Regex to match
- `-w, --workers`: Max concurrency (default 10)
- `-t, --timeout`: Request timeout in seconds (default 3.0)
- `-p, --proxy`: Proxy URL (e.g. `http://127.0.0.1:8080`)
- `-r, --retry`: Max retries (default 3)
- `-d, --debug`: Show matching context in stderr (highlighted)
- `--insecure`: Skip TLS certificate validation

## Chrome Fingerprint

Requests are crafted to mimic Google Chrome 126:

- **TLS**: rustls with Chrome-aligned cipher suites via reqwest
- **HTTP/2**: Adaptive window, nodelay, keepalive
- **Headers**: Full Sec-Ch-Ua, Sec-Fetch-*, Priority, and standard Accept/
  Accept-Language/Accept-Encoding matching Chrome defaults

## Performance

- Written in Rust — compiled, no interpreter overhead
- **mimalloc** global allocator — fast concurrent allocation
- **tokio** multi-thread async runtime
- **BytesMut** sliding-window scanner (32KB cap, 4KB overlap)
- **reqwest** connection pool with keepalive
- LTO + `codegen-units=1` + `target-cpu=native` in release builds
- **Note**: Output is streamed as found. Pipe to `sort -u` for unique results.
