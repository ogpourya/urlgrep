use std::sync::Arc;
use std::time::Duration;

use bytes::BytesMut;
use clap::Parser;
use futures::StreamExt;
use reqwest::{header, Client, Method};
use rquickjs::{Context, Function, Runtime};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_stream::wrappers::LinesStream;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const CHROME_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const BUF_CAP: usize = 1048576;
const BUF_OVERLAP: usize = 4096;

#[derive(Parser, Clone)]
#[command(version, about = "Chrome-fingerprinted async URL grepper")]
struct Args {
    #[arg(short = 's', long, help = "JavaScript match function (file path or inline code)")]
    script: String,

    #[arg(short = 't', long, default_value = "3.0")]
    timeout: f64,
    #[arg(short = 'w', long, default_value = "10")]
    workers: usize,
    #[arg(short = 'r', long, default_value = "3")]
    retry: u32,
    #[arg(short = 'd', long)]
    debug: bool,

    #[arg(short = 'X', long, default_value = "GET")]
    method: String,
    #[arg(short = 'H', long = "header")]
    headers: Vec<String>,
    #[arg(long = "data")]
    data: Option<String>,
    #[arg(short = 'L', long)]
    follow_redirects: bool,
    #[arg(short = 'A', long = "user-agent")]
    user_agent: Option<String>,

    #[arg(short = 'k', long)]
    insecure: bool,
    #[arg(short = 'p', long)]
    proxy: Option<String>,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    prefer_https: bool,
}

fn build_client(args: &Args) -> Result<Client, Box<dyn std::error::Error>> {
    let mut default_headers = header::HeaderMap::new();
    default_headers.insert(
        header::ACCEPT,
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"
            .parse()?,
    );
    default_headers.insert(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9".parse()?);
    default_headers.insert(header::ACCEPT_ENCODING, "gzip, deflate, br, zstd".parse()?);
    default_headers.insert(
        "Sec-Ch-Ua",
        "\"Google Chrome\";v=\"126\", \"Chromium\";v=\"126\", \"Not.A/Brand\";v=\"24\"".parse()?,
    );
    default_headers.insert("Sec-Ch-Ua-Mobile", "?0".parse()?);
    default_headers.insert("Sec-Ch-Ua-Platform", "\"Windows\"".parse()?);
    default_headers.insert("Sec-Fetch-Dest", "document".parse()?);
    default_headers.insert("Sec-Fetch-Mode", "navigate".parse()?);
    default_headers.insert("Sec-Fetch-Site", "none".parse()?);
    default_headers.insert("Sec-Fetch-User", "?1".parse()?);
    default_headers.insert("Upgrade-Insecure-Requests", "1".parse()?);
    default_headers.insert("Priority", "u=0, i".parse()?);

    let mut builder = Client::builder()
        .user_agent(args.user_agent.as_deref().unwrap_or(CHROME_UA))
        .default_headers(default_headers)
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
    if args.follow_redirects {
        builder = builder.redirect(reqwest::redirect::Policy::limited(10));
    } else {
        builder = builder.redirect(reqwest::redirect::Policy::none());
    }
    if let Some(proxy_url) = &args.proxy {
        builder = builder.proxy(reqwest::Proxy::all(proxy_url)?);
    }

    Ok(builder.build()?)
}

fn parse_custom_headers(raw: &[String]) -> Result<header::HeaderMap, Box<dyn std::error::Error>> {
    let mut map = header::HeaderMap::new();
    for h in raw {
        let (name, value) = h
            .split_once(':')
            .ok_or_else(|| format!("invalid header format: {h}"))?;
        map.insert(
            header::HeaderName::from_bytes(name.trim().as_bytes())?,
            header::HeaderValue::from_str(value.trim())?,
        );
    }
    Ok(map)
}

fn normalize_urls(line: &str, prefer_https: bool, debug: bool) -> Vec<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return vec![];
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        if debug {
            eprintln!("[normalize] {trimmed} (already has scheme)");
        }
        return vec![trimmed.to_string()];
    }
    let urls: Vec<String> = if prefer_https {
        vec![
            format!("https://{}", trimmed),
            format!("http://{}", trimmed),
        ]
    } else {
        vec![
            format!("http://{}", trimmed),
            format!("https://{}", trimmed),
        ]
    };
    if debug {
        eprintln!("[normalize] {trimmed} -> {urls:?}");
    }
    urls
}

struct JsMatch {
    rt: Runtime,
    source: String,
}

impl JsMatch {
    fn new(script: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let source = match std::fs::read_to_string(script) {
            Ok(s) => s,
            Err(_) => script.to_string(),
        };

        let rt = Runtime::new()?;

        let ctx = Context::full(&rt)?;
        ctx.with(|ctx| -> Result<(), Box<dyn std::error::Error>> {
            ctx.eval::<(), _>(source.as_str())?;
            let globals = ctx.globals();
            let has_match: bool = globals.contains_key("match")?;
            if !has_match {
                return Err("script must define a 'match' function".into());
            }
            Ok(())
        })?;

        Ok(Self { rt, source })
    }

    fn check(&self, url: &str, body: &str, status: u16, resp_headers: &str, debug: bool) -> bool {
        let ctx = match Context::full(&self.rt) {
            Ok(c) => c,
            Err(e) => {
                if debug {
                    eprintln!("[js-error] {url}: failed to create context: {e}");
                }
                return false;
            }
        };

        let result: Result<bool, Box<dyn std::error::Error>> = ctx.with(|ctx| {
            ctx.eval::<(), _>(self.source.as_str())?;
            let f: Function = ctx.globals().get("match")?;
            let matched: bool = f.call((url, body, status, resp_headers))?;
            Ok(matched)
        });

        match result {
            Ok(v) => v,
            Err(e) => {
                if debug {
                    eprintln!("[js-error] {url}: {e}");
                }
                false
            }
        }
    }
}

async fn do_request(
    client: &Client,
    url: &str,
    method: &Method,
    headers: &header::HeaderMap,
    req_body: &[u8],
) -> Result<reqwest::Response, reqwest::Error> {
    let mut req = client.request(method.clone(), url).headers(headers.clone());
    if !req_body.is_empty() {
        req = req.body(req_body.to_vec());
    }
    req.send().await
}

async fn fetch_and_scan(
    client: &Client,
    jsmatch: &JsMatch,
    url: &str,
    method: &Method,
    headers: &header::HeaderMap,
    req_body: &[u8],
    args: &Args,
) {
    if args.debug {
        eprintln!("[request] {url}");
    }
    for attempt in 0..=args.retry {
        match do_request(client, url, method, headers, req_body).await {
            Ok(resp) => {
                match process_response(resp, jsmatch, url, args.debug).await {
                    Ok(true) => {
                        if args.debug {
                            eprintln!("[match] {url}");
                        }
                        println!("{url}");
                        return;
                    }
                    Ok(false) => {
                        if args.debug {
                            eprintln!("[no-match] {url}");
                        }
                        return;
                    }
                    Err(e) => {
                        if args.debug {
                            eprintln!("[js-error] {url}: {e}");
                        }
                        return;
                    }
                }
            }
            Err(e) if attempt < args.retry => {
                if args.debug {
                    eprintln!(
                        "[error] {url}: {e} (attempt {}/{}, retrying)",
                        attempt + 1,
                        args.retry
                    );
                }
                tokio::time::sleep(Duration::from_millis(100 * (attempt as u64 + 1))).await;
            }
            Err(e) => {
                if args.debug {
                    eprintln!("[error] {url}: {e} (all {}/{}, exhausted)", attempt + 1, args.retry);
                }
                return;
            }
        }
    }
}

async fn process_response(
    resp: reqwest::Response,
    jsmatch: &JsMatch,
    url: &str,
    debug: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    let status = resp.status().as_u16();

    let resp_headers: String = resp
        .headers()
        .iter()
        .fold(String::new(), |mut acc, (k, v)| {
            use std::fmt::Write;
            let _ = write!(acc, "{}: {}\n", k.as_str(), v.to_str().unwrap_or(""));
            acc
        });

    if !resp.status().is_success() {
        if debug {
            eprintln!("[status] {url}: {}", resp.status());
        }
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
    }

    let body = String::from_utf8_lossy(&buf);
    Ok(jsmatch.check(url, &body, status, &resp_headers, debug))
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
    let jsmatch = Arc::new(JsMatch::new(&args.script)?);
    let method = match Method::from_bytes(args.method.to_uppercase().as_bytes()) {
        Ok(m) => m,
        Err(_) => {
            eprintln!("warning: invalid method '{}', falling back to GET", args.method);
            Method::GET
        }
    };
    let custom_headers = Arc::new(parse_custom_headers(&args.headers)?);
    let body_data = Arc::new(args.data.as_deref().unwrap_or("").as_bytes().to_vec());

    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let stream = {
        let args = args.clone();
        LinesStream::new(reader.lines())
            .filter_map(|line| async move { line.ok() })
            .flat_map(move |line| {
                let urls = normalize_urls(&line, args.prefer_https, args.debug);
                futures::stream::iter(urls)
            })
    };

    stream
        .for_each_concurrent(args.workers, {
            let client = client.clone();
            let jsmatch = jsmatch.clone();
            let args = args.clone();
            let method = method.clone();
            let custom_headers = custom_headers.clone();
            let body_data = body_data.clone();

            move |url: String| {
                let client = client.clone();
                let jsmatch = jsmatch.clone();
                let args = args.clone();
                let method = method.clone();
                let custom_headers = custom_headers.clone();
                let body_data = body_data.clone();

                async move {
                    fetch_and_scan(
                        &client,
                        &jsmatch,
                        &url,
                        &method,
                        &custom_headers,
                        &body_data,
                        &args,
                    )
                    .await;
                }
            }
        })
        .await;

    Ok(())
}
