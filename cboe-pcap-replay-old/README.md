# CBOE PCAP Replayer

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

## Kiến trúc

### CBOE PCAP Replayer (Sender)

- **Raw Socket**: Xây dựng và gửi gói tin IP+UDP trực tiếp, bypass các giới hạn của UDP socket thông thường
- **Per-port ring buffer**: Mỗi port có một hàng đợi riêng để đảm bảo thứ tự tuần tự tuyệt đối trên từng port
- **Thread pool tối ưu**: Tự động điều chỉnh số lượng thread dựa trên số port phát hiện được
- **CPU affinity đa nền tảng**: Phát hiện và gắn thread với CPU core trên các hệ điều hành hỗ trợ
- **Cấu trúc dữ liệu lock-free**: Sử dụng crossbeam và dashmap để giảm thiểu tranh chấp giữa các thread
- **Báo cáo thông minh**: Hiển thị tiến trình, tốc độ và phân tích hiệu suất trong thời gian thực

### CBOE PCAP Receiver (Receiver)

- **Berkeley Packet Filter (BPF)**: Bắt gói tin ở tầng cực thấp, trước khi vào hệ thống socket thông thường
- **Sequence checking**: Kiểm tra tính liên tục và thứ tự của chuỗi gói tin
- **Phân tích hiệu suất**: Thống kê về số lượng gói bị mất, thứ tự sai, trùng lặp
- **Đo độ trễ**: Tính toán khoảng cách thời gian giữa các gói tin

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

Sản phẩm biên dịch sẽ nằm trong thư mục `target/release/cboe-pcap-replay` và `target/release/cboe-pcap-receiver`.

### Sử dụng với Cargo

Nếu bạn đang phát triển hoặc muốn chạy trực tiếp từ mã nguồn:

```bash
# Chạy sender trong chế độ release (hiệu năng cao hơn)
cargo run --release --bin cboe-pcap-replay -- -f market_data.pcap -t 127.0.0.1

# Chạy receiver để kiểm tra packet reception
cargo run --release --bin cboe-pcap-receiver -- -p 30501,30502 -i lo0
```

### Sử dụng Sender (Replayer)

```
cboe-pcap-replay [OPTIONS] --file <FILE> --target <TARGET>
```

#### Tham số bắt buộc

- `-f, --file <FILE>`: Đường dẫn tới file PCAP
- `-t, --target <TARGET>`: Địa chỉ IP đích (hỗ trợ multicast tự động)

#### Tham số tùy chọn

- `-r, --rate <RATE>`: Tốc độ giới hạn theo số packet mỗi giây (không chỉ định: phát lại theo đúng timing trong PCAP gốc)
- `--loop`: Lặp lại liên tục

#### Ví dụ

##### Phát lại với timing gốc:

```bash
sudo ./cboe-pcap-replay -f market_data.pcap -t 192.168.1.100
```

##### Phát lại với tốc độ cố định:

```bash
sudo ./cboe-pcap-replay -f market_data.pcap -t 192.168.1.100 -r 10000
```

##### Phát lại đến địa chỉ multicast:

```bash
sudo ./cboe-pcap-replay -f market_data.pcap -t 239.1.1.1
```

##### Lặp lại liên tục:

```bash
sudo ./cboe-pcap-replay -f market_data.pcap -t 192.168.1.100 --loop
```

### Sử dụng Receiver

```
cboe-pcap-receiver [OPTIONS] --ports <PORTS> --interface <INTERFACE>
```

#### Tham số bắt buộc

- `-p, --ports <PORTS>`: Danh sách các port cần theo dõi (phân cách bằng dấu phẩy)
- `-i, --interface <INTERFACE>`: Interface mạng cần theo dõi (ví dụ: lo0, eth0)

#### Tham số tùy chọn

- `-t, --time <TIME>`: Thời gian chạy tính bằng giây (0 = vô hạn)
- `-i, --interval <INTERVAL>`: Khoảng thời gian báo cáo tính bằng giây

#### Ví dụ

```bash
sudo ./cboe-pcap-receiver -p 30501,30502 -i lo0
```

## Yêu cầu quyền root

Cả hai công cụ đều yêu cầu quyền root (sudo) để hoạt động đúng:
- **Sender (Replayer)**: Cần quyền root để sử dụng raw socket
- **Receiver**: Cần quyền root để truy cập BPF device

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

1. **Mở BPF device**: Thiết lập Berkeley Packet Filter để bắt gói tin ở tầng thấp.
2. **Lọc gói tin**: Thiết lập bộ lọc BPF để chỉ bắt các gói UDP đến các port cụ thể.
3. **Phân tích gói tin**: Trích xuất thông tin từ header và payload của gói tin.
4. **Kiểm tra sequence**: Phát hiện gói bị mất, trùng lặp hoặc thứ tự sai.
5. **Báo cáo định kỳ**: Hiển thị thống kê về các gói đã nhận, tỉ lệ mất gói, và hiệu suất.

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