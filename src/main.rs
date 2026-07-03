use std::sync::Arc;
use std::time::Duration;

use bytes::BytesMut;
use clap::Parser;
use futures::StreamExt;
use regex::bytes::Regex;
use reqwest::Client;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_stream::wrappers::LinesStream;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const CHROME_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const BUF_CAP: usize = 32768;
const BUF_OVERLAP: usize = 4096;

#[derive(Parser, Clone)]
#[command(version, about = "Chrome-fingerprinted async URL grepper")]
struct Args {
    pattern: String,
    #[arg(short = 't', long, default_value = "3.0")]
    timeout: f64,
    #[arg(short = 'w', long, default_value = "10")]
    workers: usize,
    #[arg(short = 'r', long, default_value = "3")]
    retry: u32,
    #[arg(short = 'd', long)]
    debug: bool,
    #[arg(short = 'p', long)]
    proxy: Option<String>,
    #[arg(long)]
    insecure: bool,
}

fn build_client(args: &Args) -> Result<Client, Box<dyn std::error::Error>> {
    use reqwest::header;

    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::ACCEPT,
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8".parse()?,
    );
    headers.insert(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9".parse()?);
    headers.insert(header::ACCEPT_ENCODING, "gzip, deflate, br, zstd".parse()?);
    headers.insert(
        "Sec-Ch-Ua",
        "\"Google Chrome\";v=\"126\", \"Chromium\";v=\"126\", \"Not.A/Brand\";v=\"24\"".parse()?,
    );
    headers.insert("Sec-Ch-Ua-Mobile", "?0".parse()?);
    headers.insert("Sec-Ch-Ua-Platform", "\"Windows\"".parse()?);
    headers.insert("Sec-Fetch-Dest", "document".parse()?);
    headers.insert("Sec-Fetch-Mode", "navigate".parse()?);
    headers.insert("Sec-Fetch-Site", "none".parse()?);
    headers.insert("Sec-Fetch-User", "?1".parse()?);
    headers.insert("Upgrade-Insecure-Requests", "1".parse()?);
    headers.insert("Priority", "u=0, i".parse()?);

    let mut builder = Client::builder()
        .user_agent(CHROME_UA)
        .default_headers(headers)
        .timeout(Duration::from_secs_f64(args.timeout))
        .connect_timeout(Duration::from_secs_f64(args.timeout))
        .tls_built_in_root_certs(true)
        .http2_adaptive_window(true)
        .tcp_nodelay(true)
        .tcp_keepalive(Duration::from_secs(30))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(args.workers);

    if args.insecure {
        builder = builder.danger_accept_invalid_certs(true);
    }

    if let Some(proxy_url) = &args.proxy {
        builder = builder.proxy(reqwest::Proxy::all(proxy_url)?);
    }

    Ok(builder.build()?)
}

fn normalize_url(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            trimmed.to_string()
        } else {
            format!("https://{}", trimmed)
        },
    )
}

async fn fetch_and_scan(client: &Client, pattern: &Regex, url: &str, args: &Args) {
    for attempt in 0..=args.retry {
        match fetch_once(client, pattern, url, args.timeout, args.debug).await {
            Ok(true) => {
                println!("{url}");
                return;
            }
            Ok(false) => return,
            Err(_) if attempt < args.retry => {
                tokio::time::sleep(Duration::from_millis(100 * (attempt as u64 + 1))).await;
            }
            _ => return,
        }
    }
}

async fn fetch_once(
    client: &Client,
    pattern: &Regex,
    url: &str,
    timeout: f64,
    debug: bool,
) -> Result<bool, reqwest::Error> {
    let resp = client
        .get(url)
        .timeout(Duration::from_secs_f64(timeout))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Ok(false);
    }

    let mut buf = BytesMut::with_capacity(BUF_CAP);
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let chunk_len = chunk.len();

        if buf.len() + chunk_len > BUF_CAP {
            let keep = std::cmp::min(buf.len(), BUF_OVERLAP);
            let _ = buf.split_to(buf.len() - keep);
        }

        buf.extend_from_slice(&chunk);

        if let Some(m) = pattern.find(&buf) {
            if debug {
                debug_output(m.start(), m.end(), &buf);
            }
            return Ok(true);
        }
    }

    Ok(false)
}

fn debug_output(m_start: usize, m_end: usize, buf: &[u8]) {
    let line_start = buf[..m_start]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    let line_end = buf[m_end..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(buf.len(), |i| m_end + i);
    let raw_line = &buf[line_start..line_end];

    let rel_start = m_start - line_start;
    let rel_end = m_end - line_start;

    let ctx = 50usize;
    let mut disp_start = rel_start.saturating_sub(ctx / 2);
    let disp_end = std::cmp::min(raw_line.len(), disp_start + ctx);

    if disp_end - disp_start < ctx {
        disp_start = disp_end.saturating_sub(ctx);
    }

    let display = &raw_line[disp_start..disp_end];
    let d_rel_start = rel_start - disp_start;
    let d_rel_end = std::cmp::min(rel_end - disp_start, display.len());

    let pre = String::from_utf8_lossy(&display[..d_rel_start]).replace('\n', " ");
    let match_text = String::from_utf8_lossy(&display[d_rel_start..d_rel_end]).replace('\n', " ");
    let post = String::from_utf8_lossy(&display[d_rel_end..]).replace('\n', " ");

    eprintln!("Found: {pre}\x1b[91m{match_text}\x1b[0m{post}");
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arc::new(Args::parse());

    {
        let args = args.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            let _ = args;
            std::process::exit(0);
        });
    }

    let client = Arc::new(build_client(&args)?);
    let pattern = Arc::new(Regex::new(&args.pattern)?);

    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let lines = reader.lines();
    let stream = LinesStream::new(lines).filter_map(|line| async move {
        let l = line.ok()?;
        normalize_url(&l)
    });

    stream
        .for_each_concurrent(args.workers, {
            let client = client.clone();
            let pattern = pattern.clone();
            let args = args.clone();

            move |url: String| {
                let client = client.clone();
                let pattern = pattern.clone();
                let args = args.clone();

                async move {
                    fetch_and_scan(&client, &pattern, &url, &args).await;
                }
            }
        })
        .await;

    Ok(())
}
