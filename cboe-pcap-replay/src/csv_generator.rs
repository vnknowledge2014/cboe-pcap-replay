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
    // Trading Status fields
    pub trading_status: Option<char>,
    pub market_id_code: Option<String>,
    // Add Order fields
    pub order_id: Option<String>,
    pub side_indicator: Option<char>,
    pub quantity: Option<u32>,
    pub price: Option<u64>,
    pub participant_id: Option<String>,
    // Order Executed fields
    pub executed_quantity: Option<u32>,
    pub execution_id: Option<String>,
    pub contra_order_id: Option<String>,
    pub contra_participant_id: Option<String>,
    // Order Executed at Price fields
    pub execution_type: Option<char>,
    // Reduce Size fields
    pub cancelled_quantity: Option<u32>,
    // Trade fields
    pub trade_type: Option<char>,
    pub trade_designation: Option<char>,
    pub trade_report_type: Option<char>,
    pub trade_transaction_time: Option<u64>,
    pub flags: Option<u8>,
    // Calculated Value fields
    pub value_category: Option<char>,
    pub value: Option<u64>,
    pub value_timestamp: Option<u64>,
    // Auction fields
    pub auction_type: Option<char>,
    pub buy_shares: Option<u32>,
    pub sell_shares: Option<u32>,
    pub indicative_price: Option<u64>,
    pub auction_price: Option<u64>,
    pub auction_shares: Option<u32>,
    // Trade Break fields
    pub original_execution_id: Option<String>,
    pub break_reason: Option<char>,
}

#[derive(Debug, Clone)]
struct OrderBook {
    orders: HashMap<String, Order>,
    next_order_id: u64,
    next_execution_id: u64,
    execution_history: Vec<String>, // Track execution IDs for trade breaks
}

#[derive(Debug, Clone)]
struct Order {
    order_id: String,
    _symbol: String,
    _side: char,
    quantity: u32,
    _price: u64,
    _participant_id: String,
    _timestamp: u64,
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
            execution_history: Vec::new(),
        }
    }

    fn next_order_id(&mut self) -> String {
        let id = format!("{:012}", to_base36(self.next_order_id));
        self.next_order_id += 1;
        id
    }

    fn next_execution_id(&mut self) -> String {
        let id = format!("{:09}", to_base36(self.next_execution_id));
        self.next_execution_id += 1;
        self.execution_history.push(id.clone());
        id
    }

    fn get_random_execution_id(&self, rng: &mut ThreadRng) -> Option<String> {
        if self.execution_history.is_empty() {
            return None;
        }
        self.execution_history.choose(rng).cloned()
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
        // Major Australian banks
        "ANZ" => 28.50,    // Australia and New Zealand Banking Group
        "CBA" => 105.00,   // Commonwealth Bank of Australia
        "NAB" => 35.20,    // National Australia Bank
        "WBC" => 26.80,    // Westpac Banking Corporation
        // Australian miners
        "BHP" => 45.60,    // BHP Billiton
        "RIO" => 125.40,   // Rio Tinto
        "FMG" => 22.15,    // Fortescue Metals Group
        "NCM" => 28.90,    // Newcrest Mining
        // Australian tech and telecom
        "TLS" => 4.20,     // Telstra Corporation
        "WOW" => 38.50,    // Woolworths Group
        "CSL" => 285.00,   // CSL Limited
        "TCL" => 14.75,    // Transurban Group
        // Australian retail and services
        "COL" => 18.90,    // Coles Group
        "WES" => 65.40,    // Wesfarmers
        "QAN" => 6.25,     // Qantas Airways
        "SYD" => 8.45,     // Sydney Airport
        _ => 25.00,         // Default for unknown AU symbols
    };
    
    let variation = rng.gen_range(-0.05..0.05);
    base_price * (1.0 + variation)
}

fn create_empty_record() -> CsvRecord {
    CsvRecord {
        timestamp_ns: 0,
        message_type: String::new(),
        port: 0,
        symbol: String::new(),
        hdr_length: 0,
        hdr_count: 0,
        hdr_unit: 0,
        hdr_sequence: 0,
        msg_length: 0,
        trading_status: None,
        market_id_code: None,
        order_id: None,
        side_indicator: None,
        quantity: None,
        price: None,
        participant_id: None,
        executed_quantity: None,
        execution_id: None,
        contra_order_id: None,
        contra_participant_id: None,
        execution_type: None,
        cancelled_quantity: None,
        trade_type: None,
        trade_designation: None,
        trade_report_type: None,
        trade_transaction_time: None,
        flags: None,
        value_category: None,
        value: None,
        value_timestamp: None,
        auction_type: None,
        buy_shares: None,
        sell_shares: None,
        indicative_price: None,
        auction_price: None,
        auction_shares: None,
        original_execution_id: None,
        break_reason: None,
    }
}

pub fn generate_csv(
    symbols: &str,
    duration: u64,
    output: &str,
    port: u16,
    units: u8,
) -> Result<()> {
    let symbols: Vec<&str> = symbols.split(',').collect();
    
    info!("Starting CBOE PITCH CSV generation (Realistic Trading Session)");
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
    let session_duration = duration * 1_000_000_000;
    let pre_open_end = current_time_ns + (session_duration / 10); // 10% pre-open
    let continuous_end = current_time_ns + (session_duration * 9 / 10); // 90% continuous

    info!("Generating realistic trading session from {} to {}", start_time, 
          DateTime::from_timestamp_nanos(end_time_ns as i64));

    let participants = vec!["ARCA", "BATS", "EDGX", "NSDQ", "NYSE", "CBOE"];
    let market_id_codes = vec!["XASX", "CXAW", "CXAE", "CXAQ", "CXAL"];
    
    let mut message_count = 0u64;

    // PHASE 1: PRE-OPEN - Trading Status (Halt) + Auction Updates
    for symbol in &symbols {
        let unit = (message_count % units as u64) as u8 + 1;
        let port_num = port + (unit - 1) as u16;
        let sequence = sequence_tracker.next_sequence(unit);
        
        // Trading Status: Halt (H)
        let mut record = create_empty_record();
        record.timestamp_ns = current_time_ns;
        record.message_type = "0x3B".to_string();
        record.port = port_num;
        record.symbol = format!("{:<6}", symbol);
        record.hdr_length = 16 + 22;
        record.hdr_count = 1;
        record.hdr_unit = unit;
        record.hdr_sequence = sequence;
        record.msg_length = 22;
        record.trading_status = Some('H'); // Halt before open
        record.market_id_code = Some(market_id_codes.choose(&mut rng).unwrap().to_string());
        
        writer.serialize(&record)?;
        message_count += 1;
        current_time_ns += 100_000; // 0.1ms
    }

    // Pre-open auction updates
    while current_time_ns < pre_open_end {
        for symbol in &symbols {
            let unit = (message_count % units as u64) as u8 + 1;
            let port_num = port + (unit - 1) as u16;
            let sequence = sequence_tracker.next_sequence(unit);
            
            // Auction Update (0x59)
            let mut record = create_empty_record();
            record.timestamp_ns = current_time_ns;
            record.message_type = "0x59".to_string();
            record.port = port_num;
            record.symbol = format!("{:<6}", symbol);
            record.hdr_length = 16 + 34;
            record.hdr_count = 1;
            record.hdr_unit = unit;
            record.hdr_sequence = sequence;
            record.msg_length = 34;
            record.auction_type = Some('O'); // Opening auction
            record.buy_shares = Some(rng.gen_range(1000..50000));
            record.sell_shares = Some(rng.gen_range(1000..50000));
            record.indicative_price = Some(price_to_cboe_format(generate_realistic_price(symbol, &mut rng)));
            
            writer.serialize(&record)?;
            message_count += 1;
            current_time_ns += rng.gen_range(1_000_000..5_000_000); // 1-5ms
        }
    }

    // Auction Summary before market open
    for symbol in &symbols {
        let unit = (message_count % units as u64) as u8 + 1;
        let port_num = port + (unit - 1) as u16;
        let sequence = sequence_tracker.next_sequence(unit);
        
        let mut record = create_empty_record();
        record.timestamp_ns = current_time_ns;
        record.message_type = "0x5A".to_string();
        record.port = port_num;
        record.symbol = format!("{:<6}", symbol);
        record.hdr_length = 16 + 30;
        record.hdr_count = 1;
        record.hdr_unit = unit;
        record.hdr_sequence = sequence;
        record.msg_length = 30;
        record.auction_type = Some('O'); // Opening auction
        record.auction_price = Some(price_to_cboe_format(generate_realistic_price(symbol, &mut rng)));
        record.auction_shares = Some(rng.gen_range(5000..25000));
        
        writer.serialize(&record)?;
        message_count += 1;
        current_time_ns += 100_000;
    }

    // PHASE 2: MARKET OPEN - Trading Status (Trading)
    for symbol in &symbols {
        let unit = (message_count % units as u64) as u8 + 1;
        let port_num = port + (unit - 1) as u16;
        let sequence = sequence_tracker.next_sequence(unit);
        
        let mut record = create_empty_record();
        record.timestamp_ns = current_time_ns;
        record.message_type = "0x3B".to_string();
        record.port = port_num;
        record.symbol = format!("{:<6}", symbol);
        record.hdr_length = 16 + 22;
        record.hdr_count = 1;
        record.hdr_unit = unit;
        record.hdr_sequence = sequence;
        record.msg_length = 22;
        record.trading_status = Some('T'); // Trading
        record.market_id_code = Some(market_id_codes.choose(&mut rng).unwrap().to_string());
        
        writer.serialize(&record)?;
        message_count += 1;
        current_time_ns += 100_000;
    }

    // PHASE 3: CONTINUOUS TRADING - Realistic order lifecycle
    while current_time_ns < continuous_end {
        let symbol = symbols.choose(&mut rng).unwrap();
        let unit = (message_count % units as u64) as u8 + 1;
        let port_num = port + (unit - 1) as u16;
        let sequence = sequence_tracker.next_sequence(unit);
        
        let order_book = order_books.get_mut(*symbol).unwrap();

        let action = if order_book.orders.len() < 10 {
            0 // Force add order
        } else {
            rng.gen_range(0..10)
        };

        match action {
            0..=4 => {
                // Add Order (0x37)
                let order_id = order_book.next_order_id();
                let side = if rng.gen_bool(0.5) { 'B' } else { 'S' };
                let quantity = rng.gen_range(100..10000);
                let price = price_to_cboe_format(generate_realistic_price(symbol, &mut rng));
                let participant_id = participants.choose(&mut rng).unwrap().to_string();

                let order = Order {
                    order_id: order_id.clone(),
                    _symbol: symbol.to_string(),
                    _side: side,
                    quantity,
                    _price: price,
                    _participant_id: participant_id.clone(),
                    _timestamp: current_time_ns,
                };

                let mut record = create_empty_record();
                record.timestamp_ns = current_time_ns;
                record.message_type = "0x37".to_string();
                record.port = port_num;
                record.symbol = format!("{:<6}", symbol);
                record.hdr_length = 16 + 42; // Header + message length
                record.hdr_count = 1;
                record.hdr_unit = unit;
                record.hdr_sequence = sequence;
                record.msg_length = 42;
                record.order_id = Some(order_id.clone());
                record.side_indicator = Some(side);
                record.quantity = Some(quantity);
                record.price = Some(price);
                record.participant_id = Some(participant_id);

                order_book.add_order(order);
                writer.serialize(&record)?;
            }
            5..=6 => {
                // Order Executed (0x38)
                if let Some(order) = order_book.get_random_order(&mut rng).cloned() {
                    let execution_id = order_book.next_execution_id();
                    let executed_quantity = std::cmp::min(order.quantity, rng.gen_range(100..order.quantity + 1));
                    let contra_order_id = format!("{:012}", to_base36(rng.gen_range(1..1000000)));
                    let contra_participant_id = participants.choose(&mut rng).unwrap().to_string();

                    let mut record = create_empty_record();
                    record.timestamp_ns = current_time_ns;
                    record.message_type = "0x38".to_string();
                    record.port = port_num;
                    record.symbol = format!("{:<6}", symbol);
                    record.hdr_length = 16 + 43; // Header + message length
                    record.hdr_count = 1;
                    record.hdr_unit = unit;
                    record.hdr_sequence = sequence;
                    record.msg_length = 43;
                    record.order_id = Some(order.order_id.clone());
                    record.executed_quantity = Some(executed_quantity);
                    record.execution_id = Some(execution_id);
                    record.contra_order_id = Some(contra_order_id);
                    record.contra_participant_id = Some(contra_participant_id);

                    writer.serialize(&record)?;

                    if executed_quantity >= order.quantity {
                        order_book.remove_order(&order.order_id);
                    }
                }
            }
            7 => {
                // Reduce Size (0x39)
                if let Some(order) = order_book.get_random_order(&mut rng).cloned() {
                    let cancelled_quantity = rng.gen_range(1..=order.quantity / 2);

                    let mut record = create_empty_record();
                    record.timestamp_ns = current_time_ns;
                    record.message_type = "0x39".to_string();
                    record.port = port_num;
                    record.symbol = format!("{:<6}", symbol);
                    record.hdr_length = 16 + 22; // Header + message length
                    record.hdr_count = 1;
                    record.hdr_unit = unit;
                    record.hdr_sequence = sequence;
                    record.msg_length = 22;
                    record.order_id = Some(order.order_id.clone());
                    record.cancelled_quantity = Some(cancelled_quantity);

                    writer.serialize(&record)?;
                }
            }
            8 => {
                // Modify Order (0x3A)
                if let Some(order) = order_book.get_random_order(&mut rng).cloned() {
                    let new_quantity = rng.gen_range(100..10000);
                    let new_price = price_to_cboe_format(generate_realistic_price(symbol, &mut rng));

                    let mut record = create_empty_record();
                    record.timestamp_ns = current_time_ns;
                    record.message_type = "0x3A".to_string();
                    record.port = port_num;
                    record.symbol = format!("{:<6}", symbol);
                    record.hdr_length = 16 + 31; // Header + message length
                    record.hdr_count = 1;
                    record.hdr_unit = unit;
                    record.hdr_sequence = sequence;
                    record.msg_length = 31;
                    record.order_id = Some(order.order_id.clone());
                    record.quantity = Some(new_quantity);
                    record.price = Some(new_price);

                    writer.serialize(&record)?;
                }
            }
            _ => {
                // Delete Order (0x3C)
                if let Some(order) = order_book.get_random_order(&mut rng).cloned() {
                    let mut record = create_empty_record();
                    record.timestamp_ns = current_time_ns;
                    record.message_type = "0x3C".to_string();
                    record.port = port_num;
                    record.symbol = format!("{:<6}", symbol);
                    record.hdr_length = 16 + 18; // Header + message length
                    record.hdr_count = 1;
                    record.hdr_unit = unit;
                    record.hdr_sequence = sequence;
                    record.msg_length = 18;
                    record.order_id = Some(order.order_id.clone());

                    writer.serialize(&record)?;
                    order_book.remove_order(&order.order_id);
                }
            }
        }

        // Occasionally generate Trade messages (0x3D)
        if rng.gen_range(0..100) < 5 { // 5% chance
            let quantity = rng.gen_range(100..10000);
            let price = price_to_cboe_format(generate_realistic_price(symbol, &mut rng));
            let execution_id = order_book.next_execution_id();
            let order_id = format!("{:012}", to_base36(rng.gen_range(1..1000000)));
            let contra_order_id = format!("{:012}", to_base36(rng.gen_range(1..1000000)));
            let participant_id = participants.choose(&mut rng).unwrap().to_string();
            let contra_participant_id = participants.choose(&mut rng).unwrap().to_string();

            let mut record = create_empty_record();
            record.timestamp_ns = current_time_ns;
            record.message_type = "0x3D".to_string();
            record.port = port_num;
            record.symbol = format!("{:<6}", symbol);
            record.hdr_length = 16 + 72; // Header + message length
            record.hdr_count = 1;
            record.hdr_unit = unit;
            record.hdr_sequence = sequence_tracker.next_sequence(unit);
            record.msg_length = 72;
            record.quantity = Some(quantity);
            record.price = Some(price);
            record.execution_id = Some(execution_id);
            record.order_id = Some(order_id);
            record.contra_order_id = Some(contra_order_id);
            record.participant_id = Some(participant_id);
            record.contra_participant_id = Some(contra_participant_id);
            record.trade_type = Some('N'); // Normal matching logic
            record.trade_designation = Some('C'); // CXAC (Limit)
            record.trade_report_type = Some(' '); // On-exchange
            record.trade_transaction_time = Some(0);
            record.flags = Some(0);

            writer.serialize(&record)?;
            message_count += 1;
        }

        // Generate Order Executed at Price messages (0x58) for auction executions
        if rng.gen_range(0..200) < 1 { // 0.5% chance - auction execution
            if let Some(order) = order_book.get_random_order(&mut rng).cloned() {
                let executed_quantity = std::cmp::min(order.quantity, rng.gen_range(100..order.quantity + 1));
                let execution_id = order_book.next_execution_id();
                let auction_price = price_to_cboe_format(generate_realistic_price(symbol, &mut rng));

                let mut record = create_empty_record();
                record.timestamp_ns = current_time_ns;
                record.message_type = "0x58".to_string();
                record.port = port_num;
                record.symbol = format!("{:<6}", symbol);
                record.hdr_length = 16 + 52; // Header + message length (per spec)
                record.hdr_count = 1;
                record.hdr_unit = unit;
                record.hdr_sequence = sequence_tracker.next_sequence(unit);
                record.msg_length = 52; // Per CBOE spec
                record.order_id = Some(order.order_id.clone());
                record.executed_quantity = Some(executed_quantity);
                record.execution_id = Some(execution_id);
                record.price = Some(auction_price);

                writer.serialize(&record)?;
                message_count += 1;
                
                if executed_quantity >= order.quantity {
                    order_book.remove_order(&order.order_id);
                }
            }
        }

        // Occasionally generate Trade Break messages (0x3E)
        if rng.gen_range(0..5000) < 1 { // 0.02% chance
            // Only generate trade break if we have execution history
            if let Some(execution_id) = order_book.get_random_execution_id(&mut rng) {
                let break_reasons = ['E', 'C', 'D']; // Error, Consent, Duplicate
                let break_reason = *break_reasons.choose(&mut rng).unwrap();

                let mut record = create_empty_record();
                record.timestamp_ns = current_time_ns;
                record.message_type = "0x3E".to_string();
                record.port = port_num;
                record.symbol = format!("{:<6}", symbol);
                record.hdr_length = 16 + 18; // Header + message length (per spec)
                record.hdr_count = 1;
                record.hdr_unit = unit;
                record.hdr_sequence = sequence_tracker.next_sequence(unit);
                record.msg_length = 18; // Per CBOE spec
                record.execution_id = Some(execution_id); // Execution ID being broken
                record.break_reason = Some(break_reason);

                writer.serialize(&record)?;
                message_count += 1;
            }
        }

        // Occasionally generate intraday Calculated Value messages (0xE3)
        if rng.gen_range(0..2000) < 1 { // 0.05% chance - rare during trading
            let value = price_to_cboe_format(generate_realistic_price(symbol, &mut rng));

            let mut record = create_empty_record();
            record.timestamp_ns = current_time_ns;
            record.message_type = "0xE3".to_string();
            record.port = port_num;
            record.symbol = format!("{:<6}", symbol);
            record.hdr_length = 16 + 33;
            record.hdr_count = 1;
            record.hdr_unit = unit;
            record.hdr_sequence = sequence_tracker.next_sequence(unit);
            record.msg_length = 33;
            record.value_category = Some('2'); // Intraday value
            record.value = Some(value);
            record.value_timestamp = Some(current_time_ns);

            writer.serialize(&record)?;
            message_count += 1;
        }

        message_count += 1;
        current_time_ns += rng.gen_range(1_000..100_000);

        if message_count % 100_000 == 0 {
            info!("Generated {} messages", message_count);
        }
    }

    // PHASE 4: PRE-CLOSE AUCTION
    current_time_ns = continuous_end;
    
    // Pre-close auction updates
    for symbol in &symbols {
        let unit = (message_count % units as u64) as u8 + 1;
        let port_num = port + (unit - 1) as u16;
        let sequence = sequence_tracker.next_sequence(unit);
        
        // Auction Update (0x59) - Pre-close
        let mut record = create_empty_record();
        record.timestamp_ns = current_time_ns;
        record.message_type = "0x59".to_string();
        record.port = port_num;
        record.symbol = format!("{:<6}", symbol);
        record.hdr_length = 16 + 34;
        record.hdr_count = 1;
        record.hdr_unit = unit;
        record.hdr_sequence = sequence;
        record.msg_length = 34;
        record.auction_type = Some('C'); // Closing auction
        record.buy_shares = Some(rng.gen_range(1000..30000));
        record.sell_shares = Some(rng.gen_range(1000..30000));
        record.indicative_price = Some(price_to_cboe_format(generate_realistic_price(symbol, &mut rng)));
        
        writer.serialize(&record)?;
        message_count += 1;
        current_time_ns += 500_000; // 0.5ms
    }

    // Auction Summary for close
    for symbol in &symbols {
        let unit = (message_count % units as u64) as u8 + 1;
        let port_num = port + (unit - 1) as u16;
        let sequence = sequence_tracker.next_sequence(unit);
        
        let mut record = create_empty_record();
        record.timestamp_ns = current_time_ns;
        record.message_type = "0x5A".to_string();
        record.port = port_num;
        record.symbol = format!("{:<6}", symbol);
        record.hdr_length = 16 + 30;
        record.hdr_count = 1;
        record.hdr_unit = unit;
        record.hdr_sequence = sequence;
        record.msg_length = 30;
        record.auction_type = Some('C'); // Closing auction
        record.auction_price = Some(price_to_cboe_format(generate_realistic_price(symbol, &mut rng)));
        record.auction_shares = Some(rng.gen_range(3000..15000));
        
        writer.serialize(&record)?;
        message_count += 1;
        current_time_ns += 100_000;
    }

    // PHASE 5: MARKET CLOSE - Trading Status (Close)
    for symbol in &symbols {
        let unit = (message_count % units as u64) as u8 + 1;
        let port_num = port + (unit - 1) as u16;
        let sequence = sequence_tracker.next_sequence(unit);
        
        let mut record = create_empty_record();
        record.timestamp_ns = current_time_ns;
        record.message_type = "0x3B".to_string();
        record.port = port_num;
        record.symbol = format!("{:<6}", symbol);
        record.hdr_length = 16 + 22;
        record.hdr_count = 1;
        record.hdr_unit = unit;
        record.hdr_sequence = sequence;
        record.msg_length = 22;
        record.trading_status = Some('C'); // Closed
        record.market_id_code = Some(market_id_codes.choose(&mut rng).unwrap().to_string());
        
        writer.serialize(&record)?;
        message_count += 1;
        current_time_ns += 100_000;
    }

    // Generate Calculated Values (0xE3) - End of day values
    for symbol in &symbols {
        let unit = (message_count % units as u64) as u8 + 1;
        let port_num = port + (unit - 1) as u16;
        let sequence = sequence_tracker.next_sequence(unit);
        
        let mut record = create_empty_record();
        record.timestamp_ns = current_time_ns;
        record.message_type = "0xE3".to_string();
        record.port = port_num;
        record.symbol = format!("{:<6}", symbol);
        record.hdr_length = 16 + 33;
        record.hdr_count = 1;
        record.hdr_unit = unit;
        record.hdr_sequence = sequence;
        record.msg_length = 33;
        record.value_category = Some('1'); // Closing price
        record.value = Some(price_to_cboe_format(generate_realistic_price(symbol, &mut rng)));
        record.value_timestamp = Some(current_time_ns);
        
        writer.serialize(&record)?;
        message_count += 1;
        current_time_ns += 100_000;
    }

    // Generate Unit Clear messages (0x97) if needed for recovery
    if rng.gen_range(0..20) == 0 { // 5% chance - rare recovery event
        for unit_num in 1..=units {
            let mut record = create_empty_record();
            record.timestamp_ns = current_time_ns;
            record.message_type = "0x97".to_string();
            record.port = port + (unit_num - 1) as u16;
            record.symbol = format!("{:<6}", "      ");
            record.hdr_length = 16 + 6; // Header + message length (per spec)
            record.hdr_count = 1;
            record.hdr_unit = unit_num;
            record.hdr_sequence = sequence_tracker.next_sequence(unit_num);
            record.msg_length = 6; // Per CBOE spec

            writer.serialize(&record)?;
            message_count += 1;
            current_time_ns += 100_000;
        }
    }

    // PHASE 6: END OF SESSION - MANDATORY
    current_time_ns = end_time_ns;
    for symbol in &symbols {
        let unit = (message_count % units as u64) as u8 + 1;
        let port_num = port + (unit - 1) as u16;
        let sequence = sequence_tracker.next_sequence(unit);

        let mut record = create_empty_record();
        record.timestamp_ns = current_time_ns;
        record.message_type = "0x2D".to_string();
        record.port = port_num;
        record.symbol = format!("{:<6}", symbol);
        record.hdr_length = 16 + 6; // Header + message length (per spec)
        record.hdr_count = 1;
        record.hdr_unit = unit;
        record.hdr_sequence = sequence;
        record.msg_length = 6; // Per CBOE spec

        writer.serialize(&record)?;
        message_count += 1;
        current_time_ns += 1_000_000;
    }

    writer.flush()?;
    
    info!("Generated {} total messages", message_count);
    info!("Message types generated: Trading Status (0x3B), Add Order (0x37), Order Executed (0x38)");
    info!("                        Order Executed at Price (0x58), Reduce Size (0x39), Modify Order (0x3A)");
    info!("                        Delete Order (0x3C), Trade (0x3D), Trade Break (0x3E)");
    info!("                        Auction Update (0x59), Auction Summary (0x5A), Unit Clear (0x97)");
    info!("                        Calculated Value (0xE3), End of Session (0x2D) - MANDATORY");
    info!("CSV file written to: {} with End of Session messages", output);
    
    Ok(())
}