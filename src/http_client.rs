use std::collections::HashMap;

use eyre::Context;

pub trait HttpClient: Clone + Send {
    fn request(&self, opts: RequestOptions) -> eyre::Result<Response>;
}

#[derive(Clone)]
pub struct Curl;

impl HttpClient for Curl {
    fn request(&self, opts: RequestOptions) -> eyre::Result<Response> {
        let mut handle = curl::easy::Easy::new();
        let RequestOptions {
            url,
            headers,
            method,
            mut body,
            drop_response_body,
        } = opts;

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

        let mut server_timing = None;
        let mut buf = Vec::new();
        {
            let mut transfer = handle.transfer();
            transfer
                .header_function(|header_data| {
                    let Some(header_str) = std::str::from_utf8(header_data).ok() else {
                        return true;
                    };
                    if let Some(dur) = header_str
                        .to_lowercase()
                        .strip_prefix("server-timing: cfrequestduration;dur=")
                        .and_then(|s| s.trim().parse::<f64>().ok())
                    {
                        server_timing = Some(std::time::Duration::from_millis(dur as u64));
                    }
                    true
                })
                .wrap_err_with(|| "Setting header function for curl request")?;

            transfer
                .write_function(|data| {
                    if !drop_response_body {
                        // Only store the response body if we are not dropping it
                        buf.extend_from_slice(data);
                    }
                    Ok(data.len())
                })
                .wrap_err_with(|| "Setting write function for curl request")?;

            transfer
                .perform()
                .wrap_err_with(|| "Performing curl request")?;
        }

        let starttransfer = handle
            .starttransfer_time()
            .wrap_err_with(|| "Getting starttransfer time")?;

        let total = handle.total_time().wrap_err_with(|| "Getting total time")?;

        Ok(Response {
            body: buf,
            server_timing,
            ttfb: starttransfer,
            total_time: total,
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct RequestOptions {
    pub url: String,
    pub headers: HashMap<String, String>,
    pub method: Method,
    pub body: Vec<u8>,
    pub drop_response_body: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub body: Vec<u8>,
    pub server_timing: Option<std::time::Duration>,
    pub ttfb: std::time::Duration,
    pub total_time: std::time::Duration,
}

impl RequestOptions {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: HashMap::new(),
            method: Method::Get,
            body: Vec::new(),
            drop_response_body: false,
        }
    }

    pub fn post(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: HashMap::new(),
            method: Method::Post,
            body: Vec::new(),
            drop_response_body: false,
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

    pub fn drop_response_body(mut self, drop: bool) -> Self {
        self.drop_response_body = drop;
        self
    }
}
