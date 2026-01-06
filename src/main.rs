use clap::Parser;
use eyre::Context;
use indicatif::{ProgressBar, ProgressStyle};
use parse_size::parse_size;
use std::io::Write;
use std::str::FromStr;
use tabwriter::TabWriter;

mod cloudflare;

#[derive(clap::Parser)]
struct Command {
    /// Tests to run in the format <direction>=<human_bytes_format>=<iterations> (e.g., u=10MB=5)
    #[clap(
        long = "test",
        short,
        default_value = "d=100kB=10,d=1MB=8,u=100kB=8,u=1MB=6,d=10MB=6,u=10MB=4,d=25MB=4",
        value_delimiter = ','
    )]
    tests: Vec<TestSpec>,

    /// Output format
    #[clap(long, short, value_enum, default_value_t = Format::Text)]
    format: Format,
}

#[derive(Debug, serde::Serialize)]
struct TestLog {
    size: usize,
    #[serde(skip)]
    raw_size: String,
    speeds: Vec<f64>,
    latencies: Vec<f64>,
    completed_iterations: usize,
    interations: usize,
}

#[derive(Debug, Default, serde::Serialize)]
struct Report {
    download_speeds: Vec<TestLog>,
    upload_speeds: Vec<TestLog>,
    all_download_speeds: Vec<f64>,
    all_upload_speeds: Vec<f64>,
    download_latencies: Vec<f64>,
    upload_latencies: Vec<f64>,
    download_p90: f64,
    download_p75: f64,
    upload_p90: f64,
    upload_p75: f64,

    download_jitter: f64,
    upload_jitter: f64,

    download_median_latency: f64,
    upload_median_latency: f64,

    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<cloudflare::Metadata>,
}

async fn run() -> eyre::Result<()> {
    let Command { tests, format } = Command::parse();

    let metadata = cloudflare::get_metadata()?;

    let mut tw = TabWriter::new(std::io::stderr());
    tw.write_fmt(format_args!(
        "
Cloudflare Speed Test Client
=============================
Server Location:\t{} {}
ASN:\t{} ({})
Your IP:\t{}
",
        metadata.city, metadata.country, metadata.asn, metadata.as_organization, metadata.client_ip
    ))?;
    tw.flush()?;

    let mut report = Report {
        metadata: Some(metadata),
        ..Default::default()
    };

    let pb = ProgressBar::new(tests.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} test(s) ({eta})",
        )
        .wrap_err_with(|| "Creating progress bar style")?
        .progress_chars("#>-"),
    );

    let mut completed_tests = 0;
    let t0 = std::time::Instant::now();
    for test in &tests {
        match test.direction {
            Direction::Download => {
                let result = cloudflare::meansure_download(test.size, test.iterations).await;
                let (s, l) = result
                    .iter()
                    .map(|s| (s.0, s.1))
                    .unzip::<f64, f64, Vec<_>, Vec<_>>();
                report.download_speeds.push(TestLog {
                    size: test.size,
                    raw_size: test.raw_size.clone(),
                    speeds: s.clone(),
                    latencies: l.clone(),
                    completed_iterations: s.len(),
                    interations: test.iterations,
                });
                report.all_download_speeds.extend(s);
                report.download_latencies.extend(l);
            }
            Direction::Upload => {
                let result = cloudflare::meansure_upload(test.size, test.iterations).await;
                let (s, l) = result
                    .iter()
                    .map(|s| (s.0, s.1))
                    .unzip::<f64, f64, Vec<_>, Vec<_>>();
                report.upload_speeds.push(TestLog {
                    size: test.size,
                    raw_size: test.raw_size.clone(),
                    speeds: s.clone(),
                    latencies: l.clone(),
                    completed_iterations: s.len(),
                    interations: test.iterations,
                });
                report.all_upload_speeds.extend(s);
                report.upload_latencies.extend(l);
            }
        }
        completed_tests += 1;
        pb.set_position(completed_tests as u64);
    }
    pb.finish_and_clear();

    eprintln!("\nAll tests completed in {:.2?}\n", t0.elapsed());

    report.download_p90 = quartile(&report.all_download_speeds, 0.9);
    report.download_p75 = quartile(&report.all_download_speeds, 0.75);
    report.upload_p90 = quartile(&report.all_upload_speeds, 0.9);
    report.upload_p75 = quartile(&report.all_upload_speeds, 0.75);
    report.download_jitter = jitter(&report.download_latencies);
    report.upload_jitter = jitter(&report.upload_latencies);
    report.download_median_latency = median(&report.download_latencies);
    report.upload_median_latency = median(&report.upload_latencies);

    if format == Format::Json {
        let json_report =
            serde_json::to_string_pretty(&report).wrap_err_with(|| "Serializing report to JSON")?;
        println!("{}", json_report);
        return Ok(());
    }

    let mut tw = TabWriter::new(std::io::stdout());
    tw.write_all(b"Download Results:\n")?;
    for log in &report.download_speeds {
        tw.write_fmt(format_args!(
            "\t({}/{})\t{}\t{:.2}\tMbps\n",
            log.completed_iterations,
            log.interations,
            log.raw_size,
            median(&log.speeds)
        ))?;
    }
    tw.flush()?;

    let latency_median = report.download_median_latency;
    tw.write_fmt(format_args!(
        "Download Latency (Median): \t{:.2} ms\n",
        latency_median
    ))?;
    tw.write_fmt(format_args!(
        "Download Jitter: \t{:.2} ms\n",
        report.download_jitter
    ))?;
    tw.write_all(b"Overall Download:\n")?;
    tw.write_fmt(format_args!(
        "\t90th Percentile:\t{:.2}\tMbps\n",
        report.download_p90
    ))?;
    tw.write_fmt(format_args!(
        "\t75th Percentile:\t{:.2}\tMbps\n",
        report.download_p75
    ))?;
    tw.flush()?;

    tw.write_all(b"\nUpload Results:\n")?;
    for log in &report.upload_speeds {
        tw.write_fmt(format_args!(
            "\t({}/{})\t{}\t{:.2}\tMbps\n",
            log.completed_iterations,
            log.interations,
            log.raw_size,
            median(&log.speeds)
        ))?;
    }
    tw.flush()?;

    tw.write_fmt(format_args!(
        "Upload Latency (Median): {:.2} ms\n",
        report.upload_median_latency
    ))?;
    tw.write_fmt(format_args!(
        "Upload Jitter: {:.2} ms\n",
        report.upload_jitter
    ))?;
    tw.write_all(b"Overall Upload:\n")?;
    tw.write_fmt(format_args!(
        "\t90th Percentile:\t{:.2}\tMbps\n",
        report.upload_p90
    ))?;
    tw.write_fmt(format_args!(
        "\t75th Percentile:\t{:.2}\tMbps\n",
        report.upload_p75
    ))?;
    tw.flush()?;
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("Error: {:?}", err);
        std::process::exit(1);
    }
}

#[derive(Debug, Clone)]
enum Direction {
    Upload,
    Download,
}

impl FromStr for Direction {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "u" => Ok(Direction::Upload),
            "d" => Ok(Direction::Download),
            "up" => Ok(Direction::Upload),
            "down" => Ok(Direction::Download),
            _ => Err(eyre::eyre!("Invalid direction: {}", s)),
        }
    }
}

#[derive(Debug, Clone, clap::ValueEnum, PartialEq, Eq)]
enum Format {
    Json,
    Text,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Json => write!(f, "json"),
            Format::Text => write!(f, "text"),
        }
    }
}

#[derive(Clone)]
struct TestSpec {
    direction: Direction,
    size: usize,
    raw_size: String,
    iterations: usize,
}

impl FromStr for TestSpec {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Format "<direction>=<human_bytes_format>=<iterations>"
        let parts: Vec<&str> = s.split('=').collect();
        if parts.len() != 3 {
            return Err(eyre::eyre!(
                "Invalid test spec format, expected <direction>=<human_bytes_format>=<iterations>"
            ));
        }

        let direction = parts[0]
            .parse::<Direction>()
            .wrap_err_with(|| "Parsing direction from test spec")?;

        let size = parse_size(parts[1]).wrap_err_with(|| "Parsing size from test spec")? as usize;

        let iterations = parts[2]
            .parse::<usize>()
            .wrap_err_with(|| "Parsing iterations from test spec")?;
        Ok(Self {
            direction,
            size,
            raw_size: parts[1].to_string(),
            iterations,
        })
    }
}

fn quartile(data: &[f64], q: f64) -> f64 {
    let mut sorted: Vec<f64> = data.to_vec();
    if sorted.is_empty() {
        return 0.0;
    }
    sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    let pos = q * (sorted.len() - 1) as f64;
    let base = pos.floor() as usize;
    let rest = pos - base as f64;
    if base + 1 < sorted.len() {
        sorted[base] + rest * (sorted[base + 1] - sorted[base])
    } else {
        sorted[base]
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

fn median(data: &[f64]) -> f64 {
    let mut sorted: Vec<f64> = data.to_vec();
    if sorted.is_empty() {
        return 0.0;
    }
    sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}
