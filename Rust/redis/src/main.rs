use std::{
    cmp::Ordering,
    collections::HashMap,
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    ops::Add,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{
    fs::File,
    io::{AsyncReadExt, BufReader},
};
const ASCII_UPPER_CASE_A: u8 = 65;
const ASCII_LOWER_CASE_Z: u8 = 122;
const PONG: &str = "+PONG\r\n";
const NULL: &str = "$-1\r\n";

struct Entry {
    value: String,
    expire: Option<SystemTime>,
}

enum Command {
    Ping,
    Echo,
    Set,
    Get,
    Config,
    Keys,
    Unknown,
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                tokio::spawn(async move {
                    let mut database = HashMap::new();
                    populate_database(&mut database).await;

                    loop {
                        let lines = get_lines(stream.try_clone().expect("failed to clone steam"));

                        if lines.len() == 1 && lines[0].is_empty() {
                            continue;
                        }
                        if lines.is_empty() {
                            break;
                        }
                        let message = match get_command(&lines) {
                            Command::Ping => handle_ping(),
                            Command::Echo => handle_echo(&lines),
                            Command::Set => handle_set(&lines, &mut database),
                            Command::Get => handle_get(&lines, &mut database),
                            Command::Config => handle_config(&lines),
                            Command::Keys => handle_keys(&lines).await,
                            Command::Unknown => {
                                println!("{lines:?}");
                                panic!("What should we do?")
                            }
                        };
                        let _ = stream.write_all(message.as_bytes());
                    }
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

async fn populate_database(database: &mut HashMap<String, Entry>) {
    let rdb = read_rdb().await;

    for (key, entry) in rdb {
        database.insert(key, entry);
    }
}

fn get_lines(mut stream: TcpStream) -> Vec<String> {
    let mut buf = [0; 4096];
    let len = stream.read(&mut buf).unwrap();
    let req = std::str::from_utf8(&buf[0..len]).unwrap().to_string();
    req.replace("\r\n", " ")
        .split(" ")
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .collect()
}

fn get_command(lines: &Vec<String>) -> Command {
    let len = lines.len();
    if len == 3 && lines[2] == "ping" {
        Command::Ping
    } else if len == 5 && lines[2] == "echo" {
        Command::Echo
    } else if (len == 7 || len == 11) && lines[2] == "set" {
        Command::Set
    } else if len == 5 && lines[2] == "get" {
        Command::Get
    } else if len == 7 && lines[2] == "config" {
        Command::Config
    } else if len == 5 && lines[2] == "keys" {
        Command::Keys
    } else {
        Command::Unknown
    }
}

fn handle_ping() -> String {
    PONG.to_string()
}

fn handle_echo(lines: &Vec<String>) -> String {
    format!(":{}\r\n", { &lines[4] })
}

fn handle_set(lines: &Vec<String>, database: &mut HashMap<String, Entry>) -> String {
    let len = lines.len();
    let key = lines[4].to_string();
    let value = lines[6].to_string();

    let expire = if len == 7 {
        None
    } else {
        let now = SystemTime::now();
        let milliseconds = &lines[10].parse::<u64>().expect("a number");
        Some(now.add(Duration::from_millis(*milliseconds)))
    };

    database.insert(key, Entry { value, expire });
    format!(":OK\r\n")
}

fn handle_get(lines: &Vec<String>, database: &mut HashMap<String, Entry>) -> String {
    let key = lines[4].to_string();
    match database.get(&key) {
        Some(entry) => {
            let value = &entry.value;
            let message = match &entry.expire {
                Some(expire) => {
                    let now = SystemTime::now();

                    if now.cmp(expire) == Ordering::Less {
                        format!(":{value}\r\n")
                    } else {
                        NULL.to_string()
                    }
                }
                None => format!(":{value}\r\n"),
            };
            message
        }
        None => NULL.to_string(),
    }
}

fn get_arg(key: &str) -> Option<String> {
    let args: Vec<String> = env::args().collect();
    let key_index = args.iter().position(|s| *s == format!("--{key}"));
    match key_index {
        Some(index) => Some(args[index + 1].clone()),
        None => None,
    }
}

fn handle_config(lines: &Vec<String>) -> String {
    let key = &lines[6];

    let arg = get_arg(key).unwrap_or_default();
    let key_length = key.len();
    let arg_length = arg.len();

    format!("*2\r\n${key_length}\r\n{key}\r\n${arg_length}\r\n{arg}\r\n")
}

async fn parse_rdb(file: File) -> (Vec<char>, Vec<String>) {
    let mut my_buf = BufReader::new(file);
    let mut chars = Vec::new();
    let mut expires = Vec::new();

    while let Ok(first) = my_buf.read_u8().await {
        //FC $unsigned long # "expiry time in ms", followed by 8 byte unsigned long
        if first == 252 {
            let mut current_expiry = String::new();
            for _ in 0..8 {
                match my_buf.read_u8().await {
                    Ok(number) => current_expiry += &number.to_string(),
                    Err(_) => panic!("Expected a number"),
                };
            }
            expires.push(current_expiry.clone());
            continue;
        }

        let is_letter = first >= ASCII_UPPER_CASE_A && first <= ASCII_LOWER_CASE_Z;
        if is_letter {
            chars.push(first as char);
        } else {
            chars.push(' ');
        }
        if first == 255 {
            break;
        }
    }
    (chars, expires)
}
fn generate_entries(chars: Vec<char>, expires: Vec<String>) -> HashMap<String, Entry> {
    let key_values: Vec<String> = chars
        .iter()
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let mut entries = HashMap::new();

    for (index, entry) in key_values[5..].chunks(2).enumerate() {
        if entry.len() == 2 {
            let expire = if index < expires.len() {
                let timestamp = expires[index].parse().expect("timestamp");
                let duration = std::time::Duration::from_millis(timestamp);

                let entry_time = UNIX_EPOCH + duration;
                let now = SystemTime::now();
                if now.cmp(&entry_time) == Ordering::Less {
                    Some(UNIX_EPOCH)
                } else {
                    None
                }
            } else {
                None
            };
            entries.insert(
                entry[0].clone(),
                Entry {
                    value: entry[1].clone(),
                    expire,
                },
            );
        }
    }
    entries
}

async fn read_rdb() -> HashMap<String, Entry> {
    let dir = get_arg("dir");
    let dbfilename = get_arg("dbfilename");
    if dir.is_none() || dbfilename.is_none() {
        return HashMap::new();
    }
    let dir = dir.expect("");
    let dbfilename = dbfilename.expect("");
    let path = format!("{dir}/{dbfilename}");
    let file = File::open(path).await;
    if file.is_err() {
        return HashMap::new();
    }
    let file = file.expect("");

    let (chars, expires) = parse_rdb(file).await;
    generate_entries(chars, expires)
}

async fn handle_keys(_lines: &Vec<String>) -> String {
    let rdb = read_rdb().await;
    let keys = rdb.keys();
    let keys_length = keys.len();
    let mut message = format!("*{keys_length}\r\n");

    for key in keys {
        let key_length = key.len();
        message += format!("${key_length}\r\n{key}\r\n").as_str();
    }

    message
}
