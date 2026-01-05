use std::str::FromStr;

use clap::Parser;
use eyre::Context;
use parse_size::parse_size;

mod cloudflare;

#[derive(clap::Parser)]
struct Command {
    /// Tests to run in the format <human_bytes_format>=<iterations> (e.g., 10MB=5)
    #[clap(
        long,
        short,
        default_value = "100kB=10,1MB=8,10MB=6,25MB=4,100MB=3",
        value_delimiter = ','
    )]
    down: Vec<TestSpec>,

    /// Tests to run in the format <human_bytes_format>=<iterations> (e.g., 10MB=5)
    #[clap(
        long,
        short,
        default_value = "100kB=8,1MB=6,10MB=4",
        value_delimiter = ','
    )]
    up: Vec<TestSpec>,
}

async fn run() -> eyre::Result<()> {
    let Command { mut up, mut down } = Command::parse();

    // Sort by size ascending
    up.sort_unstable_by_key(|test| test.size);
    down.sort_unstable_by_key(|test| test.size);

    let client = cloudflare::Client::new();

    let metadata = client.get_metadata()?;

    println!("Server Location: `{} {}`", metadata.city, metadata.country);
    println!("Your IP: {}", metadata.client_ip);

    let mut agg_speeds = Vec::new();
    let mut all_speeds = Vec::new();

    let mut latencies = Vec::new();
    for test in &down {
        let result = client.meansure_download(test.size, test.iterations).await;
        let (s, l) = result
            .iter()
            .map(|s| (s.0, s.1))
            .unzip::<f64, f64, Vec<_>, Vec<_>>();
        let mean_speed = s.iter().sum::<f64>() / s.len() as f64;
        agg_speeds.push((test.raw_size.clone(), mean_speed));
        all_speeds.extend(s);
        latencies.extend(l);
    }

    println!("\nDownload Results:");
    for (size, speed) in &agg_speeds {
        println!("  {:<7}: {:.2} Mbps", size, speed);
    }

    latencies.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

    let mid = latencies.len() / 2;

    let latency_median = if latencies.len() % 2 == 0 {
        (latencies[mid - 1] + latencies[mid]) / 2.0
    } else {
        latencies[mid]
    };

    println!("Download Latency (Median): {:.2} ms", latency_median);
    println!("Download Jitter: {:.2} ms", jitter(&latencies));

    if let Some(speed) = quartile(&all_speeds, 0.90) {
        println!("Overall Download: {:.2} Mbps", speed);
    }

    agg_speeds.clear();
    all_speeds.clear();

    for test in &up {
        let result = client.meansure_upload(test.size, test.iterations).await;
        let (s, l) = result
            .iter()
            .map(|s| (s.0, s.1))
            .unzip::<f64, f64, Vec<_>, Vec<_>>();
        let mean_speed = s.iter().sum::<f64>() / s.len() as f64;
        agg_speeds.push((test.raw_size.clone(), mean_speed));
        all_speeds.extend(s);
        latencies.extend(l);
    }

    println!("\nUpload Results:");
    for (size, speed) in &agg_speeds {
        println!("  {:<7}: {:.2} Mbps", size, speed);
    }

    latencies.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = latencies.len() / 2;
    let latency_median = if latencies.len() % 2 == 0 {
        (latencies[mid - 1] + latencies[mid]) / 2.0
    } else {
        latencies[mid]
    };
    println!("Upload Latency (Median): {:.2} ms", latency_median);
    println!("Upload Jitter: {:.2} ms", jitter(&latencies));

    if let Some(speed) = quartile(&all_speeds, 0.90) {
        println!("Overall Upload: {:.2} Mbps", speed);
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("Error: {:?}", err);
        std::process::exit(1);
    }
}

#[derive(Clone)]
struct TestSpec {
    size: usize,
    raw_size: String,
    iterations: usize,
}

impl FromStr for TestSpec {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Format "<human_bytes_format>=<iterations>"
        let parts: Vec<&str> = s.split('=').collect();
        if parts.len() != 2 {
            return Err(eyre::eyre!(
                "Invalid test spec format, expected <human_bytes_format>=<iterations>"
            ));
        }

        let size = parse_size(parts[0]).wrap_err_with(|| "Parsing size from test spec")? as usize;

        let iterations = parts[1]
            .parse::<usize>()
            .wrap_err_with(|| "Parsing iterations from test spec")?;
        Ok(Self {
            size,
            raw_size: parts[0].to_string(),
            iterations,
        })
    }
}

fn quartile(data: &[f64], percentile: f64) -> Option<f64> {
    let mut sorted: Vec<f64> = data.to_vec();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    let pos = percentile * (sorted.len() - 1) as f64;
    let base = pos.floor() as usize;
    let rest = pos - base as f64;
    if base + 1 < sorted.len() {
        Some(sorted[base] + rest * (sorted[base + 1] - sorted[base]))
    } else {
        Some(sorted[base])
    }
}

fn jitter(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let mut sum_diff = 0.0;
    for i in 1..data.len() {
        sum_diff += (data[i] - data[i - 1]).abs();
    }
    sum_diff / (data.len() - 1) as f64
}
