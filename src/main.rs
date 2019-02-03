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

extern crate reqwest;
use std::collections::HashMap;
use std::collections::VecDeque;

use std::io::{self, Write};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("url was not provided\nFormat: live_streamer <url> <thread_count>");
        // add output option ie to file to stdout
        process::exit(1);
    }


    let url = &args[1];
    let thread_count: i32 = args[2].parse().unwrap();
    if thread_count < 2 {
        eprintln!("Thread count must be greater than one");
        process::exit(1);
    }

    let (pl_tx, pl_rx): (Sender<Playlist>, Receiver<Playlist>) = mpsc::channel();

    let url_cpy = url.clone();
    let pl_tx_cpy = pl_tx.clone();
    let pl_getter = thread::spawn(move ||
    {
        start_playlist_getter(&url_cpy, &pl_tx_cpy);
    });
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
        let clip_getter = thread::spawn(move ||
        {
            loop { 
                let playlist = pl_rx_cpy.lock().unwrap().recv().unwrap(); 
                let playlist_url      = format!("{}/{}", base_url_cpy, playlist.name);
                let mut resp          = reqwest::get(&playlist_url).unwrap();
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
    key: String,
}

fn start_playlist_getter(url: &str, ret_pipe: &Sender<Playlist>) -> Result<(), Box<std::error::Error>> {
    let ten_millis = time::Duration::from_millis(10);
    let mut logged_vids: VecDeque<String> = VecDeque::new();
    let mut order_id: u64 = 0;
    loop {
        // find where last elem is in new playlist append all after that point.
        // if not in new playlist append whole playlist might lead to choppyness
        // return ERROR if 410 gone
        let body  = reqwest::get(url)?.text()?;
        let lines = body.split("\n");
        for line in lines {
            if line.starts_with("#") || line.is_empty() {
                continue;
            }

            let line_string = line.to_string();
            if !logged_vids.contains(&line_string) {
                if logged_vids.len() > 50 {
                    logged_vids.pop_front();
                }
                logged_vids.push_back(line_string);
                ret_pipe.send(Playlist {
                    order_id,
                    name: line.to_string(),
                    key: "".to_string(),
                })?;
                order_id += 1;
            }
        }
        thread::sleep(ten_millis);
    }
    Ok(())
}
