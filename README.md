# Spidy

A fast, customizable CLI tool for testing your internet connection speed using Cloudflare's speed test infrastructure.

## Features

- **Download & Upload Speed Testing** - Measure both download and upload speeds with configurable test sizes
- **Latency & Jitter Metrics** - Track network latency and jitter for both directions
- **Statistical Analysis** - Automatic calculation of percentiles (75th, 90th), median latency, and jitter
- **Customizable Tests** - Define custom test configurations with specific sizes and iterations
- **Multiple Output Formats** - Results in human-readable text or JSON format
- **Location & ISP Info** - Displays server location, ASN, and your IP information
- **Progress Tracking** - Real-time progress bar showing test completion

## Installation

### From Source

```bash
git clone https://github.com/mipsel64/spidy.git
cd spidt
cargo build --release
./target/release/spidy --hlep
```

### From [crates.io](https://crates.io/crates/spidy)

```bash
cargo install --locked spidy
spidy --help
```

## Usage

### Basic Usage

Run with default test configuration:

```bash
spidy
```

### Custom Tests

Define custom tests using the `--test` or `-t` flag:

```bash
spidy -t "d=100kB=10,u=1MB=5,d=10MB=3"
```

Test format: `<direction>=<size>=<iterations>`

- **Direction**: `d`/`down` for download, `u`/`up` for upload
- **Size**: Human-readable format (e.g., `100kB`, `1MB`, `10MB`, `100MB`)
- **Iterations**: Number of times to run each test

### Output Formats

**Text output (default):**
```bash
spidy
```

**JSON output:**
```bash
spidy --format json
```

### Examples

```bash
# Quick test with smaller files
spidy -t "d=100kB=5,u=100kB=5"

# Heavy download test
spidy -t "d=25MB=10,d=50MB=5"

# Upload-focused test
spidy -t "u=1MB=10,u=10MB=8,u=25MB=5"

# Get JSON output for parsing
spidy -f json > results.json
```

## Default Test Configuration

If no tests are specified, the following default configuration is used:

- Download 100kB × 10 iterations
- Download 1MB × 8 iterations
- Upload 100kB × 8 iterations
- Upload 1MB × 6 iterations
- Download 10MB × 6 iterations
- Upload 10MB × 4 iterations
- Download 25MB × 4 iterations
- Download 100MB x 3 iterations

## Output

### Text Format

```
CLOUDFLARE SPEED TEST CLI
=========================

Server Location:    San Francisco - US
ASN:                13335 (Cloudflare, Inc.)
Your IP:            xxx.xxx.xxx.xxx

Completed 8/8 tests in 42.46s

Latency details
    Min:              116.00  ms
    Max:              188.00  ms
    Average:          138.15  ms
    Median:           131.00  ms
    Jitter:           22.05   ms
    90th Percentile:  170.20  ms
    75th Percentile:  143.50  ms

Download details:
    (10/10)  100kB  233.33  Mbps
    (8/8)    1MB    59.26   Mbps
    (6/6)    10MB   164.67  Mbps
    (4/4)    25MB   194.37  Mbps
Download Latency (Median):   131.50 ms
Download Jitter:             26.36  ms
Overall Download:
    90th Percentile:  306.67  Mbps
    75th Percentile:  200.15  Mbps

Upload details:
    (8/8)  100kB  2.74   Mbps
    (6/6)  1MB    18.32  Mbps
    (4/4)  10MB   38.60  Mbps
Upload Latency (Median): 145.00 ms
Upload Jitter:           306.38 ms
Overall Upload:
    90th Percentile:  38.18  Mbps
    75th Percentile:  24.39  Mbps
```

### JSON Format

The JSON output includes:
- Individual test results with all measurements
- Aggregated statistics (percentiles, median latency, jitter)
- Server metadata (location, ASN, IP information)
- Latency stats

## How It Works

Spidy uses Cloudflare's speed test infrastructure to measure your internet connection:

1. **Metadata Request** - Retrieves information about the nearest Cloudflare server and your connection
2. **Download Tests** - Downloads data from `https://speed.cloudflare.com/__down?bytes=<size>`
3. **Upload Tests** - Uploads data to `https://speed.cloudflare.com/__up`
4. **Metrics Calculation**:
   - Speed: Calculated from transfer size and transfer time (excluding TTFB)
   - Latency: Time to first byte minus server processing time
   - Jitter: Average absolute difference between consecutive latency measurements
   - Percentiles: Statistical distribution of all speed measurements

## Proxy Support

Spidy does not have proxy-related command line flags by design. The tool uses libcurl under the hood, which automatically respects standard proxy environment variables:

- `HTTP_PROXY` / `http_proxy`
- `HTTPS_PROXY` / `https_proxy`

To use a proxy, simply set the appropriate environment variable:

```bash
export HTTPS_PROXY=http://proxy.example.com:8080
spidy
```

This approach keeps the CLI simple and allows users to run `spidy` without specifying any flags.

## License

See [LICENSE](LICENSE) for details.
