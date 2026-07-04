# urlgrep

Stream URLs from stdin, fetch concurrently, output matches via a JavaScript
matcher. Chrome TLS/HTTP2 fingerprint.

## Install

```bash
cargo install --git https://github.com/ogpourya/urlgrep.git
```

## Usage

```bash
cat urls.txt | urlgrep --script match.js [options]
```

## Matcher (QuickJS)

The `-s` flag accepts a **file path** or **inline JS code**. The code must
define a `match` function:

```javascript
function match(url, body, status, headers) {
    return body.includes("admin");
}
```

| Argument | Type | Description |
|----------|------|-------------|
| `url` | string | Full URL being fetched |
| `body` | string | Response body (first 1MB) |
| `status` | number | HTTP status code |
| `headers` | string | Response headers as `Name: value\n` lines |

Return `true` to print the URL.

```bash
# file
urlgrep -s match.js < urls.txt

# inline
urlgrep -s 'function match(u,b,s,h){return b.includes("admin")}' < urls.txt
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-s, --script SRC` | — | Matcher: file path or inline JS |
| `-w, --workers N` | 10 | Max concurrency |
| `-t, --timeout SEC` | 3.0 | Request timeout |
| `-r, --retry N` | 3 | Max retries (GET/HEAD only) |
| `-d, --debug` | — | Diagnostics to stderr |
| `-X, --method M` | GET | HTTP method |
| `-H, --header "K: V"` | — | Custom header (repeatable) |
| `--data STR` | — | Request body |
| `-L, --follow-redirects` | — | Follow redirects |
| `-A, --user-agent UA` | Chrome 126 | User-Agent |
| `-k, --insecure` | — | Skip TLS verify |
| `-p, --proxy URL` | — | Proxy |
| `--prefer-https BOOL` | true | Try https first for bare domains |

Bare domains (no `http://`/`https://`) expand to both schemes. Non-GET methods
(POST/PUT/DELETE) are not retried to avoid duplicate side effects. Response
bodies are capped at 1MB — only the first 1MB is passed to the matcher.

## Examples

```bash
# File matcher
echo "example.com" | urlgrep -s match.js

# Inline JS
echo "example.com" | urlgrep -s 'function match(u,b,s,h){return b.includes("html")}'

# 200 workers, follow redirects, skip SSL
cat millions.txt | urlgrep -s match.js -w 200 -L -k

# POST JSON, custom header
echo "api.example.com" | urlgrep -s match.js -X POST \
  --data '{}' -H "Content-Type: application/json"

# HTTP-first for bare domains
echo "neverssl.com" | urlgrep -s match.js --prefer-https false

# Debug mode shows request lifecycle
echo "example.com" | urlgrep -s match.js -d
```
