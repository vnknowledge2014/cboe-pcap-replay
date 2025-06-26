# CBOE PCAP Replayer

Công cụ hiệu năng cao để phát lại dữ liệu UDP Market Order từ file PCAP của CBOE, đảm bảo thứ tự tuần tự chính xác trên mỗi port.

## Tính năng

- **Hiệu năng cực cao**: Tối ưu với thread pool, CPU affinity đa nền tảng và cấu trúc dữ liệu lock-free
- **Đảm bảo thứ tự tuần tự**: Duy trì thứ tự chính xác của các packet trên mỗi port
- **Buffer riêng cho từng port**: Ring buffer riêng biệt cho mỗi port đảm bảo thông lượng tối đa
- **Auto-scaling thread**: Tự động điều chỉnh số lượng thread dựa trên số port phát hiện được
- **Hỗ trợ multicast tích hợp**: Tự động cấu hình cho địa chỉ UDP multicast
- **Rate limiting**: Điều chỉnh tốc độ gửi packet tự động theo PCAP gốc hoặc tốc độ cố định
- **Chế độ loop**: Phát lại liên tục file PCAP
- **Thống kê chi tiết**: Báo cáo tiến trình, tốc độ và tổng kết khi hoàn thành
- **Tương thích đa nền tảng**: Hỗ trợ CPU affinity trên các hệ điều hành khác nhau

## Kiến trúc

- **Per-port ring buffer**: Mỗi port có một hàng đợi riêng để đảm bảo thứ tự tuần tự tuyệt đối trên từng port
- **Thread pool tối ưu**: Tự động điều chỉnh số lượng thread dựa trên số port phát hiện được
- **CPU affinity đa nền tảng**: Phát hiện và gắn thread với CPU core trên các hệ điều hành hỗ trợ
- **Cấu trúc dữ liệu lock-free**: Sử dụng crossbeam và dashmap để giảm thiểu tranh chấp giữa các thread
- **Báo cáo thông minh**: Hiển thị tiến trình, tốc độ và phân tích hiệu suất trong thời gian thực

## Cài đặt

### Yêu cầu

- Rust 2024 Edition
- libpcap-dev (hoặc tương đương trên hệ điều hành của bạn)

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
- `-t, --target <TARGET>`: Địa chỉ IP đích (hỗ trợ multicast tự động)

### Tham số tùy chọn

- `-r, --rate <RATE>`: Tốc độ giới hạn theo số packet mỗi giây (không chỉ định: phát lại theo đúng timing trong PCAP gốc)
- `--loop`: Lặp lại liên tục

### Ví dụ

#### Phát lại với timing gốc:

```bash
./cboe-pcap-replay -f market_data.pcap -t 192.168.1.100
```

#### Phát lại với tốc độ cố định:

```bash
./cboe-pcap-replay -f market_data.pcap -t 192.168.1.100 -r 10000
```

#### Phát lại đến địa chỉ multicast:

```bash
./cboe-pcap-replay -f market_data.pcap -t 239.1.1.1
```

#### Lặp lại liên tục:

```bash
./cboe-pcap-replay -f market_data.pcap -t 192.168.1.100 --loop
```

## Quy trình hoạt động

1. **Phân tích file PCAP**: Công cụ đầu tiên quét file để phát hiện các port UDP duy nhất và tính toán tốc độ.
2. **Tự động phân bổ thread**: Số lượng thread được xác định dựa trên số port phát hiện được và số CPU có sẵn.
3. **Phân phối port tối ưu**: Các port được phân phối đều cho các worker thread để cân bằng tải.
4. **CPU affinity đa nền tảng**: Mỗi worker thread được gắn với một CPU core nếu hệ điều hành hỗ trợ.
5. **Ring buffer cho mỗi port**: Mỗi port có một ring buffer riêng, đảm bảo tính tuần tự của packet trên từng port.
6. **Xử lý song song**: Các port khác nhau được xử lý song song bởi nhiều thread, trong khi vẫn duy trì thứ tự trên mỗi port.
7. **Thống kê thời gian thực**: Các số liệu hiệu suất được báo cáo mỗi 5 giây với thông tin tiến trình.
8. **Báo cáo tổng kết**: Khi hoàn thành, công cụ hiển thị báo cáo chi tiết về hiệu suất.

## Theo dõi hiệu suất

Trong quá trình chạy, công cụ báo cáo các số liệu thống kê mỗi 5 giây:
- Tiến độ phát lại (phần trăm và số packet)
- Tốc độ hiện tại (packets/second)
- Số byte đã gửi
- Số lỗi (nếu có)
- Kích thước hàng đợi (nếu có tắc nghẽn)

Khi kết thúc, công cụ hiển thị báo cáo tổng kết:
- Tổng thời gian chạy
- Số packet đã xử lý
- Tổng dung lượng đã gửi
- Tốc độ trung bình
- Số lần phát lại (nếu ở chế độ loop)

## Lưu ý hệ điều hành

- **Linux**: Hỗ trợ đầy đủ, bao gồm CPU affinity
- **Windows**: Hỗ trợ đầy đủ, bao gồm CPU affinity (sử dụng core_affinity chuẩn)
- **macOS**: Hỗ trợ đầy đủ, CPU affinity tùy thuộc vào phiên bản macOS

Công cụ tự động phát hiện khả năng hỗ trợ CPU affinity và điều chỉnh phù hợp với từng hệ điều hành.

## Hiệu suất tối ưu

Để đạt hiệu suất tối đa:

1. Sử dụng phiên bản release: `cargo build --release`
2. Chạy trên máy có nhiều CPU cores
3. Đảm bảo network buffer đủ lớn
4. Cân nhắc tăng giới hạn file descriptor nếu cần: `ulimit -n 65535`
5. Đối với dữ liệu market data có tính thời gian quan trọng, sử dụng chế độ timing mặc định

## Giới hạn

- Chỉ xử lý các packet UDP trong IPv4
- CPU affinity phụ thuộc vào hỗ trợ của hệ điều hành
- Hàng đợi packet có thể tiêu tốn nhiều bộ nhớ với file PCAP lớn