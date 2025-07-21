#!/bin/bash
# Script to build and run the BPF-based UDP receiver test tool

# Check if running as root (BPF typically requires root privileges)
if [ "$(id -u)" -ne 0 ]; then
  echo "Warning: This script should be run as root (sudo) for proper BPF access"
  echo "Continuing, but you may encounter permission errors..."
fi

# Build the tool
echo "Building BPF-based UDP receiver test tool..."
cargo build --release

# Determine default interface
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
  DEFAULT_INTERFACE=$(ip -o -4 route show to default | awk '{print $5}' | head -1)
elif [[ "$OSTYPE" == "darwin"* ]]; then
  DEFAULT_INTERFACE="lo0"  # Default loopback on macOS
else
  DEFAULT_INTERFACE="lo"   # Default for others
fi

# Get parameters from environment or use defaults
INTERFACE=${INTERFACE:-$DEFAULT_INTERFACE}
PORTS=${PORTS:-30501,30502}
INTERVAL=${INTERVAL:-1}

echo "Starting BPF-based UDP receiver on interface $INTERFACE..."
echo "Capturing ports: $PORTS"
echo "Reporting interval: $INTERVAL seconds"

# Run the tool with sudo to ensure BPF access
sudo cargo run --release -- -p $PORTS -i $INTERFACE -i $INTERVAL $@

# Usage examples:
# ./run-receiver.sh                      # Default: listen on ports 30501,30502 on default interface
# INTERFACE=en0 ./run-receiver.sh        # Listen on en0 interface
# PORTS=7123,7124 ./run-receiver.sh      # Listen on custom ports
# INTERVAL=5 ./run-receiver.sh           # Report every 5 seconds
# TIME=60 ./run-receiver.sh -t 60        # Run for 60 seconds then exit