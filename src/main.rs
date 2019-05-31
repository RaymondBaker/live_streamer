use std::env;
use std::io::Read;
use std::result;
use std::net::TcpStream;
use std::process;
use std::thread;
use std::thread::JoinHandle;
use std::time;

// Multi threading pipes
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

extern crate crypto;
use crypto::aessafe::AesSafe128Decryptor;

extern crate regex;
use regex::Regex;

extern crate reqwest;
use reqwest::Url;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, REFERER};
use std::collections::HashMap;
use std::collections::VecDeque;

use std::io::{self, Write};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("url was not provided\nFormat: live_streamer <url> <referer> <thread_count>");
        // add output option ie to file to stdout
        process::exit(1);
    }


    let url = &args[1];
    let referer = &args[2];
    let thread_count: i32 = args[3].parse().unwrap();
    if thread_count < 2 {
        eprintln!("Thread count must be greater than one");
        process::exit(1);
    }

    let (pl_tx, pl_rx): (Sender<Playlist>, Receiver<Playlist>) = mpsc::channel();

    let url_cpy = url.clone();
    let pl_tx_cpy = pl_tx.clone();
    let referer_cpy = referer.clone();
    eprintln!("Starting playlist getter...");
    let pl_getter = thread::spawn(move ||
    {
        start_playlist_getter(&url_cpy, &referer_cpy, &pl_tx_cpy);
    });
    eprintln!("Playlist getter started...");
    drop(pl_tx);

    // -1 because theres a thread running above
    let pl_rx = Arc::new(Mutex::new(pl_rx));
    let base_url: Vec<&str> = url.split("/").collect();
    let base_url: String = base_url[0 .. base_url.len()-1].join("/");
    let (vid_tx, vid_rx): (Sender<VideoData>, Receiver<VideoData>) = mpsc::channel();
    for _ in 0..thread_count-1 {
        let base_url_cpy = base_url.clone();
        let vid_tx_cpy = vid_tx.clone();
        let pl_rx_cpy = Arc::clone(&pl_rx);
        let referer_cpy = referer.clone();
        let clip_getter = thread::spawn(move ||
        {
            let client = reqwest::Client::new();
            loop { 
                let playlist          = pl_rx_cpy.lock().unwrap().recv().unwrap(); 
                
                let aes_decryptor     = AesSafe128Decryptor::new(playlist.key);
                let playlist_url      = format!("{}/{}", base_url_cpy, playlist.name);
                let mut resp          = client.get(&playlist_url).headers(construct_headers(&referer_cpy)).send().unwrap();
                let mut buf: Box<Vec<u8>>  = Box::new(vec![]);
                resp.copy_to(&mut buf).unwrap();
                vid_tx_cpy.send(VideoData {
                    order_id: playlist.order_id,
                    data: buf,
                }).unwrap();
            }
        });
    }
    drop(vid_tx);
    
    let mut cur_order: u32 = 0;
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    for vid_chunk in vid_rx {
        handle.write(&vid_chunk.data).unwrap();
        handle.flush().unwrap();
    }
    //bufwriter to write out data to stdout
    pl_getter.join().unwrap();
}

struct VideoData {
    order_id: u64,
    data: Box<Vec<u8>>,
}

struct Playlist {
    order_id: u64,
    name: String,
    encryption: Encryption,
}

enum Encryption {
    NONE,
    AES128 {iv: Vec<u8>, key: Vec<u8>},
}

fn construct_headers(referer: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("reqwest"));
    headers.insert(REFERER, HeaderValue::from_str(referer).unwrap());
    headers
}

fn start_playlist_getter(url: &str, referer: &str, ret_pipe: &Sender<Playlist>) -> Result<(), Box<std::error::Error>> {
    let ten_millis = time::Duration::from_millis(10);
    let mut logged_vids: VecDeque<String> = VecDeque::new();
    let mut order_id: u64 = 0;
    eprintln!("url:{} referer:{}", url, referer);
    let client = reqwest::Client::builder().referer(false).build()?;
    let encryption_re = Regex::new(r"METHOD=(.*?),").unwrap();
    let encryption_iv_re = Regex::new(r"IV=0x(.*?),").unwrap();
    loop {
        // find where last elem is in new playlist append all after that point.
        // if not in new playlist append whole playlist might lead to choppyness
        // return ERROR if 410 gone
        let body  = client.get(url).headers(construct_headers(referer)).send()?.text()?;
        let lines = body.split("\n");
        let mut encryption_t = Encryption::NONE;
        for line in lines {
            if line.starts_with("#") || line.is_empty() {
                if line.starts_with("#EXT-X-KEY") {
                    let encryption_t = encryption_re.captures(line).unwrap().get(1).map_or("", |m| m.as_str());
                    let encryption_iv = encryption_iv_re.captures(line).unwrap().get(1).unwrap().as_str().as_bytes().to_vec();
                    let encryption_key = &encryption_iv;
                    encryption = match encryption_t {
                        "AES-128" => Encryption::AES128 {
                            key: encryption_key,
                            iv:  encryption_iv,
                        },
                        _ => Encryption::NONE,
                    };
                    //eprintln!("{} key {}", encryption, encryption_key);
                }
                continue;
            }
            let line_string = line.to_string();
            if !logged_vids.contains(&line_string) {
                if logged_vids.len() > 50 {
                    logged_vids.pop_front();
                }
                logged_vids.push_back(line_string);
                ret_pipe.send(Playlist {
                    order_id:     order_id,
                    name:         line.to_string(),
                    encryption: encryption,
                })?;
                order_id += 1;
            }
        }
        thread::sleep(ten_millis);
    }
    Ok(())
}
