use anyhow::{Result, Context};
use byteorder::{LittleEndian, WriteBytesExt};
use csv::Reader;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::net::Ipv4Addr;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
struct CsvRecord {
    timestamp_ns: u64,
    message_type: String,
    port: u16,
    symbol: String,
    hdr_length: u16,
    hdr_count: u8,
    hdr_unit: u8,
    hdr_sequence: u32,
    msg_length: u8,
    // Trading Status fields
    trading_status: Option<String>,
    market_id_code: Option<String>,
    // Add Order fields
    order_id: Option<String>,
    side_indicator: Option<String>,
    quantity: Option<u32>,
    price: Option<u64>,
    participant_id: Option<String>,
    // Order Executed fields
    executed_quantity: Option<u32>,
    execution_id: Option<String>,
    contra_order_id: Option<String>,
    contra_participant_id: Option<String>,
    // Order Executed at Price fields
    execution_type: Option<String>,
    // Reduce Size fields
    cancelled_quantity: Option<u32>,
    // Trade fields
    trade_type: Option<String>,
    trade_designation: Option<String>,
    trade_report_type: Option<String>,
    trade_transaction_time: Option<u64>,
    flags: Option<u8>,
    // Calculated Value fields
    value_category: Option<String>,
    value: Option<u64>,
    value_timestamp: Option<u64>,
    // Auction fields
    auction_type: Option<String>,
    buy_shares: Option<u32>,
    sell_shares: Option<u32>,
    indicative_price: Option<u64>,
    auction_price: Option<u64>,
    auction_shares: Option<u32>,
    // Trade Break fields
    original_execution_id: Option<String>,
    break_reason: Option<String>,
}

fn parse_message_type(msg_type: &str) -> Result<u8> {
    if msg_type.starts_with("0x") {
        Ok(u8::from_str_radix(&msg_type[2..], 16)
            .context("Failed to parse message type hex value")?)
    } else {
        Ok(msg_type.parse::<u8>()
            .context("Failed to parse message type as integer")?)
    }
}

fn encode_sequenced_unit_header(hdr_length: u16, hdr_count: u8, hdr_unit: u8, hdr_sequence: u32) -> Vec<u8> {
    let mut header = Vec::new();
    header.write_u16::<LittleEndian>(hdr_length).unwrap();
    header.push(hdr_count);
    header.push(hdr_unit);
    header.write_u32::<LittleEndian>(hdr_sequence).unwrap();
    header
}

fn pad_string(s: &str, len: usize) -> Vec<u8> {
    let mut result = s.as_bytes().to_vec();
    result.resize(len, b' ');
    result
}

fn pad_string_4(s: &str) -> Vec<u8> {
    let mut result = s.as_bytes().to_vec();
    result.resize(4, b' ');
    result
}

fn encode_base36_order_id(s: &str) -> Vec<u8> {
    // Base36 order IDs are encoded as 8-byte binary values
    // For simplicity, we'll encode the string directly as 8 bytes, padded
    let mut result = s.as_bytes().to_vec();
    result.resize(8, b'\0');
    result
}

fn encode_base36_execution_id(s: &str) -> Vec<u8> {
    // Base36 execution IDs are encoded as 8-byte binary values
    // For simplicity, we'll encode the string directly as 8 bytes, padded
    let mut result = s.as_bytes().to_vec();
    result.resize(8, b'\0');
    result
}

fn encode_pitch_message(record: &CsvRecord) -> Result<Vec<u8>> {
    let mut message = Vec::new();
    let msg_type = parse_message_type(&record.message_type)?;
    
    message.push(record.msg_length);
    message.push(msg_type);

    match msg_type {
        0x3B => {
            // Trading Status message (22 bytes total)
            // Timestamp (8 bytes) - we'll use current nanoseconds
            message.write_u64::<LittleEndian>(record.timestamp_ns).unwrap();
            // Symbol (6 bytes)
            message.extend_from_slice(&pad_string(&record.symbol, 6));
            // Trading Status (1 byte)
            let status = record.trading_status.as_ref()
                .and_then(|s| s.chars().next())
                .unwrap_or('T') as u8;
            message.push(status);
            // Market Id Code (4 bytes)
            let market_id = record.market_id_code.as_ref()
                .map(|s| s.as_str())
                .unwrap_or("XASX");
            message.extend_from_slice(&pad_string_4(market_id));
            // Reserved (1 byte)
            message.push(0);
        }
        0x37 => {
            // Add Order message (42 bytes total)
            if let (Some(order_id), Some(side), Some(quantity), Some(price), Some(participant_id)) = 
                (&record.order_id, &record.side_indicator, record.quantity, record.price, &record.participant_id) {
                
                // Timestamp (8 bytes)
                message.write_u64::<LittleEndian>(record.timestamp_ns).unwrap();
                // Order ID (8 bytes)
                message.extend_from_slice(&encode_base36_order_id(order_id));
                // Side Indicator (1 byte)
                message.push(side.chars().next().unwrap_or('B') as u8);
                // Quantity (4 bytes)
                message.write_u32::<LittleEndian>(quantity).unwrap();
                // Symbol (6 bytes)
                message.extend_from_slice(&pad_string(&record.symbol, 6));
                // Price (8 bytes) - Binary Price field
                message.write_u64::<LittleEndian>(price).unwrap();
                // PID (4 bytes)
                message.extend_from_slice(&pad_string_4(participant_id));
                // Reserved (1 byte)
                message.push(0);
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Add Order message"));
            }
        }
        0x38 => {
            // Order Executed message (43 bytes total)
            if let (Some(order_id), Some(executed_quantity), Some(execution_id), Some(contra_order_id), Some(contra_participant_id)) = 
                (&record.order_id, record.executed_quantity, &record.execution_id, &record.contra_order_id, &record.contra_participant_id) {
                
                // Timestamp (8 bytes)
                message.write_u64::<LittleEndian>(record.timestamp_ns).unwrap();
                // Order ID (8 bytes)
                message.extend_from_slice(&encode_base36_order_id(order_id));
                // Executed Quantity (4 bytes)
                message.write_u32::<LittleEndian>(executed_quantity).unwrap();
                // Execution ID (8 bytes)
                message.extend_from_slice(&encode_base36_execution_id(execution_id));
                // Contra Order ID (8 bytes)
                message.extend_from_slice(&encode_base36_order_id(contra_order_id));
                // Contra PID (4 bytes)
                message.extend_from_slice(&pad_string_4(contra_participant_id));
                // Reserved (1 byte)
                message.push(0);
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Order Executed message"));
            }
        }
        0x39 => {
            // Reduce Size message (22 bytes total)
            if let (Some(order_id), Some(cancelled_quantity)) = 
                (&record.order_id, record.cancelled_quantity) {
                
                // Timestamp (8 bytes)
                message.write_u64::<LittleEndian>(record.timestamp_ns).unwrap();
                // Order ID (8 bytes)
                message.extend_from_slice(&encode_base36_order_id(order_id));
                // Cancelled Quantity (4 bytes)
                message.write_u32::<LittleEndian>(cancelled_quantity).unwrap();
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Reduce Size message"));
            }
        }
        0x3A => {
            // Modify Order message (31 bytes total)
            if let (Some(order_id), Some(quantity), Some(price)) = 
                (&record.order_id, record.quantity, record.price) {
                
                // Timestamp (8 bytes)
                message.write_u64::<LittleEndian>(record.timestamp_ns).unwrap();
                // Order ID (8 bytes)
                message.extend_from_slice(&encode_base36_order_id(order_id));
                // Quantity (4 bytes)
                message.write_u32::<LittleEndian>(quantity).unwrap();
                // Price (8 bytes)
                message.write_u64::<LittleEndian>(price).unwrap();
                // Reserved (1 byte)
                message.push(0);
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Modify Order message"));
            }
        }
        0x3C => {
            // Delete Order message (18 bytes total)
            if let Some(order_id) = &record.order_id {
                // Timestamp (8 bytes)
                message.write_u64::<LittleEndian>(record.timestamp_ns).unwrap();
                // Order ID (8 bytes)
                message.extend_from_slice(&encode_base36_order_id(order_id));
            } else {
                return Err(anyhow::anyhow!("Missing order_id for Delete Order message"));
            }
        }
        0x3D => {
            // Trade message (72 bytes total)
            if let (Some(quantity), Some(price), Some(execution_id), Some(order_id), Some(contra_order_id), 
                   Some(participant_id), Some(contra_participant_id), Some(trade_type), Some(trade_designation), 
                   Some(trade_report_type), Some(trade_transaction_time), Some(flags)) = 
                (record.quantity, record.price, &record.execution_id, &record.order_id, &record.contra_order_id,
                 &record.participant_id, &record.contra_participant_id, &record.trade_type, &record.trade_designation,
                 &record.trade_report_type, record.trade_transaction_time, record.flags) {
                
                // Timestamp (8 bytes)
                message.write_u64::<LittleEndian>(record.timestamp_ns).unwrap();
                // Symbol (6 bytes)
                message.extend_from_slice(&pad_string(&record.symbol, 6));
                // Quantity (4 bytes)
                message.write_u32::<LittleEndian>(quantity).unwrap();
                // Price (8 bytes)
                message.write_u64::<LittleEndian>(price).unwrap();
                // Execution ID (8 bytes)
                message.extend_from_slice(&encode_base36_execution_id(execution_id));
                // Order ID (8 bytes)
                message.extend_from_slice(&encode_base36_order_id(order_id));
                // Contra Order ID (8 bytes)
                message.extend_from_slice(&encode_base36_order_id(contra_order_id));
                // PID (4 bytes)
                message.extend_from_slice(&pad_string_4(participant_id));
                // Contra PID (4 bytes)
                message.extend_from_slice(&pad_string_4(contra_participant_id));
                // Trade Type (1 byte)
                message.push(trade_type.chars().next().unwrap_or('N') as u8);
                // Trade Designation (1 byte)
                message.push(trade_designation.chars().next().unwrap_or('C') as u8);
                // Trade Report Type (1 byte)
                message.push(trade_report_type.chars().next().unwrap_or(' ') as u8);
                // Trade Transaction Time (8 bytes)
                message.write_u64::<LittleEndian>(trade_transaction_time).unwrap();
                // Flags (1 byte)
                message.push(flags);
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Trade message"));
            }
        }
        0xE3 => {
            // Calculated Value message (33 bytes total)
            if let (Some(value_category), Some(value), Some(value_timestamp)) = 
                (&record.value_category, record.value, record.value_timestamp) {
                
                // Timestamp (8 bytes)
                message.write_u64::<LittleEndian>(record.timestamp_ns).unwrap();
                // Symbol (6 bytes)
                message.extend_from_slice(&pad_string(&record.symbol, 6));
                // Value Category (1 byte)
                message.push(value_category.chars().next().unwrap_or('1') as u8);
                // Value (8 bytes)
                message.write_u64::<LittleEndian>(value).unwrap();
                // Value Timestamp (8 bytes)
                message.write_u64::<LittleEndian>(value_timestamp).unwrap();
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Calculated Value message"));
            }
        }
        0x2D => {
            // End of Session message (6 bytes total)
            // Reserved (4 bytes) - per CBOE spec
            message.write_u32::<LittleEndian>(0).unwrap();
        }
        0x59 => {
            // Auction Update message (34 bytes total)
            if let (Some(auction_type), Some(buy_shares), Some(sell_shares), Some(indicative_price)) = 
                (&record.auction_type, record.buy_shares, record.sell_shares, record.indicative_price) {
                
                // Timestamp (8 bytes)
                message.write_u64::<LittleEndian>(record.timestamp_ns).unwrap();
                // Symbol (6 bytes)
                message.extend_from_slice(&pad_string(&record.symbol, 6));
                // Auction Type (1 byte)
                message.push(auction_type.chars().next().unwrap_or('O') as u8);
                // Buy Shares (4 bytes)
                message.write_u32::<LittleEndian>(buy_shares).unwrap();
                // Sell Shares (4 bytes)
                message.write_u32::<LittleEndian>(sell_shares).unwrap();
                // Indicative Price (8 bytes)
                message.write_u64::<LittleEndian>(indicative_price).unwrap();
                // Reserved (1 byte)
                message.push(0);
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Auction Update message"));
            }
        }
        0x5A => {
            // Auction Summary message (30 bytes total)
            if let (Some(auction_type), Some(auction_price), Some(auction_shares)) = 
                (&record.auction_type, record.auction_price, record.auction_shares) {
                
                // Timestamp (8 bytes)
                message.write_u64::<LittleEndian>(record.timestamp_ns).unwrap();
                // Symbol (6 bytes)
                message.extend_from_slice(&pad_string(&record.symbol, 6));
                // Auction Type (1 byte)
                message.push(auction_type.chars().next().unwrap_or('O') as u8);
                // Price (8 bytes)
                message.write_u64::<LittleEndian>(auction_price).unwrap();
                // Shares (4 bytes)
                message.write_u32::<LittleEndian>(auction_shares).unwrap();
                // Reserved (1 byte)
                message.push(0);
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Auction Summary message"));
            }
        }
        0x58 => {
            // Order Executed at Price message (29 bytes total)
            if let (Some(order_id), Some(executed_quantity), Some(execution_id), Some(price)) = 
                (&record.order_id, record.executed_quantity, &record.execution_id, record.price) {
                
                // Timestamp (8 bytes)
                message.write_u64::<LittleEndian>(record.timestamp_ns).unwrap();
                // Order ID (8 bytes)
                message.extend_from_slice(&encode_base36_order_id(order_id));
                // Executed Quantity (4 bytes)
                message.write_u32::<LittleEndian>(executed_quantity).unwrap();
                // Execution ID (8 bytes)
                message.extend_from_slice(&encode_base36_execution_id(execution_id));
                // Price (8 bytes)
                message.write_u64::<LittleEndian>(price).unwrap();
                // Reserved (1 byte)
                message.push(0);
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Order Executed at Price message"));
            }
        }
        0x3E => {
            // Trade Break message (18 bytes total)
            if let (Some(execution_id), Some(break_reason)) = 
                (&record.execution_id, &record.break_reason) {
                
                // Timestamp (8 bytes)
                message.write_u64::<LittleEndian>(record.timestamp_ns).unwrap();
                // Execution ID (8 bytes) - ID of the execution being broken
                message.extend_from_slice(&encode_base36_execution_id(execution_id));
                // Break Reason (1 byte)
                message.push(break_reason.chars().next().unwrap_or('E') as u8);
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Trade Break message"));
            }
        }
        0x97 => {
            // Unit Clear message (6 bytes total)
            // Reserved (4 bytes) - per CBOE spec
            message.write_u32::<LittleEndian>(0).unwrap();
        }
        _ => {
            warn!("Unsupported message type: 0x{:02X}", msg_type);
            return Ok(Vec::new());
        }
    }

    Ok(message)
}

fn create_ethernet_header() -> Vec<u8> {
    let mut header = Vec::new();
    
    // Destination MAC (6 bytes)
    header.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    // Source MAC (6 bytes)
    header.extend_from_slice(&[0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
    // EtherType: IPv4 (2 bytes)
    header.write_u16::<byteorder::BigEndian>(0x0800).unwrap();
    
    header
}

fn create_ip_header(src_ip: Ipv4Addr, dest_ip: Ipv4Addr, payload_len: u16) -> Vec<u8> {
    let mut header = Vec::new();
    
    // Version (4 bits) + IHL (4 bits)
    header.push(0x45);
    // DSCP (6 bits) + ECN (2 bits)
    header.push(0x00);
    // Total Length (2 bytes)
    header.write_u16::<byteorder::BigEndian>(20 + 8 + payload_len).unwrap();
    // Identification (2 bytes)
    header.write_u16::<byteorder::BigEndian>(0x0000).unwrap();
    // Flags (3 bits) + Fragment Offset (13 bits)
    header.write_u16::<byteorder::BigEndian>(0x4000).unwrap();
    // TTL (1 byte)
    header.push(0x40);
    // Protocol: UDP (1 byte)
    header.push(0x11);
    // Header Checksum (2 bytes) - initially zero
    header.write_u16::<byteorder::BigEndian>(0x0000).unwrap();
    // Source IP (4 bytes)
    header.extend_from_slice(&src_ip.octets());
    // Destination IP (4 bytes)
    header.extend_from_slice(&dest_ip.octets());
    
    // Calculate and set checksum
    let checksum = calculate_ip_checksum(&header);
    header[10] = (checksum >> 8) as u8;
    header[11] = (checksum & 0xFF) as u8;
    
    header
}

fn create_udp_header(src_port: u16, dest_port: u16, payload_len: u16) -> Vec<u8> {
    let mut header = Vec::new();
    
    // Source Port (2 bytes)
    header.write_u16::<byteorder::BigEndian>(src_port).unwrap();
    // Destination Port (2 bytes)
    header.write_u16::<byteorder::BigEndian>(dest_port).unwrap();
    // Length (2 bytes)
    header.write_u16::<byteorder::BigEndian>(8 + payload_len).unwrap();
    // Checksum (2 bytes) - set to zero (optional for IPv4)
    header.write_u16::<byteorder::BigEndian>(0x0000).unwrap();
    
    header
}

fn calculate_ip_checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    for chunk in header.chunks(2) {
        if chunk.len() == 2 {
            sum += ((chunk[0] as u32) << 8) + (chunk[1] as u32);
        } else if chunk.len() == 1 {
            sum += (chunk[0] as u32) << 8;
        }
    }
    
    while (sum >> 16) > 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    
    !(sum as u16)
}

fn create_packet(record: &CsvRecord, src_ip: Ipv4Addr, dest_ip: Ipv4Addr, src_port: u16) -> Result<Vec<u8>> {
    let sequenced_header = encode_sequenced_unit_header(
        record.hdr_length,
        record.hdr_count,
        record.hdr_unit,
        record.hdr_sequence,
    );
    
    let pitch_message = encode_pitch_message(record)?;
    if pitch_message.is_empty() {
        return Ok(Vec::new());
    }
    
    let payload = [sequenced_header, pitch_message].concat();
    
    let ethernet_header = create_ethernet_header();
    let ip_header = create_ip_header(src_ip, dest_ip, payload.len() as u16);
    let udp_header = create_udp_header(src_port, record.port, payload.len() as u16);
    
    let packet = [ethernet_header, ip_header, udp_header, payload].concat();
    
    Ok(packet)
}

pub fn convert_csv_to_pcap(
    input: &str,
    output: &str,
    src_ip: &str,
    dest_ip: &str,
    src_port: u16,
) -> Result<()> {
    info!("Converting CBOE PITCH CSV to PCAP (Official Specification Compliant)");
    info!("Input: {}", input);
    info!("Output: {}", output);
    
    let src_ip: Ipv4Addr = src_ip.parse()
        .context("Invalid source IP address")?;
    let dest_ip: Ipv4Addr = dest_ip.parse()
        .context("Invalid destination IP address")?;
    
    let file = File::open(input)
        .context("Failed to open input CSV file")?;
    let mut reader = Reader::from_reader(BufReader::new(file));
    
    let mut records = BTreeMap::new();
    let mut record_count = 0usize;
    
    info!("Reading CSV records...");
    for result in reader.deserialize() {
        let record: CsvRecord = result.context("Failed to deserialize CSV record")?;
        records.insert(record.timestamp_ns, record);
        record_count += 1;
        
        if record_count % 100_000 == 0 {
            info!("Read {} records", record_count);
        }
    }
    
    info!("Read {} total records", record_count);
    info!("Creating PCAP file with official PITCH message formats...");
    
    let mut pcap_writer = pcap_file::pcap::PcapWriter::new(File::create(output)?)
        .context("Failed to create PCAP writer")?;
    
    let mut packet_count = 0usize;
    let mut message_type_counts = std::collections::HashMap::new();
    
    for (timestamp_ns, record) in records {
        let packet = create_packet(&record, src_ip, dest_ip, src_port)?;
        if packet.is_empty() {
            continue;
        }
        
        // Count message types for summary
        *message_type_counts.entry(record.message_type.clone()).or_insert(0) += 1;
        
        let timestamp_sec = timestamp_ns / 1_000_000_000;
        let timestamp_usec = (timestamp_ns % 1_000_000_000) / 1_000;
        
        let timestamp = Duration::new(timestamp_sec, timestamp_usec as u32 * 1000);
        
        let pcap_packet = pcap_file::pcap::PcapPacket::new(
            timestamp,
            packet.len() as u32,
            &packet,
        );
        
        pcap_writer.write_packet(&pcap_packet)
            .context("Failed to write PCAP packet")?;
        
        packet_count += 1;
        
        if packet_count % 100_000 == 0 {
            info!("Wrote {} packets", packet_count);
        }
    }
    
    info!("Successfully converted {} records to {} packets", record_count, packet_count);
    info!("Message type distribution:");
    for (msg_type, count) in message_type_counts {
        info!("  {}: {} packets", msg_type, count);
    }
    info!("PCAP file written to: {}", output);
    
    Ok(())
}