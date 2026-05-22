//! check_mongodb - Nagios plugin for MongoDB in Rust (async version)

use clap::{Arg, ArgAction, Command};
use mongodb::{
    Client,
    options::{ClientOptions, Credential},
    error::Error as MongoError,
};
use bson::{doc, Bson, Document};
use std::time::{Instant, Duration};
use std::process;

// Nagios exit codes
const OK: i32 = 0;
const WARNING: i32 = 1;
const CRITICAL: i32 = 2;
const UNKNOWN: i32 = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Action {
    Connect,
    Connections,
    ReplsetState,
    Memory,
    Databases,
    Collections,
    DatabaseSize,
    CollectionState,
    RowCount,
    Other,
}

impl std::str::FromStr for Action {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "connect" => Ok(Action::Connect),
            "connections" => Ok(Action::Connections),
            "replset_state" => Ok(Action::ReplsetState),
            "memory" => Ok(Action::Memory),
            "databases" => Ok(Action::Databases),
            "collections" => Ok(Action::Collections),
            "database_size" => Ok(Action::DatabaseSize),
            "collection_state" => Ok(Action::CollectionState),
            "row_count" => Ok(Action::RowCount),
            _ => Ok(Action::Other),
        }
    }
}

#[derive(Debug, Clone)]
struct Config {
    host: String,
    port: u16,
    user: Option<String>,
    passwd: Option<String>,
    warning: Option<f64>,
    critical: Option<f64>,
    action: Action,
    perf_data: bool,
    database: String,
    collection: String,
    timeout: u64,
}

#[tokio::main]
async fn main() {
    let matches = Command::new("check_mongodb")
        .version("1.0.0")
        .author("Rust rewrite of mzupan/nagios-plugin-mongodb")
        .about("Nagios plugin to check MongoDB health")
        .arg(Arg::new("host").short('H').long("host")
            .help("The hostname to connect to").default_value("127.0.0.1"))
        .arg(Arg::new("port").short('P').long("port")
            .help("The port mongodb is running on").default_value("27017"))
        .arg(Arg::new("user").short('u').long("user")
            .help("The username to login as"))
        .arg(Arg::new("pass").short('p').long("pass")
            .help("The password for the user"))
        .arg(Arg::new("warning").short('W').long("warning")
            .help("The warning threshold"))
        .arg(Arg::new("critical").short('C').long("critical")
            .help("The critical threshold"))
        .arg(Arg::new("action").short('A').long("action")
            .help("The action to take").default_value("connect"))
        .arg(Arg::new("perf_data").short('D').long("perf-data")
            .help("Enable performance data output").action(ArgAction::SetTrue))
        .arg(Arg::new("database").short('d').long("database")
            .help("Specify the database to check").default_value("admin"))
        .arg(Arg::new("collection").short('c').long("collection")
            .help("Specify the collection to check").default_value("admin"))
        .arg(Arg::new("timeout").short('t').long("timeout")
            .help("Connection timeout in seconds").default_value("10"))
        .get_matches();

    let config = Config {
        host: matches.get_one::<String>("host").unwrap().clone(),
        port: matches.get_one::<String>("port").unwrap().parse().unwrap_or(27017),
        user: matches.get_one::<String>("user").cloned(),
        passwd: matches.get_one::<String>("pass").cloned(),
        warning: matches.get_one::<String>("warning").and_then(|s| s.parse().ok()),
        critical: matches.get_one::<String>("critical").and_then(|s| s.parse().ok()),
        action: matches.get_one::<String>("action").unwrap().parse().unwrap(),
        perf_data: matches.get_flag("perf_data"),
        database: matches.get_one::<String>("database").unwrap().clone(),
        collection: matches.get_one::<String>("collection").unwrap().clone(),
        timeout: matches.get_one::<String>("timeout").unwrap().parse().unwrap_or(10),
    };

    // Handle invalid/unimplemented actions before attempting connection
    if config.action == Action::Other {
        println!("WARNING - Action not yet implemented");
        process::exit(WARNING);
    }

    let start = Instant::now();
    let client = match connect_mongodb(&config).await {
        Ok(c) => c,
        Err(code) => process::exit(code),
    };
    let conn_time = start.elapsed();

    let result = match config.action {
        Action::Connect => check_connect(&config, conn_time),
        Action::Connections => check_connections(&client, &config).await,
        Action::ReplsetState => check_replset_state(&client, &config).await,
        Action::Memory => check_memory(&client, &config).await,
        Action::Databases => check_databases(&client, &config).await,
        Action::Collections => check_collections(&client, &config).await,
        Action::DatabaseSize => check_database_size(&client, &config.database, &config).await,
        Action::CollectionState => check_collection_state(&client, &config).await,
        Action::RowCount => check_row_count(&client, &config).await,
        Action::Other => {
            println!("WARNING - Action not yet implemented");
            WARNING
        }
    };

    process::exit(result);
}

async fn connect_mongodb(config: &Config) -> Result<Client, i32> {
    let conn_string = format!("mongodb://{}:{}", config.host, config.port);
    let mut opts = ClientOptions::parse(&conn_string).map_err(|e| {
        println!("CRITICAL - Failed to parse connection string: {}", e);
        CRITICAL
    })?;
    
    opts.server_selection_timeout = Some(Duration::from_secs(config.timeout));

    if let (Some(ref user), Some(ref pwd)) = (&config.user, &config.passwd) {
        let mut cred = Credential::default();
        cred.username = Some(user.clone());
        cred.password = Some(pwd.clone());
        cred.source = Some(config.database.clone());
        opts.credential = Some(cred);
    }

    let client = Client::with_options(opts).map_err(|e| {
        println!("CRITICAL - Connection to Mongo server on {}:{} has failed: {}", 
            config.host, config.port, e);
        CRITICAL
    })?;

    // Test connection with ping
    let db = client.database("admin");
    db.run_command(doc! {"ping": 1}, None).await.map_err(|e| {
        println!("CRITICAL - MongoDB ping failed: {}", e);
        CRITICAL
    })?;

    Ok(client)
}

async fn get_server_status(client: &Client) -> Result<Document, MongoError> {
    client.database("admin").run_command(doc! {"serverStatus": 1}, None).await
}

async fn get_server_info(client: &Client) -> Result<Document, MongoError> {
    let db = client.database("admin");
    match db.run_command(doc! {"hello": 1}, None).await {
        Ok(r) => Ok(r),
        Err(_) => match db.run_command(doc! {"ismaster": 1}, None).await {
            Ok(r) => Ok(r),
            Err(_) => db.run_command(doc! {"isMaster": 1}, None).await,
        },
    }
}

fn check_connect(config: &Config, conn_time: Duration) -> i32 {
    let warn = config.warning.unwrap_or(3.0);
    let crit = config.critical.unwrap_or(6.0);
    let secs = conn_time.as_secs_f64();
    
    let msg = format!("Connection took {:.3} seconds", secs);
    let perf = if config.perf_data {
        format!(" |connection_time={:.3};{};{}", secs, warn, crit)
    } else {
        String::new()
    };
    
    if secs >= crit {
        println!("CRITICAL - {}{}", msg, perf);
        CRITICAL
    } else if secs >= warn {
        println!("WARNING - {}{}", msg, perf);
        WARNING
    } else {
        println!("OK - {}{}", msg, perf);
        OK
    }
}

async fn check_connections(client: &Client, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(80.0);
    let crit = config.critical.unwrap_or(95.0);
    
    match get_server_status(client).await {
        Ok(status) => {
            if let Some(conns) = status.get("connections").and_then(|c| c.as_document()) {
                let current = conns.get("current").and_then(|c| c.as_i64()).unwrap_or(0) as f64;
                let available = conns.get("available").and_then(|a| a.as_i64()).unwrap_or(0) as f64;
                let total = current + available;
                let pct = if total > 0.0 { (current / total) * 100.0 } else { 0.0 };
                
                let msg = format!("{} percent ({} of {} connections) used", 
                    pct.round() as i64, current as i64, total as i64);
                let perf = if config.perf_data {
                    format!(" |used_percent={:.0};{};{} current_connections={} available_connections={}",
                        pct, warn, crit, current as i64, available as i64)
                } else {
                    String::new()
                };
                
                if pct >= crit {
                    println!("CRITICAL - {}{}", msg, perf);
                    CRITICAL
                } else if pct >= warn {
                    println!("WARNING - {}{}", msg, perf);
                    WARNING
                } else {
                    println!("OK - {}{}", msg, perf);
                    OK
                }
            } else {
                println!("CRITICAL - Could not get connections data");
                CRITICAL
            }
        }
        Err(e) => {
            println!("CRITICAL - General MongoDB Error: {}", e);
            CRITICAL
        }
    }
}

async fn check_replset_state(client: &Client, config: &Config) -> i32 {
    match client.database("admin").run_command(doc! {"replSetGetStatus": 1}, None).await {
        Ok(status) => {
            let my_state = status.get("myState").and_then(|s| s.as_i64()).unwrap_or(-2);
            let members = status.get("members").and_then(|m| m.as_array()).cloned().unwrap_or_default();
            
            let mut worst_state = my_state;
            let mut msg = String::new();
            
            for m in &members {
                if let Some(mdoc) = m.as_document() {
                    let state = mdoc.get("state").and_then(|s| s.as_i64()).unwrap_or(-1);
                    let name = mdoc.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                    msg.push_str(&format!(" {}: {} ({})", name, state, state_text(state)));
                    
                    if state > worst_state {
                        worst_state = state;
                    }
                }
            }
            
            let perf = if config.perf_data {
                format!(" |state={}", my_state)
            } else {
                String::new()
            };
            
            let warn = config.warning.unwrap_or(3.0);
            let crit = config.critical.unwrap_or(8.0);
            
            if worst_state as f64 >= crit {
                println!("CRITICAL - {}{}", msg, perf);
                CRITICAL
            } else if worst_state as f64 >= warn {
                println!("WARNING - {}{}", msg, perf);
                WARNING
            } else {
                println!("OK - {}{}", msg, perf);
                OK
            }
        }
        Err(e) => {
            if e.to_string().to_lowercase().contains("not running with --replset") {
                println!("UNKNOWN - Not running with replSet");
                UNKNOWN
            } else {
                println!("CRITICAL - General MongoDB Error: {}", e);
                CRITICAL
            }
        }
    }
}

async fn check_memory(client: &Client, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(8000.0);  // MB
    let crit = config.critical.unwrap_or(9000.0);  // MB
    
    match get_server_status(client).await {
        Ok(status) => {
            if let Some(mem) = status.get("mem").and_then(|m| m.as_document()) {
                let resident = mem.get("resident").and_then(|v| match v {
                    Bson::Double(f) => Some(*f / 1024.0 / 1024.0), // Convert to GB
                    Bson::Int64(i) => Some(*i as f64 / 1024.0 / 1024.0),
                    _ => None,
                }).unwrap_or(0.0);
                
                let msg = format!("Memory Usage: {:.2}GB resident", resident);
                let perf = if config.perf_data {
                    format!(" |memory_usage={:.2};{};{}", resident, warn, crit)
                } else {
                    String::new()
                };
                
                if resident >= crit {
                    println!("CRITICAL - {}{}", msg, perf);
                    CRITICAL
                } else if resident >= warn {
                    println!("WARNING - {}{}", msg, perf);
                    WARNING
                } else {
                    println!("OK - {}{}", msg, perf);
                    OK
                }
            } else {
                println!("CRITICAL - Could not get memory data");
                CRITICAL
            }
        }
        Err(e) => {
            println!("CRITICAL - General MongoDB Error: {}", e);
            CRITICAL
        }
    }
}

async fn check_databases(client: &Client, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(100.0);
    let crit = config.critical.unwrap_or(500.0);
    
    match client.database("admin").run_command(doc! {"listDatabases": 1}, None).await {
        Ok(result) => {
            if let Some(Bson::Array(dbs)) = result.get("databases") {
                let count = dbs.len() as f64;
                let msg = format!("Number of DBs: {}", count as i64);
                let perf = if config.perf_data {
                    format!(" |databases={};{};{}", count, warn, crit)
                } else {
                    String::new()
                };
                
                if count >= crit {
                    println!("CRITICAL - {}{}", msg, perf);
                    CRITICAL
                } else if count >= warn {
                    println!("WARNING - {}{}", msg, perf);
                    WARNING
                } else {
                    println!("OK - {}{}", msg, perf);
                    OK
                }
            } else {
                println!("CRITICAL - Could not get databases list");
                CRITICAL
            }
        }
        Err(e) => {
            println!("CRITICAL - General MongoDB Error: {}", e);
            CRITICAL
        }
    }
}

async fn check_collections(client: &Client, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(100.0);
    let crit = config.critical.unwrap_or(500.0);
    
    match client.database("admin").run_command(doc! {"listDatabases": 1}, None).await {
        Ok(result) => {
            if let Some(Bson::Array(dbs)) = result.get("databases") {
                let mut count = 0u64;
                for db_info in dbs {
                    if let Some(db_doc) = db_info.as_document() {
                        if let Some(db_name) = db_doc.get("name").and_then(|n| n.as_str()) {
                            if !matches!(db_name, "admin" | "local" | "config") {
                                if let Ok(names) = client.database(db_name).list_collection_names(None).await {
                                    count += names.len() as u64;
                                }
                            }
                        }
                    }
                }
                let count_f64 = count as f64;
                let msg = format!("Number of collections: {}", count);
                let perf = if config.perf_data {
                    format!(" |collections={};{};{}", count, warn, crit)
                } else {
                    String::new()
                };
                
                if count_f64 >= crit {
                    println!("CRITICAL - {}{}", msg, perf);
                    CRITICAL
                } else if count_f64 >= warn {
                    println!("WARNING - {}{}", msg, perf);
                    WARNING
                } else {
                    println!("OK - {}{}", msg, perf);
                    OK
                }
            } else {
                println!("CRITICAL - Could not get databases list");
                CRITICAL
            }
        }
        Err(e) => {
            println!("CRITICAL - General MongoDB Error: {}", e);
            CRITICAL
        }
    }
}

async fn check_database_size(client: &Client, database: &str, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(1000.0);
    let crit = config.critical.unwrap_or(10000.0);
    
    match client.database(database).run_command(doc! {"dbstats": 1}, None).await {
        Ok(stats) => {
            let size = match stats.get("storageSize") {
                Some(Bson::Int64(i)) => *i as f64,
                Some(Bson::Int32(i)) => *i as f64,
                Some(Bson::Double(f)) => *f,
                _ => {
                    println!("CRITICAL - storageSize has unexpected type");
                    return CRITICAL;
                }
            };
            let size_mb = size / 1024.0 / 1024.0;
            let msg = format!("Database size: {:.0} MB, Database: {}", size_mb, database);
            let perf = if config.perf_data {
                format!(" |database_size={:.0};{};{}", size_mb, warn, crit)
            } else {
                String::new()
            };
            
            if size_mb >= crit {
                println!("CRITICAL - {}{}", msg, perf);
                CRITICAL
            } else if size_mb >= warn {
                println!("WARNING - {}{}", msg, perf);
                WARNING
            } else {
                println!("OK - {}{}", msg, perf);
                OK
            }
        }
        Err(e) => {
            println!("CRITICAL - General MongoDB Error: {}", e);
            CRITICAL
        }
    }
}

async fn check_collection_state(client: &Client, config: &Config) -> i32 {
    match client.database(&config.database)
        .collection::<Document>(&config.collection)
        .find_one(None, None).await
    {
        Ok(_) => {
            println!("OK - Collection {}.{} is reachable", config.database, config.collection);
            OK
        }
        Err(e) => {
            println!("CRITICAL - Collection {}.{} is not reachable: {}", 
                config.database, config.collection, e);
            CRITICAL
        }
    }
}

async fn check_row_count(client: &Client, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(1000.0);
    let crit = config.critical.unwrap_or(10000.0);
    
    match client.database(&config.database)
        .collection::<Document>(&config.collection)
        .estimated_document_count(None).await
    {
        Ok(count) => {
            let count_f64 = count as f64;
            let msg = format!("Row count: {}", count);
            let perf = if config.perf_data {
                format!(" |row_count={};{};{}", count, warn, crit)
            } else {
                String::new()
            };
            
            if count_f64 >= crit {
                println!("CRITICAL - {}{}", msg, perf);
                CRITICAL
            } else if count_f64 >= warn {
                println!("WARNING - {}{}", msg, perf);
                WARNING
            } else {
                println!("OK - {}{}", msg, perf);
                OK
            }
        }
        Err(e) => {
            println!("CRITICAL - General MongoDB Error: {}", e);
            CRITICAL
        }
    }
}

fn state_text(state: i64) -> &'static str {
    match state {
        0 => "Starting up, phase1",
        1 => "Primary",
        2 => "Secondary",
        3 => "Recovering",
        4 => "Fatal error",
        5 => "Starting up, phase2",
        7 => "Arbiter",
        8 => "Down",
        _ => "Unknown",
    }
}
