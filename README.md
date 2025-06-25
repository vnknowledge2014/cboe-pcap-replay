# CBOE PCAP Replayer

Công cụ hiệu năng cao để phát lại dữ liệu UDP Market Order từ file PCAP của CBOE, đảm bảo thứ tự tuần tự chính xác trên mỗi port.

## Tính năng

- **Hiệu năng cực cao**: Tối ưu với thread pool, CPU affinity và lock-free data structures
- **Đảm bảo thứ tự tuần tự**: Duy trì thứ tự chính xác của các packet trên mỗi port
- **Buffer riêng cho từng port**: Ring buffer riêng biệt cho mỗi port đảm bảo thông lượng tối đa
- **Rate limiting**: Điều chỉnh tốc độ gửi packet tự động hoặc thủ công
- **Port mapping**: Khả năng ánh xạ các port đích sang port mới
- **Hỗ trợ multicast**: Tùy chọn gửi đến địa chỉ multicast
- **Chế độ loop**: Phát lại liên tục file PCAP
- **Phân tích PCAP**: Tự động phát hiện các port, tốc độ và thông tin khác

## Kiến trúc

- **Per-port ring buffer**: Mỗi port có một hàng đợi riêng để đảm bảo thứ tự tuần tự tuyệt đối trên từng port
- **Thread pool tối ưu**: Tự động điều chỉnh số lượng thread dựa trên số port phát hiện được
- **CPU affinity** (chỉ trên Linux): Gắn mỗi worker thread vào CPU core cụ thể để tăng hiệu năng
- **Cấu trúc dữ liệu lock-free**: Sử dụng crossbeam và dashmap để giảm thiểu tranh chấp giữa các thread

## Cài đặt

### Yêu cầu

- Rust 2024 Edition
- libpcap-dev (hoặc tương đương trên hệ điều hành của bạn)
- Core affinity API (Linux: hwloc, Windows: WinAPI, macOS: thread_policy_set)

```bash
# Cài đặt libpcap
sudo apt-get install libpcap-dev    # Ubuntu/Debian
sudo yum install libpcap-devel      # CentOS/RHEL
brew install libpcap                # macOS

# Clone repository
git clone https://github.com/your-username/cboe-pcap-replay.git
cd cboe-pcap-replay

# Biên dịch
cargo build --release
```

Sản phẩm biên dịch sẽ nằm trong thư mục `target/release/cboe-pcap-replay`.

### Sử dụng với Cargo

Nếu bạn đang phát triển hoặc muốn chạy trực tiếp từ mã nguồn:

```bash
# Chạy trong chế độ debug
cargo run -- -f market_data.pcap -t 192.168.1.100

# Chạy trong chế độ release (hiệu năng cao hơn)
cargo run --release -- -f market_data.pcap -t 127.0.0.1
```

### Sử dụng

```
cboe-sequential-replayer [OPTIONS] --file <FILE> --target <TARGET>
```

### Tham số bắt buộc

- `-f, --file <FILE>`: Đường dẫn tới file PCAP
- `-t, --target <TARGET>`: Địa chỉ IP đích

### Tham số tùy chọn

- `-r, --rate <RATE>`: Tốc độ giới hạn theo số packet mỗi giây (không chỉ định: phát lại theo đúng timing trong PCAP gốc)
- `--port-map <PORT_MAP>`: Ánh xạ port "old1:new1,old2:new2" (tùy chọn)
- `-m, --multicast`: Bật chế độ multicast UDP
- `--interface <INTERFACE>`: Interface cho multicast (tùy chọn)
- `--loop`: Lặp lại liên tục
- `-n, --threads <THREADS>`: Số lượng worker threads (mặc định: tự động dựa trên số port phát hiện được)
- `-v, --verbose`: Bật ghi log chi tiết

### Ví dụ

#### Chạy với nhiều worker threads để tối đa hiệu năng:

```bash
cargo run --release -- -f market_data.pcap -t 127.0.0.1 -n 8
```

#### Ánh xạ port:

```bash
./cboe-pcap-replay -f market_data.pcap -t 192.168.1.100 --port-map "7123:8123,7124:8124"
```

#### Multicast với interface cụ thể:

```bash
./cboe-pcap-replay -f market_data.pcap -t 239.1.1.1 -m --interface 192.168.1.5
```

#### Lặp lại liên tục với log chi tiết:

```bash
./cboe-pcap-replay -f market_data.pcap -t 192.168.1.100 --loop -v
```

## Quy trình hoạt động

1. **Phân tích file PCAP**: Công cụ đầu tiên quét file để phát hiện các port UDP duy nhất và tính toán tốc độ.
2. **Phân phối port tối ưu**: Các port được phân phối đều cho các worker thread để cân bằng tải.
3. **CPU affinity**: Mỗi worker thread được gắn với một CPU core cụ thể để tối ưu hiệu năng.
4. **Ring buffer cho mỗi port**: Mỗi port có một ring buffer riêng, đảm bảo tính tuần tự của packet trên từng port.
5. **Xử lý song song**: Các port khác nhau được xử lý song song bởi nhiều thread, trong khi vẫn duy trì thứ tự trên mỗi port.
6. **Thống kê**: Các số liệu hiệu suất được báo cáo mỗi 5 giây.

## Theo dõi hiệu suất

Trong quá trình chạy, công cụ báo cáo các số liệu thống kê mỗi 5 giây:
- Số packet đã gửi
- Số byte đã gửi
- Số lỗi
- Tốc độ hiện tại (packets/second)
- Kích thước hàng đợi tổng và trên từng port (nếu có tắc nghẽn)

## Gỡ lỗi

Để xem thông tin chi tiết hơn về quá trình:

```bash
./cboe-pcap-replay -f market_data.pcap -t 192.168.1.100 -v
```

## Lưu ý hệ điều hành

- **Linux**: Hỗ trợ đầy đủ, bao gồm CPU affinity
- **Windows**: Hỗ trợ đầy đủ, bao gồm CPU affinity
- **macOS**: Hỗ trợ đầy đủ ngoại trừ CPU affinity (do hạn chế của hệ điều hành)

## Xử lý trường hợp đặc biệt

### Ánh xạ port

Khi cần ánh xạ port từ file PCAP đến port khác trên máy đích, sử dụng tham số `--port-map`:

```bash
./cboe-pcap-replay -f market_data.pcap -t 192.168.1.100 --port-map "7123:8123,7124:8124,7125:8125"
```

Cú pháp: `"port_cũ_1:port_mới_1,port_cũ_2:port_mới_2,..."`

### Hiệu suất tối ưu

Để đạt hiệu suất tối đa:

1. Sử dụng phiên bản release: `cargo build --release`
2. Chạy trên máy có nhiều CPU cores
3. Đảm bảo network buffer đủ lớn
4. Cân nhắc tăng giới hạn file descriptor nếu cần: `ulimit -n 65535`
5. Phát lại theo đúng timing PCAP để tái tạo chính xác các bản tin market data

## Giới hạn

- Chỉ xử lý các packet UDP trong IPv4
- CPU affinity không hoạt động trên macOS
- Hàng đợi packet có thể tiêu tốn nhiều bộ nhớ với file PCAP lớn