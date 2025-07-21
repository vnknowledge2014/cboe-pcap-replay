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
    order_id: Option<String>,
    side: Option<String>,
    shares: Option<u64>,
    price: Option<u64>,
    display: Option<String>,
    participant_id: Option<String>,
    time_in_force: Option<u32>,
    firm: Option<String>,
    status: Option<String>,
    execution_id: Option<String>,
    liquidity_flag: Option<String>,
    match_number: Option<u64>,
    trade_id: Option<String>,
    trade_price: Option<u64>,
    trade_quantity: Option<u64>,
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

fn encode_pitch_message(record: &CsvRecord) -> Result<Vec<u8>> {
    let mut message = Vec::new();
    let msg_type = parse_message_type(&record.message_type)?;
    
    message.push(record.msg_length);
    message.push(msg_type);

    match msg_type {
        0x31 => {
            message.extend_from_slice(&pad_string(&record.symbol, 6));
            message.push(record.status.as_ref().unwrap_or(&"H".to_string()).chars().next().unwrap_or('H') as u8);
        }
        0x37 => {
            if let (Some(order_id), Some(side), Some(shares), Some(price), Some(display), Some(participant_id), Some(time_in_force), Some(firm)) = 
                (&record.order_id, &record.side, record.shares, record.price, &record.display, &record.participant_id, record.time_in_force, &record.firm) {
                
                message.extend_from_slice(&pad_string(order_id, 12));
                message.push(side.chars().next().unwrap_or('B') as u8);
                message.write_u32::<LittleEndian>(shares as u32).unwrap();
                message.extend_from_slice(&pad_string(&record.symbol, 6));
                message.write_u64::<LittleEndian>(price).unwrap();
                message.push(display.chars().next().unwrap_or('Y') as u8);
                message.extend_from_slice(&pad_string(participant_id, 4));
                message.write_u32::<LittleEndian>(time_in_force).unwrap();
                message.extend_from_slice(&pad_string(firm, 4));
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Add Order message"));
            }
        }
        0x38 => {
            if let (Some(order_id), Some(execution_id), Some(liquidity_flag), Some(match_number)) = 
                (&record.order_id, &record.execution_id, &record.liquidity_flag, record.match_number) {
                
                message.extend_from_slice(&pad_string(order_id, 12));
                message.write_u32::<LittleEndian>(record.shares.unwrap_or(0) as u32).unwrap();
                message.extend_from_slice(&pad_string(execution_id, 12));
                message.push(liquidity_flag.chars().next().unwrap_or('A') as u8);
                message.write_u64::<LittleEndian>(match_number).unwrap();
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Order Executed message"));
            }
        }
        0x3C => {
            if let Some(order_id) = &record.order_id {
                message.extend_from_slice(&pad_string(order_id, 12));
            } else {
                return Err(anyhow::anyhow!("Missing order_id for Delete Order message"));
            }
        }
        0x3D => {
            if let (Some(trade_id), Some(side), Some(trade_price), Some(trade_quantity), Some(match_number)) = 
                (&record.trade_id, &record.side, record.trade_price, record.trade_quantity, record.match_number) {
                
                message.extend_from_slice(&pad_string(trade_id, 12));
                message.push(side.chars().next().unwrap_or('B') as u8);
                message.extend_from_slice(&pad_string(&record.symbol, 6));
                message.write_u32::<LittleEndian>(trade_quantity as u32).unwrap();
                message.write_u64::<LittleEndian>(trade_price).unwrap();
                message.write_u64::<LittleEndian>(match_number).unwrap();
            } else {
                return Err(anyhow::anyhow!("Missing required fields for Trade message"));
            }
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
    
    header.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    header.extend_from_slice(&[0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
    header.write_u16::<byteorder::BigEndian>(0x0800).unwrap();
    
    header
}

fn create_ip_header(src_ip: Ipv4Addr, dest_ip: Ipv4Addr, payload_len: u16) -> Vec<u8> {
    let mut header = Vec::new();
    
    header.push(0x45);
    header.push(0x00);
    header.write_u16::<byteorder::BigEndian>(20 + 8 + payload_len).unwrap();
    header.write_u16::<byteorder::BigEndian>(0x0000).unwrap();
    header.write_u16::<byteorder::BigEndian>(0x4000).unwrap();
    header.push(0x40);
    header.push(0x11);
    header.write_u16::<byteorder::BigEndian>(0x0000).unwrap();
    header.extend_from_slice(&src_ip.octets());
    header.extend_from_slice(&dest_ip.octets());
    
    let checksum = calculate_ip_checksum(&header);
    header[10] = (checksum >> 8) as u8;
    header[11] = (checksum & 0xFF) as u8;
    
    header
}

fn create_udp_header(src_port: u16, dest_port: u16, payload_len: u16) -> Vec<u8> {
    let mut header = Vec::new();
    
    header.write_u16::<byteorder::BigEndian>(src_port).unwrap();
    header.write_u16::<byteorder::BigEndian>(dest_port).unwrap();
    header.write_u16::<byteorder::BigEndian>(8 + payload_len).unwrap();
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
    info!("Converting CSV to PCAP");
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
    info!("Creating PCAP file...");
    
    let mut pcap_writer = pcap_file::pcap::PcapWriter::new(File::create(output)?)
        .context("Failed to create PCAP writer")?;
    
    let mut packet_count = 0usize;
    for (timestamp_ns, record) in records {
        let packet = create_packet(&record, src_ip, dest_ip, src_port)?;
        if packet.is_empty() {
            continue;
        }
        
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
    info!("PCAP file written to: {}", output);
    
    Ok(())
}