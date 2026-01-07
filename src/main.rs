use clap::Parser;
use console::Term;
use eyre::Context;
use indicatif::{ProgressBar, ProgressStyle};
use parse_size::parse_size;
use std::io::Write;
use std::str::FromStr;
use tabwriter::TabWriter;

use crate::helpers::{jitter, median, quartile};

mod cloudflare;
mod helpers;
mod http_client;

const DEFAULT_LATENCY_ITERATIONS: usize = 20;

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

#[derive(Debug, serde::Serialize, Default)]
struct LatencyStats {
    min: f64,
    max: f64,
    average: f64,
    median: f64,
    jitter: f64,
    p90: f64,
    p75: f64,
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

    latency: LatencyStats,
}

async fn run() -> eyre::Result<()> {
    let Command { tests, format } = Command::parse();

    let total_tests =
        tests.iter().map(|t| t.iterations).sum::<usize>() + DEFAULT_LATENCY_ITERATIONS;
    let pb = ProgressBar::new(total_tests as u64);
    let cf = cloudflare::Client::new(http_client::Curl).with_progress_bar(Some(pb.clone()));
    let metadata = cf.get_metadata()?;

    let mut tw = TabWriter::new(std::io::stderr());
    tw.write_fmt(format_args!(
        "
CLOUDFLARE SPEED TEST CLI
=========================

Server Location:\t{} - {}
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

    pb.set_style(
        ProgressStyle::with_template(if Term::stdout().size().1 > 60 {
            "{spinner:.green} [{elapsed_precise}] [{bar:20}] {pos}/{len} {wide_msg}"
        } else {
            "{spinner:.green} [{elapsed_precise}] [{bar:20}] {pos}/{len}"
        })
        .wrap_err_with(|| "Creating progress bar style")?
        .progress_chars("#>-"),
    );
    let mut completed_tests = 0;

    pb.set_message("Measuring latency");
    let latencies = cf.meansure_latency(DEFAULT_LATENCY_ITERATIONS).await;
    report.latency.min = *latencies
        .iter()
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(&0.0);
    report.latency.max = *latencies
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(&0.0);
    report.latency.average = if !latencies.is_empty() {
        latencies.iter().sum::<f64>() / latencies.len() as f64
    } else {
        0.0
    };
    report.latency.median = median(&latencies);
    report.latency.jitter = jitter(&latencies);
    report.latency.p90 = quartile(&latencies, 0.9);
    report.latency.p75 = quartile(&latencies, 0.75);
    report.download_latencies.extend(latencies.clone());
    report.upload_latencies.extend(latencies);

    completed_tests += 1;

    let t0 = std::time::Instant::now();
    for test in &tests {
        match test.direction {
            Direction::Download => {
                pb.set_message(format!("Measuring download {}", test.raw_size));
                let result = cf.meansure_download(test.size, test.iterations).await;
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
                pb.set_message(format!("Measuring upload {}", test.raw_size));
                let result = cf.meansure_upload(test.size, test.iterations).await;
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
    }
    pb.finish_and_clear();

    eprintln!(
        "Completed {}/{} tests in {:.2?}\n",
        completed_tests,
        tests.len() + 1, // +1 for latency test
        t0.elapsed()
    );

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
    tw.write_all(b"Latency details\n")?;
    tw.write_fmt(format_args!(
        "\tMin:\t{:.2}\tms\n\tMax:\t{:.2}\tms\n\tAverage:\t{:.2}\tms\n\tMedian:\t{:.2}\tms\n\tJitter:\t{:.2}\tms\n\t90th Percentile:\t{:.2}\tms\n\t75th Percentile:\t{:.2}\tms\n\n",
        report.latency.min,
        report.latency.max,
        report.latency.average,
        report.latency.median,
        report.latency.jitter,
        report.latency.p90,
        report.latency.p75,
    ))?;
    tw.flush()?;

    tw.write_all(b"Download details:\n")?;
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
        "Download Latency (Median):\t{:.2}\tms\n",
        latency_median
    ))?;
    tw.write_fmt(format_args!(
        "Download Jitter:\t{:.2}\tms\n",
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

    tw.write_all(b"\nUpload details:\n")?;
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
        "Upload Latency (Median):\t{:.2}\tms\n",
        report.upload_median_latency
    ))?;
    tw.write_fmt(format_args!(
        "Upload Jitter:\t{:.2}\tms\n",
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
