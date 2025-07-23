# CBOE PCAP Suite - Official PITCH Specification Compliant

Complete suite of tools for CBOE Australia PITCH market data simulation, generation, and replay - fully compliant with the official Cboe Australia PITCH Specification. This suite provides an integrated solution with automated workflow support for generating, converting, and replaying authentic Australian market data using real ASX symbols.

## Tools Overview

### Integrated CBOE PCAP Replayer (`cboe-pcap-replay`)
**All-in-one tool** with three powerful modes, fully compliant with the official PITCH specification:
- **Generate**: Creates realistic CBOE PITCH market data in CSV format with all official message types (0x3B, 0x37, 0x38, 0x58, 0x39, 0x3A, 0x3C, 0x3D, 0x3E, 0x59, 0x5A, 0x97, 0xE3, 0x2D)
- **Convert**: Converts CSV data to PCAP format with proper PITCH message encoding and sequenced unit headers
- **Replay**: High-performance sequential packet replayer with per-port ordering guarantee

### CBOE PCAP Receiver (`cboe-pcap-receiver`)
Real-time receiver for monitoring and validating replayed packets with comprehensive analysis.

### Automated Workflow (`cboe_market_data_workflow.sh`)
Complete automation script that runs the entire pipeline from data generation to replay analysis.

## Quick Start

### Using the Integrated Tool

```bash
# Build the integrated tool
cd cboe-pcap-replay
cargo build --release

# 1. Generate realistic CSV market data
./target/release/cboe-pcap-replay generate \
  --symbols ANZ,CBA,NAB,WBC \
  --duration 300 \
  --output market_data.csv

# 2. Convert CSV to PCAP format
./target/release/cboe-pcap-replay convert \
  --input market_data.csv \
  --output market_data.pcap

# 3. Replay PCAP data
./target/release/cboe-pcap-replay replay \
  --file market_data.pcap \
  --target 127.0.0.1
```

### Using the Automated Workflow

```bash
# Build all tools
cd cboe-pcap-replay && cargo build --release && cd ..
cd cboe-pcap-receiver && cargo build --release && cd ..

# Run complete automated workflow
./cboe_market_data_workflow.sh
```

The automation script will:
1. Generate 60 seconds of realistic Australian market data with all official PITCH message types
2. Convert it to PCAP format with proper PITCH message encoding
3. Start the receiver in background to monitor packet reception
4. Replay the data with correct sequencing and analyze results
5. Provide comprehensive summary showing message type distribution and compliance

## CBOE PCAP Replayer

Công cụ hiệu năng cao để phát lại dữ liệu UDP Market Order từ file PCAP của CBOE, đảm bảo thứ tự tuần tự chính xác trên mỗi port và độ tin cậy cao trong việc truyền nhận gói tin.

## Tính năng

- **Độ tin cậy cao**: Sử dụng kỹ thuật raw socket và BPF để đảm bảo không mất gói tin, ngay cả trên macOS
- **Hiệu năng cực cao**: Tối ưu với thread pool, CPU affinity đa nền tảng và cấu trúc dữ liệu lock-free
- **Đảm bảo thứ tự tuần tự**: Duy trì thứ tự chính xác của các packet trên mỗi port
- **Buffer riêng cho từng port**: Ring buffer riêng biệt cho mỗi port đảm bảo thông lượng tối đa
- **Auto-scaling thread**: Tự động điều chỉnh số lượng thread dựa trên số port phát hiện được
- **Hỗ trợ multicast tích hợp**: Tự động cấu hình cho địa chỉ UDP multicast
- **Rate limiting**: Điều chỉnh tốc độ gửi packet tự động theo PCAP gốc hoặc tốc độ cố định
- **Chế độ loop**: Phát lại liên tục file PCAP
- **Thống kê chi tiết**: Báo cáo tiến trình, tốc độ và tổng kết khi hoàn thành
- **Tương thích đa nền tảng**: Hỗ trợ trên Linux, Windows và macOS với hiệu suất tối ưu

## Architecture - PITCH Specification Compliant

### CBOE PCAP Replayer (Sender)

- **PITCH Message Encoding**: Proper binary encoding of all official PITCH message types according to specification
- **Sequenced Unit Headers**: Correct implementation of 8-byte headers with length, count, unit, and sequence fields
- **Per-port ring buffer**: Each port has a dedicated queue to ensure absolute sequential ordering per port
- **Thread pool optimization**: Automatically adjusts thread count based on detected ports
- **CPU affinity multi-platform**: Detects and binds threads to CPU cores on supported operating systems
- **Lock-free data structures**: Uses crossbeam and dashmap to minimize thread contention
- **Intelligent reporting**: Real-time display of progress, rate, and performance analysis
- **Base36 Encoding**: Proper encoding of Order IDs and Execution IDs as per PITCH specification
- **Little Endian Binary Format**: Correct byte ordering for all numeric fields as specified

### CBOE PCAP Receiver (Receiver)

- **Berkeley Packet Filter (BPF)**: Bắt gói tin ở tầng cực thấp, trước khi vào hệ thống socket thông thường
- **Sequence checking**: Kiểm tra tính liên tục và thứ tự của chuỗi gói tin
- **Phân tích hiệu suất**: Thống kê về số lượng gói bị mất, thứ tự sai, trùng lặp
- **Đo độ trễ**: Tính toán khoảng cách thời gian giữa các gói tin

## Installation

### Requirements

- Rust 2021 Edition (corrected from invalid 2024 edition)
- libpcap-dev (or equivalent on your OS)
- Compliance with Cboe Australia PITCH Specification

```bash
# Install libpcap
sudo apt-get install libpcap-dev    # Ubuntu/Debian
sudo yum install libpcap-devel      # CentOS/RHEL
brew install libpcap                # macOS

# Clone repository
git clone https://github.com/your-username/cboe-pcap-suite.git
cd cboe-pcap-suite

# Build all tools
for dir in cboe-pcap-replay cboe-pcap-receiver; do
    cd $dir && cargo build --release && cd ..
done
```

Compiled binaries will be available in each tool's `target/release/` directory.

## Tool Documentation

- [CBOE PCAP Replayer](cboe-pcap-replay/README.md) - Complete integrated tool (generate, convert, replay)
- [CBOE PCAP Receiver](cboe-pcap-receiver/README.md) - Monitor packet reception

## CBOE PITCH Message Support

All tools now fully comply with the official Cboe Australia PITCH Specification. The following message types are supported:

- **Trading Status (0x3B)**: Market status information, timestamps, symbol, trading status, market ID code
- **Add Order (0x37)**: New order book entries with timestamps, order ID, side, quantity, symbol, price, participant ID
- **Order Executed (0x38)**: Order executions with timestamps, order ID, executed quantity, execution ID, contra order ID, contra participant ID
- **Order Executed at Price (0x58)**: Auction executions with timestamps, order ID, executed quantity, execution ID, auction price
- **Reduce Size (0x39)**: Order quantity reductions with timestamps, order ID, cancelled quantity
- **Modify Order (0x3A)**: Order modifications with timestamps, order ID, new quantity, new price
- **Delete Order (0x3C)**: Order cancellations with timestamps, order ID
- **Trade (0x3D)**: On-exchange and off-exchange trade reports with complete trade details including participants, trade type, designation
- **Trade Break (0x3E)**: Trade cancellations and corrections with timestamps, original execution ID, break reason
- **Auction Update (0x59)**: Pre-open/pre-close auction status with buy/sell shares and indicative price
- **Auction Summary (0x5A)**: Final auction results with auction price and shares
- **Unit Clear (0x97)**: Clear all orders for unit during recovery events
- **Calculated Value (0xE3)**: Index/NAV values with timestamps, symbol, value category, calculated value
- **End of Session (0x2D)**: Session termination messages

## Workflow Examples

### Generate and Test Market Data

```bash
# Generate 1 hour of ANZ,CBA data
cboe-pcap-replay generate --symbols ANZ,CBA --duration 3600 --output market.csv

# Convert to PCAP
cboe-pcap-replay convert --input market.csv --output market.pcap

# Test replay
cboe-pcap-replay replay --file market.pcap --target 127.0.0.1 --rate 1000
```

### High Performance Simulation

```bash  
# Generate large dataset
cboe-pcap-replay generate --symbols ANZ,CBA,NAB,WBC,BHP --duration 28800 --output full_day.csv

# Convert with custom network settings
cboe-pcap-replay convert --input full_day.csv --output full_day.pcap --dest-ip 239.1.1.1

# Replay at full speed
cboe-pcap-replay replay --file full_day.pcap --target 239.1.1.1 --loop
```

## Integrated Tool Usage

### Generate Command
Create realistic CBOE PITCH market data in CSV format:

```bash
cboe-pcap-replay generate [OPTIONS]
```

**Options:**
- `-s, --symbols <SYMBOLS>`: Comma-separated symbols (default: ANZ,CBA,NAB,WBC,BHP,RIO,FMG,NCM,TLS,WOW,CSL,TCL)
- `-d, --duration <DURATION>`: Duration in seconds (default: 3600)
- `-o, --output <OUTPUT>`: Output CSV file (default: market_data.csv)
- `-p, --port <PORT>`: Base port number (default: 30501)
- `-u, --units <UNITS>`: Number of units/ports (default: 4)

**Example:**
```bash
./cboe-pcap-replay generate --symbols ANZ,CBA,BHP --duration 300 --output my_data.csv
```

### Convert Command
Convert CSV market data to PCAP format:

```bash
cboe-pcap-replay convert [OPTIONS] --input <INPUT> --output <OUTPUT>
```

**Required Options:**
- `-i, --input <INPUT>`: Input CSV file path
- `-o, --output <OUTPUT>`: Output PCAP file path

**Optional:**
- `--src-ip <IP>`: Source IP address (default: 192.168.1.1)
- `--dest-ip <IP>`: Destination IP address (default: 192.168.1.100)
- `--src-port <PORT>`: Source port (default: 12345)

**Example:**
```bash
./cboe-pcap-replay convert --input my_data.csv --output my_data.pcap
```

### Replay Command  
Replay PCAP data with high performance:

```bash
cboe-pcap-replay replay [OPTIONS] --file <FILE> --target <TARGET>
```

**Required Options:**
- `-f, --file <FILE>`: Path to PCAP file
- `-t, --target <TARGET>`: Target IP address

**Optional:**
- `-r, --rate <RATE>`: Rate limit in packets/second (default: original PCAP timing)
- `--loop`: Loop replay indefinitely

**Examples:**

```bash
# Replay with original timing
./cboe-pcap-replay replay --file market_data.pcap --target 127.0.0.1

# Replay with fixed rate
./cboe-pcap-replay replay --file market_data.pcap --target 127.0.0.1 --rate 10000

# Loop replay to multicast
./cboe-pcap-replay replay --file market_data.pcap --target 239.1.1.1 --loop
```

### Development Usage

If running from source:

```bash
# Generate data
cargo run --release -- generate --symbols ANZ,CBA --duration 300

# Convert to PCAP  
cargo run --release -- convert --input market_data.csv --output market_data.pcap

# Replay data
cargo run --release -- replay --file market_data.pcap --target 127.0.0.1
```

### CBOE Receiver Usage

```
cboe-pcap-receiver [OPTIONS] --ports <PORTS>
```

#### Required Parameters

- `-p, --ports <PORTS>`: Comma-separated list of ports to monitor

#### Optional Parameters

- `-i, --interface <INTERFACE>`: IP address to bind to [default: 127.0.0.1]
- `-t, --time <TIME>`: Runtime in seconds (0 = infinite) [default: 0]
- `-r, --interval <INTERVAL>`: Report interval in seconds [default: 5]
- `-v, --verbose`: Enable verbose logging

#### Examples

```bash
# Listen on loopback interface
./cboe-pcap-receiver -p 30501,30502 -i 127.0.0.1

# Listen on all interfaces
./cboe-pcap-receiver -p 30501,30502 -i 0.0.0.0

# Listen with specific IP and custom interval
./cboe-pcap-receiver -p 30501,30502 -i 192.168.1.100 -r 10

# Verbose mode with time limit
./cboe-pcap-receiver -p 30501,30502 -v -t 60
```

## Permissions

- **Replayer**: No special permissions required - uses standard UDP sockets
- **Receiver**: No special permissions required - uses standard UDP sockets

Both tools have been redesigned to work without root privileges for easier deployment and testing.

## Quy trình hoạt động

### Sender (Replayer)

1. **Phân tích file PCAP**: Công cụ đầu tiên quét file để phát hiện các port UDP duy nhất và tính toán tốc độ.
2. **Tự động phân bổ thread**: Số lượng thread được xác định dựa trên số port phát hiện được và số CPU có sẵn.
3. **Phân phối port tối ưu**: Các port được phân phối đều cho các worker thread để cân bằng tải.
4. **CPU affinity đa nền tảng**: Mỗi worker thread được gắn với một CPU core nếu hệ điều hành hỗ trợ.
5. **Ring buffer cho mỗi port**: Mỗi port có một ring buffer riêng, đảm bảo tính tuần tự của packet trên từng port.
6. **Xây dựng và gửi packet**: Sử dụng raw socket để xây dựng và gửi packet IP+UDP, đảm bảo kiểm soát đầy đủ quá trình.
7. **Thống kê thời gian thực**: Các số liệu hiệu suất được báo cáo mỗi 5 giây với thông tin tiến trình.
8. **Báo cáo tổng kết**: Khi hoàn thành, công cụ hiển thị báo cáo chi tiết về hiệu suất.

### Receiver

1. **UDP Socket Binding**: Creates standard UDP sockets on specified ports and IP addresses
2. **Packet Reception**: Receives UDP packets using standard socket operations
3. **CBOE PITCH Parsing**: Extracts and validates CBOE PITCH message headers and payloads
4. **Sequence Analysis**: Detects missing, duplicate, or out-of-order packets by sequence number
5. **Real-time Statistics**: Displays comprehensive reception statistics and performance metrics

## Theo dõi hiệu suất

Trong quá trình chạy, sender báo cáo các số liệu thống kê mỗi 5 giây:
- Tiến độ phát lại (phần trăm và số packet)
- Tốc độ hiện tại (packets/second)
- Số byte đã gửi
- Số lỗi (nếu có)
- Kích thước hàng đợi (nếu có tắc nghẽn)

Khi kết thúc, sender hiển thị báo cáo tổng kết:
- Tổng thời gian chạy
- Số packet đã xử lý
- Tổng dung lượng đã gửi
- Tốc độ trung bình
- Số lần phát lại (nếu ở chế độ loop)

Receiver hiển thị các báo cáo định kỳ với thông tin:
- Số lượng gói nhận được trên mỗi port
- Tốc độ nhận (packets/second và MB/second)
- Khoảng cách thời gian lớn nhất giữa các gói
- Thông tin về sequence: phạm vi, tỷ lệ nhận đầy đủ, số gói mất/sai thứ tự/trùng lặp

## Lưu ý hệ điều hành

- **Linux**: Hỗ trợ đầy đủ, bao gồm CPU affinity và BPF
- **macOS**: Hỗ trợ đầy đủ với một số điều chỉnh đặc thù cho networking stack của Darwin
- **Windows**: Hỗ trợ cơ bản, nhưng một số tính năng như raw socket và BPF có thể bị giới hạn

Công cụ tự động phát hiện môi trường và điều chỉnh cấu hình phù hợp với từng hệ điều hành.

## Xử lý sự cố

### Vấn đề phổ biến

#### Permission errors

```
Failed to set IP_HDRINCL option. Raw sockets typically require root privileges.
```

**Giải pháp**: Chạy với sudo hoặc quyền administrator.

#### Buffer size warnings

```
Failed to set receive buffer to ... bytes
```

**Giải pháp**: Tăng giới hạn buffer hệ thống:
```bash
sudo sysctl -w net.core.rmem_max=26214400  # Linux
sudo sysctl -w net.inet.udp.recvspace=8388608  # macOS
```

#### Packet loss on macOS

Trên macOS, hiện tượng mất gói khi truyền qua UDP loopback là khá phổ biến do đặc thù của networking stack.

**Giải pháp**: Sử dụng các tùy chọn mặc định về raw socket và BPF để bypass các giới hạn này.

## Hiệu suất tối ưu

Để đạt hiệu suất tối đa:

1. Sử dụng phiên bản release: `cargo build --release`
2. Chạy trên máy có nhiều CPU cores
3. Đảm bảo CPU affinity được bật (cần quyền root)
4. Cân nhắc tăng giới hạn file descriptor nếu cần: `ulimit -n 65535`
5. Đối với dữ liệu market data có tính thời gian quan trọng, sử dụng chế độ timing mặc định