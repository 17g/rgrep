extern crate regex;
extern crate atty;
extern crate num_cpus;

use std::env;
use std::sync::mpsc::channel;
use std::thread;
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::path::PathBuf;
use std::io::BufReader;
use std::io::BufRead;
use std::io::stdout;
use std::io::Write;
use std::io::BufWriter;
use regex::Regex;
use atty::Stream;

struct SearchResult {
    dir_list :Vec<PathBuf>,
    file_list :Vec<String>
}

fn search_dir(path: &PathBuf) -> SearchResult {
    let mut result = SearchResult{dir_list: vec![], file_list: vec![]};
    for entry in path.read_dir().expect("read_dir call failed") {
        let node = entry.unwrap();
        let child = node.path();
        if child.is_file() {
            result.file_list.push(child.to_str().unwrap().to_string());
        }else{
            result.dir_list.push(child);
        }
    }
    return result;
}

fn main(){

    let thread_count = num_cpus::get() - 2;
    let args: Vec<String> = env::args().collect();
    let search_word = args[1].clone();
    let (r_tx, r_rx) = channel();
    let (wait_tx, wait_rx) = channel();
    let path_str = args[2].clone();
    let start_path = PathBuf::from(&path_str);
    let multi_file = start_path.is_dir();

    let alive = Arc::new(0);
    let path_list :Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let ins_path_list = path_list.clone();
    let search_thread = thread::spawn(move||{
        //println!("search thread start");
        if start_path.is_file() {
            ins_path_list.lock().unwrap().push(path_str.clone());
            let _ = wait_tx.send("start");    
        } else {
            let mut dir_list = vec![start_path];
            while &dir_list.len() > &0 {
                let mut need_search  = vec![];
                for dir in &dir_list {
                    let mut result = search_dir(dir);
                    {
                        let mut guard = ins_path_list.lock().unwrap();
                        guard.append(&mut result.file_list);
                        if guard.len() > thread_count {
                            let _ = wait_tx.send("start");    
                        }
                        //println!("{:?},{}", dir_list, guard.len());
                    }
                    need_search.append(&mut result.dir_list);
                }
                dir_list.truncate(0);
                dir_list.append(&mut need_search);
            }
            let _ = wait_tx.send("start");    
        }
        //println!("search thread end");
    });

    let mut read_threads = vec![];
    let _ = wait_rx.recv();
    for _i in 0..thread_count {
        let read_alive = alive.clone();
        let is_stdout = atty::is(Stream::Stdout);
        let thread_multi_file = multi_file.clone();
        let thread_path_list = path_list.clone();
        let thread_r_tx = r_tx.clone();
        let thread_search_word = search_word.clone();
        read_threads.push(thread::spawn(move||{
            //println!("read thread-{} start", i);
            let mut _read_path = String::new();
            let _ = read_alive;
            loop {
                {
                    let mut guard = thread_path_list.lock().unwrap();
                    if guard.len() > 0 {
                        _read_path = guard.remove(0);
                    }else{
                        break;
                    }
                }
                //println!("thread-{}:{}", i,_read_path);
                let file = File::open(&_read_path).expect("open file fail");
                let mut reader = BufReader::new(file);
                //let mut line = String::new();
                let mut line_bytes = vec![];
                let mut line_num = 1;
                let re = Regex::new(&thread_search_word[..]).expect("get regex fail");
                let mut ret_val = String::new();
                //while reader.read_line(&mut line).expect("read file fail") > 0 {
                while reader.read_until(b'\n', &mut line_bytes).expect("read file fail") > 0 {
                    if !line_bytes.is_ascii(){
                        //println!("Skipped binary file");
                        break;
                    }
                    let line = String::from_utf8(line_bytes.clone()).expect("Found invalid UTF-8");
                    if re.is_match(&line[..]) {
                        let result = if is_stdout || thread_multi_file {
                                        if thread_multi_file {
                                            format!("\u{1b}[32m{}@{}\u{1b}[0m: {}", _read_path, line_num, re.replace_all(&line[..], "\u{1b}[31m$0\u{1b}[0m"))
                                        }else{
                                            format!("\u{1b}[32m{}:\u{1b}[0m{}", line_num, re.replace_all(&line[..], "\u{1b}[31m$0\u{1b}[0m"))
                                        }
                                     }else{
                                        line.clone()
                                     };
                        ret_val += &result[..];
                    }
                    line_num += 1;
                    //line.clear();
                    line_bytes.truncate(0);
                }
                _read_path.clear();
                let _ = thread_r_tx.send(ret_val);
            }
            //println!("read thread-{} end", i);
        }));
    }

    let write_alive = alive.clone();
    let write_thread = thread::spawn(move||{
        //println!("write thread start");
        let alive = write_alive;
        // 自分のスレッド内と親スレッド内の参照が残るので、2以上の参照でループ継続
        while Arc::strong_count(&alive) > 2 {
            match r_rx.try_recv(){
                Err(_) => {},
                Ok(v) => {
                            let out = stdout();
                            let mut out = BufWriter::new(out.lock());
                            let _ = out.write(v.as_bytes());
                            let _ = stdout().flush();
                         }
            }
        }
        //println!("write thread end");
    });

    let _ = search_thread.join();
    for read_thread in read_threads {
        let _ = read_thread.join();
    }
    //println!("remain file count:{}",path_list.lock().unwrap().len());

    let _ = write_thread.join();
}

