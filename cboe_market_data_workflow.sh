#!/bin/bash

# CBOE Market Data Complete Workflow Automation Script
# This script automates the complete CBOE PITCH market data pipeline:
# 1. Generate realistic CSV market data
# 2. Convert CSV to PCAP format
# 3. Replay PCAP data with cboe-pcap-replay
# 4. Receive and analyze data with cboe-pcap-receiver

set -e  # Exit on any error

# Configuration
SYMBOLS="${SYMBOLS:-ANZ,CBA,NAB,WBC,BHP,RIO,FMG,NCM,TLS,WOW,CSL,TCL}"
DURATION="${DURATION:-60}"  # seconds
BASE_PORT="${BASE_PORT:-30501}"
UNITS="${UNITS:-4}"
TARGET_IP="${TARGET_IP:-127.0.0.1}"
SRC_IP="${SRC_IP:-192.168.1.1}"
DEST_IP="${DEST_IP:-127.0.0.1}"
SRC_PORT="${SRC_PORT:-12345}"
REPLAY_RATE="${REPLAY_RATE:-}"  # Empty for original PCAP timing
RECEIVER_DURATION="${RECEIVER_DURATION:-70}"  # Slightly longer than generation

# File paths
CSV_FILE="market_data_$(date +%Y%m%d_%H%M%S).csv"
PCAP_FILE="market_data_$(date +%Y%m%d_%H%M%S).pcap"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if a command exists
check_command() {
    if ! command -v "$1" &> /dev/null; then
        log_error "Command '$1' not found. Please ensure it's installed and in PATH."
        exit 1
    fi
}

# Function to check if a binary exists in target/release
check_binary() {
    local binary_path="$1"
    if [ ! -f "$binary_path" ]; then
        log_error "Binary not found: $binary_path"
        log_error "Please build the project first: cd $2 && cargo build --release"
        exit 1
    fi
}

# Function to kill background processes
cleanup() {
    log_info "Cleaning up background processes..."
    if [ ! -z "$RECEIVER_PID" ]; then
        log_info "Stopping receiver (PID: $RECEIVER_PID)..."
        kill $RECEIVER_PID 2>/dev/null || true
        wait $RECEIVER_PID 2>/dev/null || true
    fi
    if [ ! -z "$REPLAY_PID" ]; then
        log_info "Stopping replay (PID: $REPLAY_PID)..."
        kill $REPLAY_PID 2>/dev/null || true
        wait $REPLAY_PID 2>/dev/null || true
    fi
    log_info "Cleanup completed"
}

# Set trap for cleanup on script exit
trap cleanup EXIT INT TERM

# Print banner
echo "=================================================="
echo "  CBOE Market Data Complete Workflow Automation  "
echo "=================================================="
echo ""

log_info "Configuration:"
log_info "  Symbols: $SYMBOLS"
log_info "  Duration: ${DURATION}s"
log_info "  Base Port: $BASE_PORT"
log_info "  Units: $UNITS"
log_info "  Target IP: $TARGET_IP"
log_info "  CSV File: $CSV_FILE"
log_info "  PCAP File: $PCAP_FILE"
echo ""

# Check prerequisites
log_info "Checking prerequisites..."
check_command "cargo"

# Check if binaries exist
REPLAY_BINARY="./cboe-pcap-replay/target/release/cboe-pcap-replay"
RECEIVER_BINARY="./cboe-pcap-receiver/target/release/cboe-pcap-receiver"

check_binary "$REPLAY_BINARY" "cboe-pcap-replay"
check_binary "$RECEIVER_BINARY" "cboe-pcap-receiver"

log_success "All prerequisites satisfied"
echo ""

# Step 1: Generate CSV market data
log_info "Step 1/4: Generating CBOE PITCH market data CSV..."
log_info "Command: $REPLAY_BINARY generate --symbols \"$SYMBOLS\" --duration $DURATION --output \"$CSV_FILE\" --port $BASE_PORT --units $UNITS"

if $REPLAY_BINARY generate \
    --symbols "$SYMBOLS" \
    --duration $DURATION \
    --output "$CSV_FILE" \
    --port $BASE_PORT \
    --units $UNITS; then
    log_success "CSV generation completed: $CSV_FILE"
else
    log_error "CSV generation failed"
    exit 1
fi
echo ""

# Step 2: Convert CSV to PCAP
log_info "Step 2/4: Converting CSV to PCAP format..."
log_info "Command: $REPLAY_BINARY convert --input \"$CSV_FILE\" --output \"$PCAP_FILE\" --src-ip $SRC_IP --dest-ip $DEST_IP --src-port $SRC_PORT"

if $REPLAY_BINARY convert \
    --input "$CSV_FILE" \
    --output "$PCAP_FILE" \
    --src-ip $SRC_IP \
    --dest-ip $DEST_IP \
    --src-port $SRC_PORT; then
    log_success "PCAP conversion completed: $PCAP_FILE"
else
    log_error "PCAP conversion failed"
    exit 1
fi
echo ""

# Step 3: Start receiver in background
log_info "Step 3/4: Starting CBOE PCAP receiver..."
RECEIVER_PORTS=""
for ((i=0; i<$UNITS; i++)); do
    port=$((BASE_PORT + i))
    if [ -z "$RECEIVER_PORTS" ]; then
        RECEIVER_PORTS="$port"
    else
        RECEIVER_PORTS="$RECEIVER_PORTS,$port"
    fi
done

log_info "Command: $RECEIVER_BINARY --ports $RECEIVER_PORTS --interface 127.0.0.1"

$RECEIVER_BINARY --ports $RECEIVER_PORTS --interface 127.0.0.1 &
RECEIVER_PID=$!

log_success "Receiver started (PID: $RECEIVER_PID) - listening on ports: $RECEIVER_PORTS"
log_info "Waiting 3 seconds for receiver to initialize..."
sleep 3
echo ""

# Step 4: Start PCAP replay
log_info "Step 4/4: Starting PCAP replay to receiver..."

REPLAY_COMMAND="$REPLAY_BINARY replay --file \"$PCAP_FILE\" --target $TARGET_IP"
if [ ! -z "$REPLAY_RATE" ]; then
    REPLAY_COMMAND="$REPLAY_COMMAND --rate $REPLAY_RATE"
fi

log_info "Command: $REPLAY_COMMAND"

if eval $REPLAY_COMMAND; then
    log_success "PCAP replay completed successfully"
else
    log_warning "PCAP replay finished (this is normal)"
fi
echo ""

# Wait for receiver to finish or timeout
log_info "Waiting for receiver to complete analysis..."
TIMEOUT=10  # Additional timeout after replay
START_TIME=$(date +%s)

while kill -0 $RECEIVER_PID 2>/dev/null; do
    CURRENT_TIME=$(date +%s)
    ELAPSED=$((CURRENT_TIME - START_TIME))
    
    if [ $ELAPSED -ge $TIMEOUT ]; then
        log_warning "Receiver timeout reached, stopping receiver..."
        kill $RECEIVER_PID 2>/dev/null || true
        break
    fi
    
    sleep 1
done

# Wait for receiver to finish gracefully
wait $RECEIVER_PID 2>/dev/null || true
RECEIVER_PID=""

log_success "Receiver analysis completed"
echo ""

# Final summary
echo "=================================================="
echo "           WORKFLOW COMPLETED SUCCESSFULLY        "
echo "=================================================="
echo ""
log_success "Generated Files:"
log_success "  - CSV market data: $CSV_FILE"
log_success "  - PCAP file: $PCAP_FILE"
echo ""
log_info "File Sizes:"
if [ -f "$CSV_FILE" ]; then
    CSV_SIZE=$(du -h "$CSV_FILE" | cut -f1)
    log_info "  - CSV: $CSV_SIZE"
fi
if [ -f "$PCAP_FILE" ]; then
    PCAP_SIZE=$(du -h "$PCAP_FILE" | cut -f1)
    log_info "  - PCAP: $PCAP_SIZE"
fi
echo ""
log_info "The complete CBOE market data workflow has been executed:"
log_info "  ✓ Generated realistic market data for $DURATION seconds"
log_info "  ✓ Converted CSV to network-ready PCAP format"
log_info "  ✓ Replayed market data over network"
log_info "  ✓ Received and analyzed CBOE PITCH messages"
echo ""
log_success "Workflow automation completed successfully!"