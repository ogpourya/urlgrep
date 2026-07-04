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

The `-s` flag accepts a **file path**, **inline JS code**, or a **builtin tag**
(`@name`). The code must define a `match` function:

```javascript
function match(url, body, status, headers) {
    return body.includes("admin");
}
```

| Argument | Type | Description |
|----------|------|-------------|
| `url` | string | Full URL being fetched |
| `body` | string | Response body (up to 1MB) |
| `status` | number | HTTP status code |
| `headers` | string | Response headers as `Name: value\n` |

Return `true` to print the URL.

### Inline code

```bash
urlgrep -s 'function match(u,b,s,h){return b.includes("admin")}' < urls.txt
```

### Builtin tags

Use `@tagname` to load a production-quality detection script:

```bash
urlgrep -s @env < urls.txt
urlgrep -s @secrets < urls.txt
urlgrep -s @admin < urls.txt
```

List all builtins:

```bash
urlgrep --list-tags
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-s, --script SRC` | — | Matcher: file, inline JS, or `@tag` |
| `-w, --workers N` | 10 | Max concurrency |
| `-t, --timeout SEC` | 3.0 | Request timeout |
| `-r, --retry N` | 3 | Max retries |
| `-d, --debug` | — | Diagnostics to stderr |
| `-X, --method M` | GET | HTTP method |
| `-H, --header "K: V"` | — | Custom header (repeatable) |
| `--data STR` | — | Request body |
| `-L, --follow-redirects` | — | Follow redirects |
| `-A, --user-agent UA` | Chrome 126 | User-Agent |
| `-k, --insecure` | — | Skip TLS verify |
| `-p, --proxy URL` | — | Proxy |
| `--prefer-https BOOL` | true | Try https first for bare domains |
| `--list-tags` | — | Show available builtin tags |

Bare domains (no `http://`/`https://`) expand to both schemes.

## Examples

```bash
# File matcher
echo "example.com" | urlgrep -s match.js

# Inline JS
echo "example.com" | urlgrep -s 'function match(u,b,s,h){return b.includes("html")}'

# Builtin tag
echo "example.com" | urlgrep -s @admin

# 200 workers, follow redirects, skip SSL
cat millions.txt | urlgrep -s @secrets -w 200 -L -k

# POST JSON, custom header
echo "api.example.com" | urlgrep -s match.js -X POST \
  --data '{}' -H "Content-Type: application/json"

# HTTP-first for bare domains
echo "neverssl.com" | urlgrep -s match.js --prefer-https false

# Debug mode shows request lifecycle
echo "example.com" | urlgrep -s match.js -d
```
