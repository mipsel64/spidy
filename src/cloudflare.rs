use crate::http_client::{self, RequestOptions};
use eyre::Context;

const SPEED_URL: &str = "https://speed.cloudflare.com/";
const USER_AGENT: &str = constcat::concat!("spidt/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
pub struct Client<R> {
    http_client: R,
    pb: Option<indicatif::ProgressBar>,
}

impl<R> Client<R>
where
    R: http_client::HttpClient + Clone + Send + 'static,
{
    pub fn new(http_client: R) -> Self {
        Self {
            http_client,
            pb: None,
        }
    }

    pub fn with_progress_bar(mut self, pb: Option<indicatif::ProgressBar>) -> Self {
        self.pb = pb;
        self
    }

    pub fn get_metadata(&self) -> eyre::Result<Metadata> {
        let resp = self
            .http_client
            .request(
                Self::get_req("/meta")
                    .set_header("Accept", "application/json")
                    .to_owned(),
            )
            .wrap_err_with(|| "Sending metadata request")?;
        let meta: Metadata =
            serde_json::from_slice(&resp.body).wrap_err_with(|| "Parsing metadata response")?;
        Ok(meta)
    }

    pub async fn meansure_download(&self, size: usize, iterations: usize) -> Vec<(f64, f64)> {
        let mut speeds = Vec::with_capacity(iterations);
        let size_f64 = size as f64;
        for i in 0..iterations {
            match self.download_async(size).await {
                Ok(result) => {
                    let transfer_time = (result.total_time - result.ttfb).as_millis() as f64;
                    let speed_mbps = (size_f64 * 8.0) / (transfer_time / 1000.0) / 1e6;
                    let latency_ms = result.ttfb.as_millis() as f64
                        - result.server_timing.unwrap_or_default().as_millis() as f64;
                    speeds.push((speed_mbps, latency_ms));
                    self.pb_inc(1);
                }
                Err(err) => {
                    eprintln!("Error measuring download (iteration {}): {:?}", i + 1, err);
                }
            }
        }
        speeds
    }

    pub async fn meansure_latency(&self, iterations: usize) -> Vec<f64> {
        let mut latencies = Vec::with_capacity(iterations);
        for i in 0..iterations {
            match self.download_async(0).await {
                Ok(result) => {
                    let latency_ms = result.ttfb.as_millis() as f64
                        - result.server_timing.unwrap_or_default().as_millis() as f64;
                    latencies.push(latency_ms);
                    self.pb_inc(1);
                }
                Err(err) => {
                    eprintln!("Error measuring latency (ping {}): {:?}", i + 1, err);
                }
            }
        }
        latencies
    }

    pub async fn meansure_upload(&self, size: usize, iterations: usize) -> Vec<(f64, f64)> {
        let mut speeds = Vec::with_capacity(iterations);
        let size_f64 = size as f64;
        for i in 0..iterations {
            match self.upload_async(size).await {
                Ok(result) => {
                    let transfer_time = result
                        .server_timing
                        .map(|d| d.as_millis() as f64)
                        .unwrap_or(1.0);
                    let speed_mbps = (size_f64 * 8.0) / (transfer_time / 1000.0) / 1e6;
                    let latency_ms = result.ttfb.as_millis() as f64 - transfer_time;
                    speeds.push((speed_mbps, latency_ms));
                    self.pb_inc(1);
                }
                Err(err) => {
                    eprintln!("Error measuring upload (iteration {}): {:?}", i + 1, err);
                }
            }
        }
        speeds
    }

    async fn download_async(&self, size: usize) -> eyre::Result<http_client::Response> {
        let client = self.http_client.clone();
        tokio::task::spawn_blocking(move || {
            client
                .request(Self::get_req(&format!("/__down?bytes={}", size)).drop_response_body(true))
        })
        .await
        .wrap_err_with(|| "Task panicked")?
    }

    async fn upload_async(&self, size: usize) -> eyre::Result<http_client::Response> {
        let client = self.http_client.clone();
        tokio::task::spawn_blocking(move || {
            let body = vec![0u8; size];
            client.request(
                Self::post_req("/__up")
                    .drop_response_body(true)
                    .with_body(body)
                    .set_header("Content-Length", size.to_string())
                    .to_owned(),
            )
        })
        .await
        .wrap_err_with(|| "Task panicked")?
    }

    fn join(endpoint: &str) -> String {
        let endpoint = endpoint.trim_start_matches('/');
        format!("{}{}", SPEED_URL, endpoint)
    }

    fn get_req(endpoint: &str) -> RequestOptions {
        RequestOptions::get(Self::join(endpoint))
            .set_header("User-Agent", USER_AGENT)
            .set_header("Referer", SPEED_URL)
            .to_owned()
    }

    fn post_req(endpoint: &str) -> RequestOptions {
        RequestOptions::post(Self::join(endpoint))
            .set_header("User-Agent", USER_AGENT)
            .set_header("Referer", SPEED_URL)
            .to_owned()
    }

    fn pb_inc(&self, n: u64) {
        if let Some(pb) = &self.pb {
            pb.inc(n);
        }
    }
}

#[derive(Debug, serde::Deserialize, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub city: String,
    pub client_ip: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub country: String,
    #[serde(default)]
    pub asn: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub as_organization: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub region: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub postal_code: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub longitude: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub latitude: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub colo: String,
}
