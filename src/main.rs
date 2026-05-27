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
    ReplsetQuorum,
    Memory,
    MemoryMapped,
    Lock,
    Flushing,
    LastFlushTime,
    IndexMissRatio,
    Databases,
    Collections,
    DatabaseSize,
    DatabaseIndexes,
    CollectionDocuments,
    CollectionIndexes,
    CollectionSize,
    CollectionStorageSize,
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
            "replset_quorum" => Ok(Action::ReplsetQuorum),
            "memory" => Ok(Action::Memory),
            "memory_mapped" => Ok(Action::MemoryMapped),
            "lock" => Ok(Action::Lock),
            "flushing" => Ok(Action::Flushing),
            "last_flush_time" => Ok(Action::LastFlushTime),
            "index_miss_ratio" => Ok(Action::IndexMissRatio),
            "databases" => Ok(Action::Databases),
            "collections" => Ok(Action::Collections),
            "database_size" => Ok(Action::DatabaseSize),
            "database_indexes" => Ok(Action::DatabaseIndexes),
            "collection_documents" => Ok(Action::CollectionDocuments),
            "collection_indexes" => Ok(Action::CollectionIndexes),
            "collection_size" => Ok(Action::CollectionSize),
            "collection_storageSize" => Ok(Action::CollectionStorageSize),
            "collection_storagesize" => Ok(Action::CollectionStorageSize),
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
        .after_help("\nAvailable actions:\n  connect              - Check connection to MongoDB\n  connections         - Check connection usage\n  replset_state       - Check replica set state\n  replset_quorum      - Check replica set quorum\n  memory              - Check memory usage\n  memory_mapped       - Check mapped memory usage\n  lock                - Check global lock percentage\n  flushing            - Check average flush time\n  last_flush_time     - Check last flush time\n  index_miss_ratio    - Check index miss ratio\n  databases           - Count databases\n  collections         - Count collections\n  database_size       - Check database size\n  database_indexes    - Check database index size\n  collection_documents - Count documents in collection\n  collection_indexes  - Check collection index size\n  collection_size      - Check collection size\n  collection_storageSize - Check collection storage size\n  collection_state    - Check if collection is reachable\n  row_count           - Count rows in a collection")
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
            .help("The action to take")
            .default_value("connect"))
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
        Action::ReplsetQuorum => check_replset_quorum(&client, &config).await,
        Action::Memory => check_memory(&client, &config).await,
        Action::MemoryMapped => check_memory_mapped(&client, &config).await,
        Action::Lock => check_lock(&client, &config).await,
        Action::Flushing => check_flushing(&client, &config, true).await,
        Action::LastFlushTime => check_flushing(&client, &config, false).await,
        Action::IndexMissRatio => check_index_miss_ratio(&client, &config).await,
        Action::Databases => check_databases(&client, &config).await,
        Action::Collections => check_collections(&client, &config).await,
        Action::DatabaseSize => check_database_size(&client, &config.database, &config).await,
        Action::DatabaseIndexes => check_database_indexes(&client, &config.database, &config).await,
        Action::CollectionDocuments => check_collection_documents(&client, &config.database, &config.collection, &config).await,
        Action::CollectionIndexes => check_collection_indexes(&client, &config.database, &config.collection, &config).await,
        Action::CollectionSize => check_collection_size(&client, &config.database, &config.collection, &config).await,
        Action::CollectionStorageSize => check_collection_storage_size(&client, &config.database, &config.collection, &config).await,
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
                let current = conns.get("current").and_then(|c| match c {
                    Bson::Int32(i) => Some(*i as i64),
                    Bson::Int64(i) => Some(*i),
                    _ => None,
                }).unwrap_or(0) as f64;
                let available = conns.get("available").and_then(|a| match a {
                    Bson::Int32(i) => Some(*i as i64),
                    Bson::Int64(i) => Some(*i),
                    _ => None,
                }).unwrap_or(0) as f64;
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
                let get_mem_value = |field: &str| -> f64 {
                    mem.get(field).and_then(|v| match v {
                        Bson::Double(f) => Some(*f),
                        Bson::Int64(i) => Some(*i as f64),
                        Bson::Int32(i) => Some(*i as f64),
                        _ => None,
                    }).unwrap_or(0.0)
                };
                
                let resident = get_mem_value("resident") / 1024.0;
                let r#virtual = get_mem_value("virtual") / 1024.0;
                let mapped = get_mem_value("mapped");
                let mapped_with_journal = get_mem_value("mappedWithJournal");
                
                let mut msg = "Memory Usage:".to_string();
                msg.push_str(&format!(" {:.2}GB resident,", resident));
                msg.push_str(&format!(" {:.2}GB virtual,", r#virtual));
                
                if mapped > 0.0 {
                    msg.push_str(&format!(" {:.2}GB mapped,", mapped / 1024.0));
                } else {
                    msg.push_str(" mapped unsupported,");
                }
                
                if mapped_with_journal > 0.0 {
                    msg.push_str(&format!(" {:.2}GB mappedWithJournal", mapped_with_journal / 1024.0));
                }
                
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

async fn check_replset_quorum(client: &Client, config: &Config) -> i32 {
    let warning = 1.0;
    let critical = 2.0;
    let db = client.database("admin");

    match db.run_command(doc! {"replSetGetStatus": 1}, None).await {
        Ok(status) => {
            let mut primary_count = 0u64;
            if let Some(Bson::Array(members)) = status.get("members") {
                for m in members {
                    if let Some(mdoc) = m.as_document() {
                        if let Some(Bson::Int32(1)) = mdoc.get("state") {
                            primary_count += 1;
                        }
                    }
                }
            }

            let state = if primary_count == 1 { 0 } else { 2 };
            let msg = if state == 0 {
                "Cluster is quorate"
            } else {
                "Cluster is not quorate and cannot operate"
            };
            let perf = if config.perf_data {
                format!(" |state={}", state)
            } else {
                String::new()
            };

            if state as f64 >= critical {
                println!("CRITICAL - {}{}", msg, perf);
                CRITICAL
            } else if state as f64 >= warning {
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

async fn check_memory_mapped(client: &Client, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(8000.0);
    let crit = config.critical.unwrap_or(16000.0);

    match get_server_status(client).await {
        Ok(status) => {
            if let Some(mem) = status.get("mem").and_then(|m| m.as_document()) {
                let mapped = mem.get("mapped");
                
                if mapped.is_none() {
                    println!("OK - Server does not provide mem.mapped info");
                    return OK;
                }
                
                let mapped_gb = match mapped.unwrap() {
                    Bson::Double(f) => *f / 1024.0 / 1024.0,
                    Bson::Int64(i) => *i as f64 / 1024.0 / 1024.0,
                    Bson::Int32(i) => *i as f64 / 1024.0 / 1024.0,
                    _ => 0.0,
                };

                let msg = format!("Memory Usage: {:.2}GB mapped", mapped_gb);
                let perf = if config.perf_data {
                    format!(" |memory_mapped={:.2};{};{}", mapped_gb, warn, crit)
                } else {
                    String::new()
                };

                if mapped_gb >= crit {
                    println!("CRITICAL - {}{}", msg, perf);
                    CRITICAL
                } else if mapped_gb >= warn {
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

async fn check_lock(client: &Client, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(10.0);
    let crit = config.critical.unwrap_or(30.0);

    match get_server_status(client).await {
        Ok(status) => {
            if let Some(global_lock) = status.get("globalLock").and_then(|g| g.as_document()) {
                let lock_time = global_lock.get("lockTime").and_then(|l| l.as_i64()).unwrap_or(0) as f64;
                let total_time = global_lock.get("totalTime").and_then(|t| t.as_i64()).unwrap_or(1) as f64;
                
                // In MongoDB 7.0+, lockTime field may not exist - only totalTime, currentQueue, activeClients
                if lock_time > 0.0 && total_time > 0.0 {
                    let pct = (lock_time / total_time) * 100.0;
                    let msg = format!("Lock Percentage: {:.2}%", pct);
                    let perf = if config.perf_data {
                        format!(" |lock_percentage={:.2};{};{}", pct, warn, crit)
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
                    // lockTime is 0 or missing - no lock data available in this MongoDB version
                    println!("OK - No lock data available for this MongoDB version");
                    OK
                }
            } else {
                println!("OK - MongoDB version 3+ doesn't report on global locks");
                OK
            }
        }
        Err(e) => {
            println!("CRITICAL - General MongoDB Error: {}", e);
            CRITICAL
        }
    }
}

async fn check_flushing(client: &Client, config: &Config, avg: bool) -> i32 {
    let warn = config.warning.unwrap_or(5000.0);
    let crit = config.critical.unwrap_or(15000.0);

    match get_server_status(client).await {
        Ok(status) => {
            if let Some(background_flushing) = status.get("backgroundFlushing").and_then(|b| b.as_document()) {
                let flush_time = if avg {
                    background_flushing.get("average_ms").and_then(|f| f.as_f64()).unwrap_or(0.0)
                } else {
                    background_flushing.get("last_ms").and_then(|f| f.as_f64()).unwrap_or(0.0)
                };
                let stat_type = if avg { "Average" } else { "Last" };

                let msg = format!("{} Flush Time: {:.2}ms", stat_type, flush_time);
                let perf = if config.perf_data {
                    format!(" |{}_flush_time={:.2};{};{}", stat_type.to_lowercase(), flush_time, warn, crit)
                } else {
                    String::new()
                };

                if flush_time >= crit {
                    println!("CRITICAL - {}{}", msg, perf);
                    CRITICAL
                } else if flush_time >= warn {
                    println!("WARNING - {}{}", msg, perf);
                    WARNING
                } else {
                    println!("OK - {}{}", msg, perf);
                    OK
                }
            } else {
                println!("OK - flushing stats not available for this storage engine");
                OK
            }
        }
        Err(e) => {
            println!("CRITICAL - General MongoDB Error: {}", e);
            CRITICAL
        }
    }
}

async fn check_index_miss_ratio(client: &Client, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(10.0);
    let crit = config.critical.unwrap_or(30.0);

    match get_server_status(client).await {
        Ok(status) => {
            let ic_field = status.get("indexCounters");
            
            // Check if indexCounters has a note about not being supported
            if let Some(Bson::Document(ic_doc)) = ic_field {
                if let Some(Bson::String(note)) = ic_doc.get("note") {
                    if note.contains("not supported on this platform") {
                        println!("OK - MongoDB says: not supported on this platform");
                        return OK;
                    }
                }
                // If we have indexCounters but no note, try to get missRatio
                let miss_ratio = ic_doc.get("missRatio").and_then(|m| m.as_f64()).unwrap_or(0.0);
                let msg = format!("Miss Ratio: {:.2}%", miss_ratio);
                let perf = if config.perf_data {
                    format!(" |index_miss_ratio={:.2};{};{}", miss_ratio, warn, crit)
                } else {
                    String::new()
                };

                if miss_ratio >= crit {
                    println!("CRITICAL - {}{}", msg, perf);
                    CRITICAL
                } else if miss_ratio >= warn {
                    println!("WARNING - {}{}", msg, perf);
                    WARNING
                } else {
                    println!("OK - {}{}", msg, perf);
                    OK
                }
            } else {
                println!("OK - MongoDB says: not supported on this platform");
                OK
            }
        }
        Err(e) => {
            println!("CRITICAL - General MongoDB Error: {}", e);
            CRITICAL
        }
    }
}

async fn check_database_indexes(client: &Client, database: &str, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(100.0);
    let crit = config.critical.unwrap_or(1000.0);

    match client.database(database).run_command(doc! {"dbstats": 1}, None).await {
        Ok(stats) => {
            let index_size = match stats.get("indexSize") {
                Some(Bson::Int64(i)) => *i as f64,
                Some(Bson::Int32(i)) => *i as f64,
                Some(Bson::Double(f)) => *f,
                _ => 0.0,
            };
            let index_size_mb = index_size / 1024.0 / 1024.0;

            let msg = format!("{} indexSize: {:.0} MB", database, index_size_mb);
            let perf = if config.perf_data {
                format!(" |database_indexes={:.0};{};{}", index_size_mb, warn, crit)
            } else {
                String::new()
            };

            if index_size_mb >= crit {
                println!("CRITICAL - {}{}", msg, perf);
                CRITICAL
            } else if index_size_mb >= warn {
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

async fn check_collection_documents(client: &Client, database: &str, collection: &str, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(100.0);
    let crit = config.critical.unwrap_or(1000.0);

    match client.database(database)
        .collection::<Document>(collection)
        .estimated_document_count(None).await
    {
        Ok(count) => {
            let count_f64 = count as f64;
            let msg = format!("{}.{} documents: {}", database, collection, count);
            let perf = if config.perf_data {
                format!(" |collection_documents={};{};{}", count, warn, crit)
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

async fn check_collection_indexes(client: &Client, database: &str, collection: &str, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(100.0);
    let crit = config.critical.unwrap_or(1000.0);

    match client.database(database)
        .run_command(doc! {"collstats": collection}, None).await
    {
        Ok(stats) => {
            let total_index_size = match stats.get("totalIndexSize") {
                Some(Bson::Int64(i)) => *i as f64,
                Some(Bson::Int32(i)) => *i as f64,
                Some(Bson::Double(f)) => *f,
                _ => 0.0,
            };
            let size_mb = total_index_size / 1024.0 / 1024.0;

            let msg = format!("{}.{} totalIndexSize: {:.0} MB", database, collection, size_mb);
            let perf = if config.perf_data {
                format!(" |collection_indexes={:.0};{};{}", size_mb, warn, crit)
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

async fn check_collection_size(client: &Client, database: &str, collection: &str, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(100.0);
    let crit = config.critical.unwrap_or(1000.0);

    match client.database(database)
        .run_command(doc! {"collstats": collection}, None).await
    {
        Ok(stats) => {
            let size = match stats.get("size") {
                Some(Bson::Int64(i)) => *i as f64,
                Some(Bson::Int32(i)) => *i as f64,
                Some(Bson::Double(f)) => *f,
                _ => 0.0,
            };
            let size_mb = size / 1024.0 / 1024.0;

            let msg = format!("{}.{} size: {:.0} MB", database, collection, size_mb);
            let perf = if config.perf_data {
                format!(" |collection_size={:.0};{};{}", size_mb, warn, crit)
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

async fn check_collection_storage_size(client: &Client, database: &str, collection: &str, config: &Config) -> i32 {
    let warn = config.warning.unwrap_or(100.0);
    let crit = config.critical.unwrap_or(1000.0);

    match client.database(database)
        .run_command(doc! {"collstats": collection}, None).await
    {
        Ok(stats) => {
            let storage_size = match stats.get("storageSize") {
                Some(Bson::Int64(i)) => *i as f64,
                Some(Bson::Int32(i)) => *i as f64,
                Some(Bson::Double(f)) => *f,
                _ => 0.0,
            };
            let size_mb = storage_size / 1024.0 / 1024.0;

            let msg = format!("{}.{} storageSize: {:.0} MB", database, collection, size_mb);
            let perf = if config.perf_data {
                format!(" |collection_storageSize={:.0};{};{}", size_mb, warn, crit)
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
