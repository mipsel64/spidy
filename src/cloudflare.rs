use std::collections::HashMap;

use curl::easy::Easy;
use eyre::Context;

const SPEED_URL: &str = "https://speed.cloudflare.com/";
const USER_AGENT: &str = constcat::concat!("spidt/", env!("CARGO_PKG_VERSION"));

pub fn get_metadata() -> eyre::Result<Metadata> {
    let meta: Metadata =
        get(RequestOptions::get("/meta")).wrap_err_with(|| "Sending metadata request")?;
    Ok(meta)
}

pub async fn meansure_download(size: usize, iterations: usize) -> Vec<(f64, f64)> {
    let mut speeds = Vec::with_capacity(iterations);
    let size_f64 = size as f64;
    for i in 0..iterations {
        match download_async(size).await {
            Ok(result) => {
                let transfer_time = result.total_time - result.ttfb;
                let speed_mbps = size_f64 * 8.0 / transfer_time.as_secs_f64() / 1_000_000.0;
                let latency_ms = (result.ttfb - result.server_processing).as_millis() as f64;
                speeds.push((speed_mbps, latency_ms));
            }
            Err(err) => {
                eprintln!("Error measuring download (iteration {}): {:?}", i + 1, err);
            }
        }
    }
    speeds
}

pub async fn meansure_upload(size: usize, iterations: usize) -> Vec<(f64, f64)> {
    let mut speeds = Vec::with_capacity(iterations);
    let size_f64 = size as f64;
    for i in 0..iterations {
        match upload_async(size).await {
            Ok(result) => {
                let Some(transfer_time) = result.server_timing else {
                    eprintln!("No server timing info for upload (iteration {})", i + 1);
                    continue;
                };
                let speed_mbps = size_f64 * 8.0 / transfer_time.as_secs_f64() / 1_000_000.0;
                let latency_ms = (result.ttfb - result.server_processing).as_millis() as f64;
                speeds.push((speed_mbps, latency_ms));
            }
            Err(err) => {
                eprintln!("Error measuring upload (iteration {}): {:?}", i + 1, err);
            }
        }
    }
    speeds
}

pub async fn download_async(size: usize) -> eyre::Result<MeasurementResult> {
    tokio::task::spawn_blocking(move || download(size))
        .await
        .wrap_err_with(|| "Task panicked")?
}

pub fn download(size: usize) -> eyre::Result<MeasurementResult> {
    measure_request(RequestOptions::get(format!("/__down?bytes={}", size)))
}

pub async fn upload_async(size: usize) -> eyre::Result<MeasurementResult> {
    tokio::task::spawn_blocking(move || upload(size))
        .await
        .wrap_err_with(|| "Task panicked")?
}

pub fn upload(size: usize) -> eyre::Result<MeasurementResult> {
    let body = vec![0u8; size];
    measure_request(
        RequestOptions::post("/__up")
            .with_body(body)
            .set_header("Content-Length", size.to_string())
            .to_owned(),
    )
}

fn measure_request(opts: RequestOptions) -> eyre::Result<MeasurementResult> {
    let mut handle = curl::easy::Easy::new();
    let RequestOptions {
        endpoint,
        headers,
        method,
        mut body,
    } = opts;

    let url = join(&endpoint);

    let mut list = curl::easy::List::new();
    for (key, value) in headers {
        list.append(&format!("{}: {}", key, value))
            .wrap_err_with(|| format!("Adding header {}: {}", key, value))?;
    }

    handle
        .http_headers(list)
        .wrap_err_with(|| "Setting headers for curl request")?;

    handle
        .url(url.as_str())
        .wrap_err_with(|| "Setting URL for curl request")?;

    if method == Method::Post {
        handle
            .post(true)
            .wrap_err_with(|| "Setting POST method for curl request")?;
        if !body.is_empty() {
            handle
                .post_field_size(body.len() as u64)
                .wrap_err_with(|| "Setting POST field size for curl request")?;
            handle
                .read_function(move |buf| {
                    let len = std::cmp::min(buf.len(), body.len());
                    buf[..len].copy_from_slice(&body[..len]);
                    body.drain(..len);
                    Ok(len)
                })
                .wrap_err_with(|| "Setting read function for curl POST body")?;
        }
    }

    if method == Method::Get {
        handle
            .get(true)
            .wrap_err_with(|| "Setting GET method for curl request")?;
    }

    handle
        .write_function(|data| Ok(data.len())) // Discard response body
        .wrap_err_with(|| "Setting write function for curl request")?;
    let mut server_timing = None;
    {
        let mut transfer = handle.transfer();
        transfer
            .header_function(|header_data| {
                let Some(header_str) = std::str::from_utf8(header_data).ok() else {
                    return true;
                };
                if let Some(dur) = header_str
                    .strip_prefix("Server-Timing: cfRequestDuration;dur=")
                    .and_then(|s| s.trim().parse::<f64>().ok())
                {
                    server_timing = Some(std::time::Duration::from_millis(dur as u64));
                }
                true
            })
            .wrap_err_with(|| "Setting header function for curl request")?;
        transfer
            .perform()
            .wrap_err_with(|| "Performing curl request")?;
    }

    let pretransfer = handle
        .pretransfer_time()
        .wrap_err_with(|| "Getting pretransfer time")?;

    let starttransfer = handle
        .starttransfer_time()
        .wrap_err_with(|| "Getting starttransfer time")?;

    let server_processing = starttransfer - pretransfer;

    let total = handle.total_time().wrap_err_with(|| "Getting total time")?;

    Ok(MeasurementResult {
        server_timing,
        ttfb: starttransfer,
        total_time: total,
        server_processing,
    })
}

fn get<T>(opts: RequestOptions) -> eyre::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let mut handle = Easy::new();

    let RequestOptions {
        endpoint, headers, ..
    } = opts;

    let url = join(&endpoint);
    let mut list = curl::easy::List::new();
    for (key, value) in headers {
        list.append(&format!("{}: {}", key, value))
            .wrap_err_with(|| format!("Adding header {}: {}", key, value))?;
    }
    list.append(&format!("Referer: {}", SPEED_URL))
        .wrap_err_with(|| format!("Adding referer header: Referer: {}", SPEED_URL))?;

    list.append(&format!("User-Agent: {}", USER_AGENT))
        .wrap_err_with(|| "Adding user agent header")?;

    handle
        .http_headers(list)
        .wrap_err_with(|| "Setting headers for curl request")?;

    handle
        .url(url.as_str())
        .wrap_err_with(|| "Setting URL for curl request")?;
    handle
        .get(true)
        .wrap_err_with(|| "Setting GET method for curl request")?;

    let mut buf = Vec::new();
    {
        let mut transfer = handle.transfer();

        transfer
            .write_function(|data| {
                buf.extend_from_slice(data);
                Ok(data.len())
            })
            .wrap_err_with(|| "Setting write function for curl request")?;

        transfer
            .perform()
            .wrap_err_with(|| "Performing curl request")?;
    }

    let result = serde_json::from_slice::<T>(&buf).wrap_err_with(|| "Parsing JSON response")?;
    Ok(result)
}

fn join(endpoint: &str) -> String {
    // Remove leading slash from endpoint if present
    let endpoint = if let Some(stripped) = endpoint.strip_prefix('/') {
        stripped
    } else {
        endpoint
    };

    format!("{}{}", SPEED_URL, endpoint)
}

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub city: String,
    pub client_ip: String,
    pub country: String,
}

#[derive(Debug, Clone)]
pub(super) struct RequestOptions {
    pub endpoint: String,
    pub headers: HashMap<String, String>,
    pub method: Method,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

impl RequestOptions {
    pub fn get(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            headers: HashMap::new(),
            method: Method::Get,
            body: Vec::new(),
        }
    }

    pub fn post(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            headers: HashMap::new(),
            method: Method::Post,
            body: Vec::new(),
        }
    }

    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    pub fn set_header(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeasurementResult {
    pub server_timing: Option<std::time::Duration>,
    pub server_processing: std::time::Duration,
    pub ttfb: std::time::Duration,
    pub total_time: std::time::Duration,
}
