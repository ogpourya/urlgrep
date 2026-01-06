# urlgrep

Streams URLs from stdin, fetches them concurrently, and outputs only those whose responses match a regex.

## Install

```bash
uv tool install https://github.com/ogpourya/urlgrep.git
```

## Usage

```bash
cat urls.txt | urlgrep "pattern" [options] | sort -u
```

## Options

- `pattern`: Regex to match
- `-w, --workers`: Max concurrency (default 10)
- `-t, --timeout`: Request timeout (default 3s)
- `-p, --proxy`: Proxy URL
- `-r, --retry`: Max retries (default 3)
- `-d, --debug`: Show matching context in stderr

## Performance

- Asyncio + uvloop
- Zero-copy (mostly) bytes scanning
- Shared connection pooling
- **Note**: Output is streamed as found. Pipe to `sort -u` for unique results.
