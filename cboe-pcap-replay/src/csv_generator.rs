use anyhow::Result;
use csv::Writer;
use chrono::{DateTime, Utc};
use rand::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use tracing::{info};

#[derive(Debug, Serialize)]
pub struct CsvRecord {
    pub timestamp_ns: u64,
    pub message_type: String,
    pub port: u16,
    pub symbol: String,
    pub hdr_length: u16,
    pub hdr_count: u8,
    pub hdr_unit: u8,
    pub hdr_sequence: u32,
    pub msg_length: u8,
    pub order_id: String,
    pub side: Option<char>,
    pub shares: Option<u64>,
    pub price: Option<u64>,
    pub display: Option<char>,
    pub participant_id: Option<String>,
    pub time_in_force: Option<u32>,
    pub firm: Option<String>,
    pub status: Option<char>,
    pub execution_id: Option<String>,
    pub liquidity_flag: Option<char>,
    pub match_number: Option<u64>,
    pub trade_id: Option<String>,
    pub trade_price: Option<u64>,
    pub trade_quantity: Option<u64>,
}

#[derive(Debug, Clone)]
struct OrderBook {
    orders: HashMap<String, Order>,
    next_order_id: u64,
    next_execution_id: u64,
    next_match_number: u64,
}

#[derive(Debug, Clone)]
struct Order {
    order_id: String,
    symbol: String,
    side: char,
    shares: u64,
    price: u64,
    display: char,
    participant_id: String,
    firm: String,
    timestamp: u64,
}

struct SequenceTracker {
    sequences: HashMap<u8, u32>,
}

impl SequenceTracker {
    fn new() -> Self {
        Self {
            sequences: HashMap::new(),
        }
    }

    fn next_sequence(&mut self, unit: u8) -> u32 {
        let seq = self.sequences.entry(unit).or_insert(0);
        *seq += 1;
        *seq
    }
}

impl OrderBook {
    fn new() -> Self {
        Self {
            orders: HashMap::new(),
            next_order_id: 1,
            next_execution_id: 1,
            next_match_number: 1,
        }
    }

    fn next_order_id(&mut self) -> String {
        let id = format!("{:012}", to_base36(self.next_order_id));
        self.next_order_id += 1;
        id
    }

    fn next_execution_id(&mut self) -> String {
        let id = format!("{:012}", to_base36(self.next_execution_id));
        self.next_execution_id += 1;
        id
    }


    fn next_match_number(&mut self) -> u64 {
        let id = self.next_match_number;
        self.next_match_number += 1;
        id
    }

    fn add_order(&mut self, order: Order) {
        self.orders.insert(order.order_id.clone(), order);
    }

    fn get_random_order(&self, rng: &mut ThreadRng) -> Option<&Order> {
        if self.orders.is_empty() {
            return None;
        }
        let keys: Vec<_> = self.orders.keys().collect();
        let key = keys.choose(rng)?;
        self.orders.get(*key)
    }

    fn remove_order(&mut self, order_id: &str) {
        self.orders.remove(order_id);
    }
}

fn to_base36(num: u64) -> String {
    const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if num == 0 {
        return "0".to_string();
    }

    let mut result = String::new();
    let mut n = num;
    while n > 0 {
        result.push(CHARS[(n % 36) as usize] as char);
        n /= 36;
    }
    result.chars().rev().collect()
}

fn price_to_cboe_format(price_dollars: f64) -> u64 {
    (price_dollars * 10_000_000.0) as u64
}

fn generate_realistic_price(symbol: &str, rng: &mut ThreadRng) -> f64 {
    let base_price = match symbol {
        "AAPL" => 150.0,
        "MSFT" => 300.0,
        "GOOGL" => 2800.0,
        "TSLA" => 200.0,
        "AMZN" => 3200.0,
        "META" => 350.0,
        "NVDA" => 800.0,
        "NFLX" => 450.0,
        _ => 100.0,
    };
    
    let variation = rng.gen_range(-0.05..0.05);
    base_price * (1.0 + variation)
}

pub fn generate_csv(
    symbols: &str,
    duration: u64,
    output: &str,
    port: u16,
    units: u8,
) -> Result<()> {
    let symbols: Vec<&str> = symbols.split(',').collect();
    
    info!("Starting CBOE CSV generation");
    info!("Symbols: {:?}", symbols);
    info!("Duration: {} seconds", duration);
    info!("Output file: {}", output);

    let mut rng = thread_rng();
    let mut order_books: HashMap<String, OrderBook> = HashMap::new();
    let mut sequence_tracker = SequenceTracker::new();
    
    for symbol in &symbols {
        order_books.insert(symbol.to_string(), OrderBook::new());
    }

    let file = File::create(output)?;
    let mut writer = Writer::from_writer(file);

    let start_time = Utc::now();
    let mut current_time_ns = start_time.timestamp_nanos_opt().unwrap() as u64;
    let end_time_ns = current_time_ns + (duration * 1_000_000_000);

    info!("Generating market data from {} to {}", start_time, 
          DateTime::from_timestamp_nanos(end_time_ns as i64));

    let participants = vec!["ARCA", "BATS", "EDGX", "NSDQ", "NYSE", "CBOE"];
    let firms = vec!["CITD", "VIRT", "JANE", "JUMP", "GSCO", "MSCO"];
    
    let mut message_count = 0u64;

    // Generate trading status messages for market open
    for symbol in &symbols {
        let unit = (message_count % units as u64) as u8 + 1;
        let port_num = port + (unit - 1) as u16;
        let sequence = sequence_tracker.next_sequence(unit);
        
        let record = CsvRecord {
            timestamp_ns: current_time_ns,
            message_type: "0x31".to_string(),
            port: port_num,
            symbol: format!("{:<6}", symbol),
            hdr_length: 16,
            hdr_count: 1,
            hdr_unit: unit,
            hdr_sequence: sequence,
            msg_length: 9,
            order_id: String::new(),
            side: None,
            shares: None,
            price: None,
            display: None,
            participant_id: None,
            time_in_force: None,
            firm: None,
            status: Some('H'),
            execution_id: None,
            liquidity_flag: None,
            match_number: None,
            trade_id: None,
            trade_price: None,
            trade_quantity: None,
        };
        
        writer.serialize(&record)?;
        message_count += 1;
        current_time_ns += rng.gen_range(100_000..1_000_000);
    }

    // Generate market data messages
    while current_time_ns < end_time_ns {
        let symbol = symbols.choose(&mut rng).unwrap();
        let unit = (message_count % units as u64) as u8 + 1;
        let port_num = port + (unit - 1) as u16;
        let sequence = sequence_tracker.next_sequence(unit);
        
        let order_book = order_books.get_mut(*symbol).unwrap();

        let action = if order_book.orders.len() < 10 {
            0
        } else {
            rng.gen_range(0..10)
        };

        match action {
            0..=6 => {
                // Add Order
                let order_id = order_book.next_order_id();
                let side = if rng.gen_bool(0.5) { 'B' } else { 'S' };
                let shares = rng.gen_range(100..10000);
                let price = price_to_cboe_format(generate_realistic_price(symbol, &mut rng));
                let display = if rng.gen_bool(0.8) { 'Y' } else { 'N' };
                let participant_id = participants.choose(&mut rng).unwrap().to_string();
                let firm = firms.choose(&mut rng).unwrap().to_string();

                let order = Order {
                    order_id: order_id.clone(),
                    symbol: symbol.to_string(),
                    side,
                    shares,
                    price,
                    display,
                    participant_id: participant_id.clone(),
                    firm: firm.clone(),
                    timestamp: current_time_ns,
                };

                let record = CsvRecord {
                    timestamp_ns: current_time_ns,
                    message_type: "0x37".to_string(),
                    port: port_num,
                    symbol: format!("{:<6}", symbol),
                    hdr_length: 42,
                    hdr_count: 1,
                    hdr_unit: unit,
                    hdr_sequence: sequence,
                    msg_length: 34,
                    order_id: order_id.clone(),
                    side: Some(side),
                    shares: Some(shares),
                    price: Some(price),
                    display: Some(display),
                    participant_id: Some(participant_id),
                    time_in_force: Some(0),
                    firm: Some(firm),
                    status: None,
                    execution_id: None,
                    liquidity_flag: None,
                    match_number: None,
                    trade_id: None,
                    trade_price: None,
                    trade_quantity: None,
                };

                order_book.add_order(order);
                writer.serialize(&record)?;
            }
            7..=8 => {
                // Execute Order
                if let Some(order) = order_book.get_random_order(&mut rng).cloned() {
                    let execution_id = order_book.next_execution_id();
                    let match_number = order_book.next_match_number();
                    let shares_executed = std::cmp::min(order.shares, rng.gen_range(100..order.shares + 1));

                    let record = CsvRecord {
                        timestamp_ns: current_time_ns,
                        message_type: "0x38".to_string(),
                        port: port_num,
                        symbol: format!("{:<6}", symbol),
                        hdr_length: 37,
                        hdr_count: 1,
                        hdr_unit: unit,
                        hdr_sequence: sequence,
                        msg_length: 29,
                        order_id: order.order_id.clone(),
                        side: None,
                        shares: Some(shares_executed),
                        price: None,
                        display: None,
                        participant_id: None,
                        time_in_force: None,
                        firm: None,
                        status: None,
                        execution_id: Some(execution_id),
                        liquidity_flag: Some('A'),
                        match_number: Some(match_number),
                        trade_id: None,
                        trade_price: None,
                        trade_quantity: None,
                    };
                    writer.serialize(&record)?;

                    if shares_executed >= order.shares {
                        order_book.remove_order(&order.order_id);
                    }
                }
            }
            _ => {
                // Delete Order
                if let Some(order) = order_book.get_random_order(&mut rng).cloned() {
                    let record = CsvRecord {
                        timestamp_ns: current_time_ns,
                        message_type: "0x3C".to_string(),
                        port: port_num,
                        symbol: format!("{:<6}", symbol),
                        hdr_length: 20,
                        hdr_count: 1,
                        hdr_unit: unit,
                        hdr_sequence: sequence,
                        msg_length: 12,
                        order_id: order.order_id.clone(),
                        side: None,
                        shares: None,
                        price: None,
                        display: None,
                        participant_id: None,
                        time_in_force: None,
                        firm: None,
                        status: None,
                        execution_id: None,
                        liquidity_flag: None,
                        match_number: None,
                        trade_id: None,
                        trade_price: None,
                        trade_quantity: None,
                    };
                    writer.serialize(&record)?;
                    order_book.remove_order(&order.order_id);
                }
            }
        }

        message_count += 1;
        current_time_ns += rng.gen_range(1_000..100_000);

        if message_count % 100_000 == 0 {
            info!("Generated {} messages", message_count);
        }
    }

    writer.flush()?;
    
    info!("Generated {} total messages", message_count);
    info!("CSV file written to: {}", output);
    
    Ok(())
}