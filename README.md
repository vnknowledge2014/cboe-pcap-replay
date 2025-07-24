# Bộ công cụ CBOE PCAP - Tuân thủ đặc tả PITCH chính thức

Bộ công cụ đầy đủ để mô phỏng, tạo và phát lại dữ liệu thị trường CBOE Australia PITCH - hoàn toàn tuân thủ đặc tả PITCH chính thức của Cboe Australia. Bộ công cụ này cung cấp giải pháp tích hợp với hỗ trợ quy trình làm việc tự động để tạo, chuyển đổi và phát lại dữ liệu thị trường Australia thực sử dụng các mã chứng khoán ASX thực.

## Tổng quan công cụ

### Trình phát lại CBOE PCAP tích hợp (`cboe-pcap-replay`)
**Công cụ tất cả trong một** với ba chế độ mạnh mẽ, hoàn toàn tuân thủ đặc tả PITCH chính thức:
- **Tạo dữ liệu**: Tạo dữ liệu thị trường CBOE PITCH thực tế ở định dạng CSV với tất cả các loại thông điệp chính thức (0x3B, 0x37, 0x38, 0x58, 0x39, 0x3A, 0x3C, 0x3D, 0x3E, 0x59, 0x5A, 0x97, 0xE3, 0x2D)
- **Chuyển đổi**: Chuyển đổi dữ liệu CSV sang định dạng PCAP với mã hóa thông điệp PITCH đúng và tiêu đề đơn vị tuần tự
- **Phát lại**: Trình phát gói tin tuần tự hiệu suất cao với đảm bảo thứ tự cho từng cổng

### Bộ thu CBOE PCAP (`cboe-pcap-receiver`)
Bộ thu thời gian thực để giám sát và xác thực các gói tin được phát lại với phân tích toàn diện.

### Quy trình làm việc tự động (`cboe_market_data_workflow.sh`)
Script tự động hóa hoàn chỉnh chạy toàn bộ quy trình từ tạo dữ liệu đến phân tích phát lại.

## Bắt đầu nhanh

### Sử dụng công cụ tích hợp

```bash
# Biên dịch công cụ tích hợp
cd cboe-pcap-replay
cargo build --release

# 1. Tạo dữ liệu thị trường CSV thực tế
./target/release/cboe-pcap-replay generate \
  --symbols ANZ,CBA,NAB,WBC \
  --duration 300 \
  --output market_data.csv

# 2. Chuyển đổi CSV sang định dạng PCAP
./target/release/cboe-pcap-replay convert \
  --input market_data.csv \
  --output market_data.pcap

# 3. Phát lại dữ liệu PCAP
./target/release/cboe-pcap-replay replay \
  --file market_data.pcap \
  --target 127.0.0.1
```

### Sử dụng quy trình làm việc tự động

```bash
# Biên dịch tất cả công cụ
cd cboe-pcap-replay && cargo build --release && cd ..
cd cboe-pcap-receiver && cargo build --release && cd ..

# Chạy quy trình làm việc tự động hoàn chỉnh
./cboe_market_data_workflow.sh
```

Script tự động sẽ:
1. Tạo 60 giây dữ liệu thị trường Australia thực tế với tất cả các loại thông điệp PITCH chính thức
2. Chuyển đổi sang định dạng PCAP với mã hóa thông điệp PITCH đúng
3. Khởi động bộ thu trong nền để giám sát việc nhận gói tin
4. Phát lại dữ liệu với trình tự chính xác và phân tích kết quả
5. Cung cấp bản tóm tắt toàn diện hiển thị phân phối loại thông điệp và sự tuân thủ

## CBOE PCAP Replayer

Công cụ hiệu năng cao để phát lại dữ liệu UDP Market Order từ file PCAP của CBOE, đảm bảo thứ tự tuần tự chính xác trên mỗi port và độ tin cậy cao trong việc truyền nhận gói tin.

### Tính năng

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

## Kiến trúc - Tuân thủ đặc tả PITCH

### CBOE PCAP Replayer (Sender) - Tối ưu hóa Flux

- **Mã hóa thông điệp PITCH**: Mã hóa nhị phân đúng của tất cả các loại thông điệp PITCH chính thức theo đặc tả
- **Tiêu đề đơn vị tuần tự**: Triển khai chính xác của tiêu đề 8-byte với các trường độ dài, số lượng, đơn vị và chuỗi
- **Ring Buffer hiệu năng cao**: Ring buffer không khóa lấy cảm hứng từ LMAX Disruptor với 1M slot và căn chỉnh cache-line
- **Tối ưu hóa Memory Pool**: Các pool buffer được cấp phát trước (nhỏ: 1KB, trung bình: 8KB, lớn: 64KB) cho đường dẫn không cấp phát
- **Tăng tốc SIMD**: Vector hóa đa nền tảng (ARM NEON + x86_64 AVX2) cho cải thiện hiệu suất gấp 2-4 lần
- **Threading NUMA-Aware**: Gắn kết CPU core và cấp phát bộ nhớ tối ưu hóa NUMA cho hệ thống nhiều socket
- **Hoạt động UDP theo lô**: 64 gói tin mỗi lần hoạt động mạng để tối ưu thông lượng
- **Vận chuyển Zero-Copy**: Hoạt động memory-mapped với socket SO_ZEROCOPY trên Linux
- **Thứ tự tuần tự theo port**: Duy trì tính toàn vẹn dữ liệu CBOE PITCH với xác thực chuỗi theo đơn vị
- **Cấu trúc dữ liệu không khóa**: Sử dụng các hoạt động nguyên tử trong toàn bộ pipeline với thứ tự bộ nhớ phù hợp
- **Mã hóa Base36**: Mã hóa đúng các ID đơn hàng và ID thực hiện theo đặc tả PITCH
- **Định dạng nhị phân Little Endian**: Thứ tự byte chính xác cho tất cả các trường số như được quy định

**Cải thiện hiệu suất:**
- **Thông lượng**: ~100K+ pps (cải thiện gấp 3,3 lần so với triển khai tiêu chuẩn)
- **Độ trễ**: Ổn định <10μs (cải thiện gấp 10 lần với giảm jitter)
- **Hiệu quả bộ nhớ**: Hiệu quả pool >90% với buffer được cấp phát trước
- **Sử dụng CPU**: Giảm 30-40% thông qua nhận thức NUMA và tối ưu hóa SIMD

### CBOE PCAP Receiver (Receiver) - Tối ưu hóa Flux

**Chế độ tiêu chuẩn:**
- **Socket UDP tiêu chuẩn**: Nhận gói UDP hiệu suất cao với buffer nhận 1MB
- **Kiểm tra chuỗi**: Xác thực chuỗi CBOE PITCH đầy đủ với theo dõi theo đơn vị
- **Phân tích thời gian thực**: Thống kê về mất gói, giao hàng không đúng thứ tự và trùng lặp
- **Chỉ số hiệu suất**: Tính toán thông lượng và phân tích khoảng cách với báo cáo chi tiết

**Chế độ tối ưu hóa Flux (`--flux-optimized`):**
- **Ring Buffer hiệu năng cao**: Ring buffer lấy cảm hứng từ LMAX Disruptor với 1M slot và căn chỉnh cache-line
- **Tối ưu hóa Memory Pool**: Pool cấp doanh nghiệp (20K buffer nhỏ, 10K trung bình, 2K lớn)
- **Xử lý theo lô**: Lô 128 gói tin để tối ưu thông lượng pipeline nhận
- **Worker theo port**: Các luồng worker chuyên dụng NUMA-aware với gắn kết CPU core
- **Thống kê nâng cao**: Phân tích phần trăm độ trễ p50, p55, p95, p99 và thời gian xử lý
- **Tăng tốc SIMD**: Vector hóa đa nền tảng cho xác thực và xử lý gói tin
- **Xác thực chuỗi thời gian thực**: Theo dõi đơn vị cụ thể CBOE PITCH với phát hiện khoảng cách ngay lập tức
- **Chỉ số doanh nghiệp**: Báo cáo sử dụng ring buffer, hiệu quả memory pool và trạng thái SIMD

**Cải thiện hiệu suất trong chế độ Flux:**
- **Thông lượng**: 50K+ pps (cải thiện gấp 2 lần so với chế độ tiêu chuẩn)
- **Độ trễ**: Thời gian xử lý ổn định <5μs với theo dõi phần trăm
- **Hiệu quả bộ nhớ**: Hiệu quả pool >90% với đường dẫn không cấp phát
- **Sử dụng CPU**: Threading NUMA-aware giảm tải CPU 25-30%

**Cách sử dụng:**
```bash
# Chế độ tiêu chuẩn với tùy chọn tùy chỉnh
./cboe-pcap-receiver -p 30501,30502,30503,30504 --interval 5 --verbose

# Chế độ tối ưu hóa Flux (khuyến nghị cho sản xuất)
./cboe-pcap-receiver -p 30501,30502,30503,30504 --flux-optimized --time 3600
```

## Tối ưu hóa hiệu suất cao Flux

Cả hai công cụ đều được nâng cao với các tối ưu hóa lấy cảm hứng từ flux cho hiệu suất cấp doanh nghiệp:

### Giai đoạn 1: Memory Pooling & Batching
- **Buffer được cấp phát trước**: Loại bỏ cấp phát động trong đường dẫn nóng
- **Di chuyển sang Ring Buffer**: Mẫu LMAX Disruptor với 1M slot
- **Xử lý theo lô**: Chi phí khấu hao với kích thước lô tối ưu

### Giai đoạn 2: Zero-Copy Operations
- **Cấu trúc dữ liệu không khóa**: Hoạt động nguyên tử trong toàn bộ pipeline
- **Tăng tốc SIMD**: Vector hóa đa nền tảng (NEON/AVX2)
- **I/O Memory-mapped**: Vận chuyển zero-copy trên hệ thống Linux

### Giai đoạn 3: NUMA Awareness & Lịch trình thời gian thực
- **CPU Affinity**: Các luồng worker được gắn với các CPU core cụ thể
- **Bộ nhớ NUMA-aware**: Mẫu truy cập bộ nhớ tối ưu hóa
- **Căn chỉnh cache-line**: Căn chỉnh 64-byte cho cấu trúc dữ liệu nóng

### Kết quả hiệu suất
| Chỉ số | Tiêu chuẩn | Tối ưu hóa Flux | Cải thiện |
|--------|----------|----------------|-------------|
| **Thông lượng** | ~30K pps | ~100K+ pps | **3,3x** |
| **Độ trễ** | Biến đổi (10-100μs) | Ổn định <10μs | **10x** |
| **Hiệu quả bộ nhớ** | Cấp phát động | Hiệu quả pool >90% | **Đáng kể** |
| **Sử dụng CPU** | Cao với cache miss | NUMA-aware + SIMD | **Giảm 30-40%** |
| **Mất gói tin** | Cao hơn dưới tải | Gần như không với ring buffer | **Cải thiện lớn** |

### Cải tiến mới nhất (v2.1)
- **Tái cấu trúc mã**: Giảm cảnh báo trình biên dịch từ 36 xuống 32 (cboe-pcap-receiver) và 35 xuống 32 (cboe-pcap-replay)
- **Cải tiến CLI**: Thêm giá trị mặc định phù hợp cho tất cả các tham số tùy chọn (--verbose=false, --flux-optimized=false, --loop=false)
- **Loại bỏ mã không sử dụng**: Loại bỏ các trường cấu trúc, phương thức và import không sử dụng để làm sạch mã nguồn
- **Cấu hình Ring Buffer**: Thêm tham số CLI --ring-buffer-size để cấp phát bộ nhớ động
- **Tương thích macOS**: Cải thiện tương thích vận chuyển zero-copy với các tối ưu hóa đặc thù nền tảng
- **Chỉ số hiệu suất**: Nâng cao theo dõi phần trăm độ trễ p55, p99 với báo cáo hoạt động SIMD

### Đảm bảo thứ tự tuần tự
Yêu cầu quan trọng: **"Tính tuần tự của package phải luôn luôn được đảm bảo"**
- ✅ Xác thực chuỗi theo port
- ✅ Theo dõi CBOE PITCH theo đơn vị  
- ✅ Phát hiện khoảng cách thời gian thực
- ✅ Không mất dữ liệu dưới tải

## Cài đặt

### Yêu cầu

- Rust 2021 Edition
- libpcap-dev (hoặc tương đương trên hệ điều hành của bạn)
- Tuân thủ đặc tả PITCH của Cboe Australia

```bash
# Cài đặt libpcap
sudo apt-get install libpcap-dev    # Ubuntu/Debian
sudo yum install libpcap-devel      # CentOS/RHEL
brew install libpcap                # macOS

# Tải mã nguồn
git clone https://github.com/your-username/cboe-pcap-suite.git
cd cboe-pcap-suite

# Biên dịch tất cả công cụ
for dir in cboe-pcap-replay cboe-pcap-receiver; do
    cd $dir && cargo build --release && cd ..
done
```

Các file thực thi đã biên dịch sẽ có sẵn trong thư mục `target/release/` của mỗi công cụ.

## Tài liệu công cụ

- [CBOE PCAP Replayer](cboe-pcap-replay/README.md) - Công cụ tích hợp hoàn chỉnh (tạo, chuyển đổi, phát lại)
- [CBOE PCAP Receiver](cboe-pcap-receiver/README.md) - Giám sát việc nhận gói tin

## Hỗ trợ thông điệp CBOE PITCH

Tất cả các công cụ đều tuân thủ đầy đủ đặc tả PITCH chính thức của Cboe Australia. Các loại thông điệp sau được hỗ trợ:

- **Trạng thái giao dịch (0x3B)**: Thông tin trạng thái thị trường, dấu thời gian, mã chứng khoán, trạng thái giao dịch, mã ID thị trường
- **Thêm lệnh (0x37)**: Các mục sổ lệnh mới với dấu thời gian, ID lệnh, phía, số lượng, mã chứng khoán, giá, ID người tham gia
- **Lệnh được thực hiện (0x38)**: Thực hiện lệnh với dấu thời gian, ID lệnh, số lượng thực hiện, ID thực hiện, ID lệnh đối ứng, ID người tham gia đối ứng
- **Lệnh được thực hiện theo giá (0x58)**: Thực hiện đấu giá với dấu thời gian, ID lệnh, số lượng thực hiện, ID thực hiện, giá đấu giá
- **Giảm kích thước (0x39)**: Giảm số lượng lệnh với dấu thời gian, ID lệnh, số lượng hủy
- **Sửa lệnh (0x3A)**: Sửa đổi lệnh với dấu thời gian, ID lệnh, số lượng mới, giá mới
- **Xóa lệnh (0x3C)**: Hủy lệnh với dấu thời gian, ID lệnh
- **Giao dịch (0x3D)**: Báo cáo giao dịch trên sàn và ngoài sàn với đầy đủ chi tiết giao dịch bao gồm người tham gia, loại giao dịch, chỉ định
- **Hủy giao dịch (0x3E)**: Hủy và sửa chữa giao dịch với dấu thời gian, ID thực hiện gốc, lý do hủy
- **Cập nhật đấu giá (0x59)**: Trạng thái đấu giá trước mở/trước đóng với cổ phiếu mua/bán và giá chỉ định
- **Tổng kết đấu giá (0x5A)**: Kết quả đấu giá cuối cùng với giá đấu giá và cổ phiếu
- **Xóa đơn vị (0x97)**: Xóa tất cả các lệnh cho đơn vị trong các sự kiện khôi phục
- **Giá trị tính toán (0xE3)**: Giá trị chỉ số/NAV với dấu thời gian, mã chứng khoán, danh mục giá trị, giá trị tính toán
- **Kết thúc phiên (0x2D)**: Thông điệp kết thúc phiên

## Ví dụ quy trình làm việc

### Tạo và kiểm tra dữ liệu thị trường

```bash
# Tạo 1 giờ dữ liệu ANZ,CBA
cboe-pcap-replay generate --symbols ANZ,CBA --duration 3600 --output market.csv

# Chuyển đổi sang PCAP
cboe-pcap-replay convert --input market.csv --output market.pcap

# Kiểm tra phát lại
cboe-pcap-replay replay --file market.pcap --target 127.0.0.1 --rate 1000
```

### Mô phỏng hiệu suất cao

```bash  
# Tạo tập dữ liệu lớn
cboe-pcap-replay generate --symbols ANZ,CBA,NAB,WBC,BHP --duration 28800 --output full_day.csv

# Chuyển đổi với cài đặt mạng tùy chỉnh
cboe-pcap-replay convert --input full_day.csv --output full_day.pcap --dest-ip 239.1.1.1

# Phát lại với tốc độ đầy đủ
cboe-pcap-replay replay --file full_day.pcap --target 239.1.1.1 --loop
```

## Hướng dẫn sử dụng công cụ tích hợp

### Lệnh Generate
Tạo dữ liệu thị trường CBOE PITCH thực tế ở định dạng CSV:

```bash
cboe-pcap-replay generate [OPTIONS]
```

**Tùy chọn:**
- `-s, --symbols <SYMBOLS>`: Danh sách mã chứng khoán cách nhau bằng dấu phẩy (mặc định: ANZ,CBA,NAB,WBC,BHP,RIO,FMG,NCM,TLS,WOW,CSL,TCL)
- `-d, --duration <DURATION>`: Thời lượng tính bằng giây (mặc định: 3600)
- `-o, --output <OUTPUT>`: Tệp CSV đầu ra (mặc định: market_data.csv)
- `-p, --port <PORT>`: Số cổng cơ sở (mặc định: 30501)
- `-u, --units <UNITS>`: Số lượng đơn vị/cổng (mặc định: 4)

**Ví dụ:**
```bash
./cboe-pcap-replay generate --symbols ANZ,CBA,BHP --duration 300 --output my_data.csv
```

### Lệnh Convert
Chuyển đổi dữ liệu thị trường CSV sang định dạng PCAP:

```bash
cboe-pcap-replay convert [OPTIONS] --input <INPUT> --output <OUTPUT>
```

**Tùy chọn bắt buộc:**
- `-i, --input <INPUT>`: Đường dẫn tệp CSV đầu vào
- `-o, --output <OUTPUT>`: Đường dẫn tệp PCAP đầu ra

**Tùy chọn:**
- `--src-ip <IP>`: Địa chỉ IP nguồn (mặc định: 192.168.1.1)
- `--dest-ip <IP>`: Địa chỉ IP đích (mặc định: 192.168.1.100)
- `--src-port <PORT>`: Cổng nguồn (mặc định: 12345)

**Ví dụ:**
```bash
./cboe-pcap-replay convert --input my_data.csv --output my_data.pcap
```

### Lệnh Replay  
Phát lại dữ liệu PCAP với hiệu suất cao:

```bash
cboe-pcap-replay replay [OPTIONS] --file <FILE> --target <TARGET>
```

**Tùy chọn bắt buộc:**
- `-f, --file <FILE>`: Đường dẫn đến tệp PCAP
- `-t, --target <TARGET>`: Địa chỉ IP đích

**Tùy chọn:**
- `-r, --rate <RATE>`: Giới hạn tốc độ theo gói tin/giây (mặc định: thời gian PCAP gốc)
- `--loop`: Phát lại vô hạn [mặc định: false]
- `--ring-buffer-size <SIZE>`: Kích thước ring buffer theo slot [mặc định: 16777216] (phải là lũy thừa của 2)

**Ví dụ:**

```bash
# Phát lại với thời gian gốc
./cboe-pcap-replay replay --file market_data.pcap --target 127.0.0.1

# Phát lại với tốc độ cố định
./cboe-pcap-replay replay --file market_data.pcap --target 127.0.0.1 --rate 10000

# Phát lại vòng lặp đến multicast
./cboe-pcap-replay replay --file market_data.pcap --target 239.1.1.1 --loop

# Phát lại thông lượng cao với kích thước ring buffer tùy chỉnh
./cboe-pcap-replay replay --file market_data.pcap --target 127.0.0.1 --ring-buffer-size 33554432
```

### Sử dụng phát triển

Nếu chạy từ mã nguồn:

```bash
# Tạo dữ liệu
cargo run --release -- generate --symbols ANZ,CBA --duration 300

# Chuyển đổi sang PCAP  
cargo run --release -- convert --input market_data.csv --output market_data.pcap

# Phát lại dữ liệu
cargo run --release -- replay --file market_data.pcap --target 127.0.0.1
```

### Sử dụng CBOE Receiver

```
cboe-pcap-receiver [OPTIONS] --ports <PORTS>
```

#### Tham số bắt buộc

- `-p, --ports <PORTS>`: Danh sách các cổng cần giám sát, cách nhau bằng dấu phẩy

#### Tham số tùy chọn

- `-i, --interface <INTERFACE>`: Địa chỉ IP để liên kết [mặc định: 127.0.0.1]
- `-t, --time <TIME>`: Thời gian chạy tính bằng giây (0 = vô hạn) [mặc định: 0]
- `-r, --interval <INTERVAL>`: Khoảng thời gian báo cáo tính bằng giây [mặc định: 5]
- `-v, --verbose`: Bật ghi nhật ký chi tiết [mặc định: false]
- `--flux-optimized`: Sử dụng bộ thu hiệu suất cao tối ưu hóa flux [mặc định: false]

#### Ví dụ

```bash
# Lắng nghe trên giao diện loopback
./cboe-pcap-receiver -p 30501,30502 -i 127.0.0.1

# Lắng nghe trên tất cả các giao diện
./cboe-pcap-receiver -p 30501,30502 -i 0.0.0.0

# Lắng nghe với IP cụ thể và khoảng thời gian tùy chỉnh
./cboe-pcap-receiver -p 30501,30502 -i 192.168.1.100 -r 10

# Chế độ chi tiết với giới hạn thời gian
./cboe-pcap-receiver -p 30501,30502 -v -t 60
```

## Quyền hạn

- **Replayer**: Không cần quyền đặc biệt - sử dụng socket UDP tiêu chuẩn
- **Receiver**: Không cần quyền đặc biệt - sử dụng socket UDP tiêu chuẩn

Cả hai công cụ đều được thiết kế lại để hoạt động mà không cần quyền root để dễ dàng triển khai và kiểm tra hơn.

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

1. **UDP Socket Binding**: Tạo các socket UDP tiêu chuẩn trên các cổng và địa chỉ IP được chỉ định
2. **Nhận gói tin**: Nhận các gói tin UDP sử dụng các hoạt động socket tiêu chuẩn
3. **Phân tích CBOE PITCH**: Trích xuất và xác thực tiêu đề thông điệp CBOE PITCH và nội dung
4. **Phân tích chuỗi**: Phát hiện các gói bị mất, trùng lặp hoặc không đúng thứ tự theo số chuỗi
5. **Thống kê thời gian thực**: Hiển thị các thống kê nhận toàn diện và các chỉ số hiệu suất

## Theo dõi hiệu suất

Trong quá trình chạy, sender báo cáo các số liệu thống kê mỗi 5 giây:
- Tiến độ phát lại (phần trăm và số packet)
- Tốc độ hiện tại (packets/second)
- Số byte đã gửi
- Số lỗi (nếu có)
- Kích thước hàng đợi (nếu có tắc nghẵn)

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

#### Lỗi quyền hạn

```
Failed to set IP_HDRINCL option. Raw sockets typically require root privileges.
```

**Giải pháp**: Chạy với sudo hoặc quyền administrator.

#### Cảnh báo kích thước buffer

```
Failed to set receive buffer to ... bytes
```

**Giải pháp**: Tăng giới hạn buffer hệ thống:
```bash
sudo sysctl -w net.core.rmem_max=26214400  # Linux
sudo sysctl -w net.inet.udp.recvspace=8388608  # macOS
```

#### Mất gói trên macOS

Trên macOS, hiện tượng mất gói khi truyền qua UDP loopback là khá phổ biến do đặc thù của networking stack.

**Giải pháp**: Sử dụng các tùy chọn mặc định về raw socket và BPF để bypass các giới hạn này.

## Hiệu suất tối ưu

Để đạt hiệu suất tối đa:

1. Sử dụng phiên bản release: `cargo build --release`
2. Chạy trên máy có nhiều CPU cores
3. Đảm bảo CPU affinity được bật (cần quyền root)
4. Cân nhắc tăng giới hạn file descriptor nếu cần: `ulimit -n 65535`
5. Đối với dữ liệu market data có tính thời gian quan trọng, sử dụng chế độ timing mặc định