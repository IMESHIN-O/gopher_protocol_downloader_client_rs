use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, io};
const PORT: &str = "70";
const INIT_LIST: &str = "\r\n";
const TIME_OUT: u64 = 1000;
const MAX_BYTE: u64 = 65536;
const BUFF_LEN: usize = 1024;
static mut DIRE_NUM: u64 = 0;
static mut BIN_NUM: u64 = 0;
static mut TXT_NUM: u64 = 0;
static mut MIN_SIZE_TXT: u64 = 0xffffffff;
static mut MAX_SIZE_TXT: u64 = 0;
static mut MIN_SIZE_BIN: u64 = 0xffffffff;
static mut MAX_SIZE_BIN: u64 = 0;
static mut MIN_SIZE_TXT_CONTENT: String = String::new();
static mut INVALID_NUM: u64 = 0;
enum FileType {
    Bin,
    Txt,
}
struct Server {
    host: String,
    port: String,
}

impl Server {
    fn new(host: String, port: String) -> Server {
        Server { host, port }
    }
    fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
    fn connect(&self) -> io::Result<TcpStream> {
        TcpStream::connect(self.address())
    }
}
/*
  Main function of this client app, this main function will add
  any non-visited directory to the current queue, and if there is
  direcory in queue, visit this direcory and repeat.
*/
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <host>", args[0]);
        return;
    }
    let server = Server::new(String::from(&args[1]), String::from(PORT));
    drop(args);
    let mut mark: HashSet<String> = HashSet::new();
    let mut que: VecDeque<String> = VecDeque::new();
    let mut rename: HashMap<String, u64> = HashMap::new();
    let mut bin_path_list: Vec<String> = Vec::new();
    let mut txt_path_list: Vec<String> = Vec::new();
    let mut external_list: Vec<String> = Vec::new();
    mark.insert(INIT_LIST.to_string());
    que.push_back(INIT_LIST.to_string());
    while !que.is_empty() {
        let s = que.pop_front().unwrap();
        if let Err(_) = client(
            &mut mark,
            &mut que,
            s,
            &mut rename,
            &mut bin_path_list,
            &mut txt_path_list,
            &mut external_list,
            &server,
        ) {
            panic!("Failed to download!");
        }
    }
    unsafe {
        println!("The number of directories is {}.", DIRE_NUM);
        println!("The number of text files is {}.", TXT_NUM);
        println!("The path of the text files are:");
        for path in txt_path_list {
            println!("{}", path);
        }
        println!("The number of binary files is {}.", BIN_NUM);
        for path in bin_path_list {
            println!("{}", path);
        }
        println!(
            "The Content of mimimum size text files is: {}",
            MIN_SIZE_TXT_CONTENT
        );
        println!("The minimum size of text files is {}.", MIN_SIZE_TXT);
        println!("The maximum size of text files is {}.", MAX_SIZE_TXT);
        println!("The minimum size of binary files is {}.", MIN_SIZE_BIN);
        println!("The maximum size of binary files is {}.", MAX_SIZE_BIN);
        println!("The number of invalid refereneces is {}.", INVALID_NUM);
        println!("The path of the external servers are:");
        for path in external_list {
            println!("{}", path);
        }
    }
}

/*
  Connect to the server and send the path of the directory to the server. Then read
  the reply and add the sub directory path to the queue. If there is text or binary
  file in this directory, download it.
*/
fn client(
    mark: &mut HashSet<String>,
    que: &mut VecDeque<String>,
    s: String,
    rename: &mut HashMap<String, u64>,
    bin_list: &mut Vec<String>,
    txt_list: &mut Vec<String>,
    ext_list: &mut Vec<String>,
    server: &Server,
) -> io::Result<()> {
    let stream = server.connect()?;
    print!("Connected to {}", server.address());
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = BufWriter::new(stream);
    let now = SystemTime::now();
    match now.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let total_seconds = duration.as_secs();
            let current_second = total_seconds % 60;
            let total_minutes = total_seconds / 60;
            let current_minute = total_minutes % 60;
            let total_hours = total_minutes / 60;
            let current_hour = total_hours % 24;
            println!(
                "Current time (UTC): {:02}:{:02}:{:02}",
                current_hour, current_minute, current_second
            );
        }
        Err(_) => println!("SystemTime before UNIX EPOCH!"),
    }
    writer.write_all(s.as_bytes())?;
    writer.flush()?;
    let mut response = String::new();
    while reader.read_line(&mut response)? > 0 {
        println!("{}", response);
        let ch = response.chars().next().unwrap();
        let parts: Vec<&str> = response.split_whitespace().collect();
        let part_num = parts.len();
        if part_num < 3 {
            continue;
        }
        let port = parts[part_num - 1].to_string();
        let host = parts[part_num - 2].to_string();
        //external resources found
        if ch == '1' && (port != server.port || host != server.host) {
            let is_alive: bool = test_conn(&host, &port);
            ext_list.push(format!(
                "Host: {}, Port: {}, Is alive? {}",
                host, port, is_alive
            ));
            response.clear();
            continue;
        }
        //if it is information line, continue
        else if port != server.port || host != server.host {
            response.clear();
            continue;
        }
        //get the path and name, and make the name shorter and remove illegal character for windows system file name
        let path = parts[part_num - 3].to_string();
        let name: String = parts[..part_num - 3].join(" ").chars().take(50).collect();
        let mut clean_name = name
            .replace("<", "_")
            .replace(">", "_")
            .replace(":", "_")
            .replace("\"", "_")
            .replace("/", "_")
            .replace("\\", "_")
            .replace("|", "_")
            .replace("?", "_")
            .replace("*", "_");
        //if there are some files have same name, rename it, let the name followed by the number of this name appear
        if rename.contains_key(&clean_name) {
            let count = rename[&clean_name];
            rename.insert(clean_name.clone(), count + 1);
        } else {
            rename.insert(clean_name.clone(), 1);
        }
        if rename[&clean_name] > 1 {
            clean_name = format!("{}{}", clean_name, rename[&clean_name]);
        }
        //match the first character, if it is '0'(text) or '9'(binary) download it, otherwise add the sub directory to the queue
        match ch {
            '0' => {
                unsafe {
                    TXT_NUM += 1;
                }
                match download(
                    format!("{}{}", path, INIT_LIST),
                    FileType::Txt,
                    clean_name,
                    &server,
                ) {
                    Ok(_) => {
                        txt_list.push(path);
                    }
                    _ => {
                        panic!("Failed to download!");
                    }
                }
                response.clear();
            }
            '1' => {
                unsafe {
                    DIRE_NUM += 1;
                }
                let path_req = format!("{}{}", path, INIT_LIST);
                if !mark.contains(&path_req) {
                    mark.insert(path_req);
                    que.push_back(format!("{}{}", path, INIT_LIST));
                }
                response.clear();
            }
            '9' => {
                unsafe {
                    BIN_NUM += 1;
                }
                match download(
                    format!("{}{}", path, INIT_LIST),
                    FileType::Bin,
                    clean_name,
                    &server,
                ) {
                    Ok(_) => {
                        bin_list.push(path);
                    }
                    _ => {
                        panic!("Failed to download!");
                    }
                }
                response.clear();
            }
            _ => {
                response.clear();
            }
        }
    }
    Ok(())
}

/*
  download funtion, use different method for different files to download
*/
fn download(
    path: String,
    file_type: FileType,
    name: String,
    server: &Server,
) -> std::io::Result<()> {
    let mut stream = server.connect()?;
    stream.set_read_timeout(Some(Duration::from_millis(TIME_OUT)))?;
    let now = SystemTime::now();
    match now.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let total_seconds = duration.as_secs();
            let current_second = total_seconds % 60;
            let total_minutes = total_seconds / 60;
            let current_minute = total_minutes % 60;
            let total_hours = total_minutes / 60;
            let current_hour = total_hours % 24;
            println!(
                "Current time (UTC): {:02}:{:02}:{:02}",
                current_hour, current_minute, current_second
            );
        }
        Err(_) => println!("SystemTime before UNIX EPOCH!"),
    }
    stream.write_all(path.as_bytes())?;
    let mut reader = BufReader::new(stream);
    let mut cur_bytes = 0;
    match file_type {
        FileType::Txt => {
            let mut file = File::create(format!("./{}.txt", name))?;
            let mut line = String::new();
            while cur_bytes < MAX_BYTE {
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        unsafe {
                            if cur_bytes < MIN_SIZE_TXT {
                                MIN_SIZE_TXT = cur_bytes;
                            } else {
                                MIN_SIZE_TXT_CONTENT.clear();
                            }
                            if cur_bytes > MAX_SIZE_TXT {
                                MAX_SIZE_TXT = cur_bytes;
                            }
                        }
                        break;
                    }
                    Ok(_) => {
                        let mut line_chars = line.chars();
                        if line_chars.next() == Some('.') && line_chars.next() != Some('.') {
                            continue;
                        }
                        if line.starts_with("..") {
                            line.remove(0);
                        }
                        cur_bytes += line.as_bytes().len() as u64;
                        unsafe { MIN_SIZE_TXT_CONTENT.push_str(&line) };
                        file.write_all(line.as_bytes())?;
                        line.clear();
                    }
                    Err(e) if e.kind() == ErrorKind::TimedOut => {
                        unsafe { INVALID_NUM += 1 };
                        println!("Read time out!");
                        break;
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
            if cur_bytes > MAX_BYTE {
                unsafe { INVALID_NUM += 1 };
                println!("Over the read limit bytes!");
            }
        }
        FileType::Bin => {
            let mut file = File::create(format!("./{}.bin", name))?;
            let mut line: [u8; BUFF_LEN] = [0; BUFF_LEN];
            while cur_bytes < MAX_BYTE {
                match reader.read(&mut line) {
                    Ok(0) => {
                        unsafe {
                            if cur_bytes < MIN_SIZE_BIN {
                                MIN_SIZE_BIN = cur_bytes;
                            }
                            if cur_bytes > MAX_SIZE_BIN {
                                MAX_SIZE_BIN = cur_bytes;
                            }
                        }
                        break;
                    }
                    Ok(n) => {
                        cur_bytes += n as u64;
                        file.write(&line[..n]).unwrap();
                        line.fill(0);
                    }
                    Err(e) if e.kind() == ErrorKind::TimedOut => {
                        unsafe { INVALID_NUM += 1 };
                        println!("Read time out!");
                        break;
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
            if cur_bytes > MAX_BYTE {
                unsafe { INVALID_NUM += 1 };
                println!("Over the read limit bytes!");
            }
        }
    }
    Ok(())
}

/*
  try to connect to an external resource, if timeout mark it as dead, otherwise mark it as alive
*/
fn test_conn(host: &String, port: &String) -> bool {
    println!("{}:{}\n", host, port);
    let address = format!("{}:{}", host, port);
    let socket_addrs = match address.to_socket_addrs() {
        Ok(r) => r,
        _ => {
            println!("Invalid host or port");
            return false;
        }
    };
    if let Some(socket_addr) = socket_addrs.into_iter().next() {
        let timeout = Duration::from_secs(3);
        match TcpStream::connect_timeout(&socket_addr, timeout) {
            Ok(_) => true,
            Err(e) if e.kind() == ErrorKind::TimedOut => {
                println!("Connection to {} timed out.", address);
                false
            }
            Err(e) => {
                println!("{}", e);
                false
            }
        }
    } else {
        false
    }
}
