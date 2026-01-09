use crate::http_client::{self, RequestOptions};
use eyre::Context;

const SPEED_URL: &str = "https://speed.cloudflare.com/";

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
        let meta: Metadata = serde_json::from_slice(&resp.body).wrap_err_with(|| {
            format!(
                "Parsing metadata response: {:?}",
                String::from_utf8_lossy(&resp.body)
            )
        })?;
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
            .set_header("User-Agent", format!("spidy/{}", env!("CARGO_PKG_VERSION")))
            .set_header("Referer", SPEED_URL)
            .to_owned()
    }

    fn post_req(endpoint: &str) -> RequestOptions {
        RequestOptions::post(Self::join(endpoint))
            .set_header("User-Agent", format!("spidy/{}", env!("CARGO_PKG_VERSION")))
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
    #[serde(default, skip_serializing_if = "skip_if_zero")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colo: Option<Colo>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct Colo {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub iata: String,
    #[serde(default, skip_serializing_if = "skip_if_zero")]
    pub lat: f64,
    #[serde(default, skip_serializing_if = "skip_if_zero")]
    pub lon: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cca2: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub region: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub city: String,
}

fn skip_if_zero<T: PartialEq + Default>(value: &T) -> bool {
    *value == T::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::Response;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// A thread-safe mock HTTP client that can be cloned and used across threads.
    #[derive(Clone)]
    struct MockHttpClient {
        responses: Arc<Mutex<Vec<eyre::Result<Response>>>>,
        requests: Arc<Mutex<Vec<http_client::RequestOptions>>>,
    }

    impl MockHttpClient {
        fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(Vec::new())),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_responses(responses: Vec<eyre::Result<Response>>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses)),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_response(response: Response) -> Self {
            Self::with_responses(vec![Ok(response)])
        }

        fn with_repeated_response(response: Response, count: usize) -> Self {
            let responses: Vec<_> = (0..count).map(|_| Ok(response.clone())).collect();
            Self::with_responses(responses)
        }

        fn with_error(err: &'static str) -> Self {
            Self::with_responses(vec![Err(eyre::eyre!(err))])
        }

        fn with_repeated_error(err: &'static str, count: usize) -> Self {
            let responses: Vec<_> = (0..count).map(|_| Err(eyre::eyre!(err))).collect();
            Self::with_responses(responses)
        }

        fn get_requests(&self) -> Vec<http_client::RequestOptions> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl http_client::HttpClient for MockHttpClient {
        fn request(&self, opts: http_client::RequestOptions) -> eyre::Result<Response> {
            self.requests.lock().unwrap().push(opts);
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                Err(eyre::eyre!("No more mock responses available"))
            } else {
                responses.remove(0)
            }
        }
    }

    fn mock_response(server_timing_ms: Option<u64>, ttfb_ms: u64, total_time_ms: u64) -> Response {
        Response {
            body: vec![],
            server_timing: server_timing_ms.map(Duration::from_millis),
            ttfb: Duration::from_millis(ttfb_ms),
            total_time: Duration::from_millis(total_time_ms),
        }
    }

    fn mock_metadata_response() -> Response {
        let metadata_json = serde_json::json!({
            "city": "San Francisco",
            "clientIp": "192.168.1.1",
            "country": "US",
            "asn": 12345,
            "asOrganization": "Test ISP",
            "region": "California",
            "postalCode": "94102",
            "longitude": "-122.4194",
            "latitude": "37.7749",
            "colo": {
                "iata": "SFO",
                "lat": 37.6213,
                "lon": -122.3790,
                "cca2": "US",
                "region": "California",
                "city": "San Francisco"
            }
        });
        Response {
            body: metadata_json.to_string().into_bytes(),
            server_timing: None,
            ttfb: Duration::from_millis(50),
            total_time: Duration::from_millis(100),
        }
    }

    #[test]
    fn test_get_metadata_success() {
        let mock = MockHttpClient::with_response(mock_metadata_response());
        let client = Client::new(mock.clone());
        let metadata = client.get_metadata().unwrap();

        assert_eq!(metadata.city, "San Francisco");
        assert_eq!(metadata.client_ip, "192.168.1.1");
        assert_eq!(metadata.country, "US");
        assert_eq!(metadata.asn, 12345);
        assert_eq!(metadata.as_organization, "Test ISP");
        assert_eq!(metadata.region, "California");
        assert!(
            matches!(metadata.colo, Some(colo) if colo.iata == "SFO"),
            "Colo iata code mismatch"
        );

        // Verify the request was made correctly
        let requests = mock.get_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].url.contains("/meta"));
    }

    #[test]
    fn test_get_metadata_invalid_json() {
        let mock = MockHttpClient::with_response(Response {
            body: b"not valid json".to_vec(),
            server_timing: None,
            ttfb: Duration::from_millis(50),
            total_time: Duration::from_millis(100),
        });

        let client = Client::new(mock);
        let result = client.get_metadata();

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Parsing metadata response")
        );
    }

    #[test]
    fn test_get_metadata_request_error() {
        let mock = MockHttpClient::with_error("Connection failed");
        let client = Client::new(mock);
        let result = client.get_metadata();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_measure_download_speed_calculation() {
        // server_timing = 10ms, ttfb = 50ms, total_time = 150ms
        // transfer_time = total_time - ttfb = 100ms
        // For 100_000 bytes: speed = (100000 * 8) / (100 / 1000) / 1e6 = 8 Mbps
        // latency = ttfb - server_timing = 50 - 10 = 40ms
        let mock = MockHttpClient::with_response(mock_response(Some(10), 50, 150));
        let client = Client::new(mock);
        let speeds = client.meansure_download(100_000, 1).await;

        assert_eq!(speeds.len(), 1);
        let (speed_mbps, latency_ms) = speeds[0];
        assert!(
            (speed_mbps - 8.0).abs() < 0.001,
            "Expected 8 Mbps, got {}",
            speed_mbps
        );
        assert!(
            (latency_ms - 40.0).abs() < 0.001,
            "Expected 40ms latency, got {}",
            latency_ms
        );
    }

    #[tokio::test]
    async fn test_measure_download_multiple_iterations() {
        let mock = MockHttpClient::with_repeated_response(mock_response(Some(10), 50, 150), 3);
        let client = Client::new(mock);
        let speeds = client.meansure_download(100_000, 3).await;

        assert_eq!(speeds.len(), 3);
    }

    #[tokio::test]
    async fn test_measure_download_handles_errors() {
        let mock = MockHttpClient::with_repeated_error("Network error", 2);
        let client = Client::new(mock);
        let speeds = client.meansure_download(100_000, 2).await;

        // Errors are logged but not included in results
        assert_eq!(speeds.len(), 0);
    }

    #[tokio::test]
    async fn test_measure_upload_speed_calculation() {
        // For upload: speed is based on server_timing
        // server_timing = 100ms, size = 100_000 bytes
        // speed = (100000 * 8) / (100 / 1000) / 1e6 = 8 Mbps
        // latency = ttfb - server_timing = 150 - 100 = 50ms
        let mock = MockHttpClient::with_response(mock_response(Some(100), 150, 200));
        let client = Client::new(mock.clone());
        let speeds = client.meansure_upload(100_000, 1).await;

        assert_eq!(speeds.len(), 1);
        let (speed_mbps, latency_ms) = speeds[0];
        assert!(
            (speed_mbps - 8.0).abs() < 0.001,
            "Expected 8 Mbps, got {}",
            speed_mbps
        );
        assert!(
            (latency_ms - 50.0).abs() < 0.001,
            "Expected 50ms latency, got {}",
            latency_ms
        );

        // Verify the request was made to the upload endpoint
        let requests = mock.get_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].url.contains("/__up"));
    }

    #[tokio::test]
    async fn test_measure_upload_without_server_timing() {
        // When server_timing is None, it defaults to 1.0ms
        let mock = MockHttpClient::with_response(mock_response(None, 50, 100));
        let client = Client::new(mock);
        let speeds = client.meansure_upload(1000, 1).await;

        assert_eq!(speeds.len(), 1);
        let (speed_mbps, _) = speeds[0];
        // speed = (1000 * 8) / (1 / 1000) / 1e6 = 8000 Mbps (unrealistic but expected with 1ms)
        assert!(speed_mbps > 0.0);
    }

    #[tokio::test]
    async fn test_measure_latency() {
        // latency = ttfb - server_timing = 50 - 10 = 40ms
        let mock = MockHttpClient::with_repeated_response(mock_response(Some(10), 50, 100), 5);
        let client = Client::new(mock);
        let latencies = client.meansure_latency(5).await;

        assert_eq!(latencies.len(), 5);
        for latency in latencies {
            assert!(
                (latency - 40.0).abs() < 0.001,
                "Expected 40ms, got {}",
                latency
            );
        }
    }

    #[tokio::test]
    async fn test_measure_latency_without_server_timing() {
        // When server_timing is None (defaults to 0), latency = ttfb = 50ms
        let mock = MockHttpClient::with_response(mock_response(None, 50, 100));
        let client = Client::new(mock);
        let latencies = client.meansure_latency(1).await;

        assert_eq!(latencies.len(), 1);
        assert!((latencies[0] - 50.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_download_request_has_correct_url() {
        let mock = MockHttpClient::with_response(mock_response(Some(10), 50, 150));
        let client = Client::new(mock.clone());
        let _ = client.meansure_download(1024, 1).await;

        let requests = mock.get_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].url.contains("/__down?bytes=1024"));
    }

    #[tokio::test]
    async fn test_upload_request_has_body() {
        let mock = MockHttpClient::with_response(mock_response(Some(100), 150, 200));
        let client = Client::new(mock.clone());
        let _ = client.meansure_upload(512, 1).await;

        let requests = mock.get_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body.len(), 512);
        assert!(requests[0].headers.contains_key("Content-Length"));
    }

    #[test]
    fn test_join_with_leading_slash() {
        let result = Client::<MockHttpClient>::join("/meta");
        assert_eq!(result, "https://speed.cloudflare.com/meta");
    }

    #[test]
    fn test_join_without_leading_slash() {
        let result = Client::<MockHttpClient>::join("meta");
        assert_eq!(result, "https://speed.cloudflare.com/meta");
    }

    #[test]
    fn test_get_req_sets_headers() {
        let opts = Client::<MockHttpClient>::get_req("/test");
        assert!(opts.headers.contains_key("User-Agent"));
        assert!(opts.headers.contains_key("Referer"));
        assert_eq!(opts.headers.get("Referer").unwrap(), SPEED_URL);
    }

    #[test]
    fn test_post_req_sets_headers() {
        let opts = Client::<MockHttpClient>::post_req("/test");
        assert!(opts.headers.contains_key("User-Agent"));
        assert!(opts.headers.contains_key("Referer"));
        assert_eq!(opts.method, http_client::Method::Post);
    }

    #[test]
    fn test_metadata_serialization() {
        let metadata = Metadata {
            city: "London".to_string(),
            client_ip: "10.0.0.1".to_string(),
            country: "UK".to_string(),
            asn: 54321,
            as_organization: "ISP Co".to_string(),
            region: "England".to_string(),
            postal_code: "EC1A".to_string(),
            longitude: "-0.1276".to_string(),
            latitude: "51.5074".to_string(),
            colo: Some(Colo {
                iata: "LON".to_string(),
                lat: 51.4700,
                lon: -0.4543,
                cca2: "GB".to_string(),
                region: "England".to_string(),
                city: "London".to_string(),
            }),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: Metadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.city, metadata.city);
        assert_eq!(deserialized.client_ip, metadata.client_ip);
        assert_eq!(deserialized.asn, metadata.asn);
    }

    #[test]
    fn test_metadata_with_empty_optional_fields() {
        let json = r#"{"clientIp": "1.2.3.4"}"#;
        let metadata: Metadata = serde_json::from_str(json).unwrap();

        assert_eq!(metadata.client_ip, "1.2.3.4");
        assert!(metadata.city.is_empty());
        assert!(metadata.country.is_empty());
        assert_eq!(metadata.asn, 0);
    }

    #[test]
    fn test_client_with_progress_bar() {
        let mock = MockHttpClient::new();
        let pb = indicatif::ProgressBar::new(100);
        let client = Client::new(mock).with_progress_bar(Some(pb));

        assert!(client.pb.is_some());
    }
}
