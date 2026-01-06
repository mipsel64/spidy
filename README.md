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
./target/release/spidy
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

## Output

### Text Format

```
Cloudflare Speed Test Client
=============================
Server Location:    San Francisco US
ASN:                13335 (Cloudflare, Inc.)
Your IP:            xxx.xxx.xxx.xxx

Download Results:
    (10/10)    100kB    125.34    Mbps
    (8/8)      1MB      142.67    Mbps
    (6/6)      10MB     158.92    Mbps
Download Latency (Median):     12.45 ms
Download Jitter:               2.34 ms
Overall Download:
    90th Percentile:    156.78    Mbps
    75th Percentile:    148.23    Mbps

Upload Results:
    (8/8)      100kB    89.45     Mbps
    (6/6)      1MB      95.23     Mbps
    (4/4)      10MB     102.34    Mbps
Upload Latency (Median): 15.67 ms
Upload Jitter: 3.12 ms
Overall Upload:
    90th Percentile:    98.56     Mbps
    75th Percentile:    92.34     Mbps
```

### JSON Format

The JSON output includes:
- Individual test results with all measurements
- Aggregated statistics (percentiles, median latency, jitter)
- Server metadata (location, ASN, IP information)

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

## Dependencies

- **tokio** - Async runtime
- **curl** - HTTP requests with detailed timing information
- **clap** - Command-line argument parsing
- **serde/serde_json** - Serialization and JSON output
- **indicatif** - Progress bars
- **tabwriter** - Formatted text output
- **parse-size** - Human-readable size parsing
- **eyre** - Error handling

## License

[Add your license here]

## Contributing

[Add contribution guidelines here]
