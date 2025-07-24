use std::net::SocketAddr;

/// Packet data structure for high-performance processing
#[derive(Debug, Clone)]
pub struct PacketData {
    pub dest_port: u16,
    pub timestamp: f64,
    pub payload: Vec<u8>,
}

/// Represents a received UDP packet with CBOE PITCH data
#[derive(Debug, Clone)]
pub struct ReceivedPacket {
    pub dest_port: u16,
    pub timestamp: f64,
    pub payload: Vec<u8>,
}

impl ReceivedPacket {
    pub fn new(_source_addr: SocketAddr, dest_port: u16, payload: Vec<u8>) -> Self {
        Self {
            dest_port,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            payload,
        }
    }
}

/// CBOE PITCH Sequenced Unit Header
#[derive(Debug, Clone, Copy)]
pub struct PitchHeader {
    pub unit: u8,
    pub sequence: u32,
}

impl PitchHeader {
    /// Parse PITCH header from bytes
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        let unit = data[3];
        let sequence = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

        Some(Self {
            unit,
            sequence,
        })
    }
}

/// CBOE PITCH Message Types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PitchMessageType {
    Time,
    AddOrderLong,
    AddOrderShort,
    OrderExecuted,
    OrderCancel,
    Trade,
    TradingStatus,
    AuctionUpdate,
    AuctionSummary,
    UnitClear,
    Unknown(u8),
}

impl From<u8> for PitchMessageType {
    fn from(byte: u8) -> Self {
        match byte {
            0x20 => PitchMessageType::Time,
            0x97 => PitchMessageType::UnitClear,
            0x21 => PitchMessageType::AddOrderLong,
            0x22 => PitchMessageType::AddOrderShort,
            0x23 => PitchMessageType::OrderExecuted,
            0x24 => PitchMessageType::OrderCancel,
            0x2A => PitchMessageType::Trade,
            0x31 => PitchMessageType::TradingStatus,
            0x95 => PitchMessageType::AuctionUpdate,
            0x96 => PitchMessageType::AuctionSummary,
            other => PitchMessageType::Unknown(other),
        }
    }
}

/// Parse message type from PITCH packet payload
pub fn parse_message_type(data: &[u8]) -> Option<PitchMessageType> {
    if data.len() < 10 { // 8 bytes header + 2 bytes message minimum
        return None;
    }

    // Skip sequenced unit header (8 bytes) and message length (1 byte)
    Some(PitchMessageType::from(data[9])) // Message type is at offset 9
}