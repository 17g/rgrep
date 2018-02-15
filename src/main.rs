extern crate regex;
extern crate atty;

use std::env;
use std::sync::mpsc::channel;
use std::thread;
use std::sync::Arc;
use std::fs::File;
use std::io::BufReader;
use std::io::BufRead;
use std::io::stdout;
use std::io::Write;
use std::io::BufWriter;
use regex::Regex;
use atty::Stream;

fn main(){

    let args: Vec<String> = env::args().collect();
    let search_word = args[1].clone();
	let (r_tx, r_rx) = channel();

	let path = args[2].clone();
	let alive = Arc::new(0);

    let is_stdout = atty::is(Stream::Stdout);
    let read_alive = alive.clone();
	let read_thread = thread::spawn(move||{
        let _ = read_alive;
        let file = File::open(path).unwrap();
		let mut reader = BufReader::new(file);
		let mut line = String::new();
        let mut line_num = 1;
        let re = Regex::new(&search_word[..]).unwrap();
        let mut ret_val = String::new();
		while reader.read_line(&mut line).unwrap() > 0 {
            if re.is_match(&line[..]) {
                let result = if is_stdout {
			                    format!("\u{1b}[32m{}:\u{1b}[0m{}", line_num, re.replace_all(&line[..], "\u{1b}[31m$0\u{1b}[0m"))
                             }else{
			                    format!("{}", re.replace_all(&line[..], "$0"))
                             };
			    ret_val += &result[..];
			}
			line_num += 1;
			line.clear();
		}
	    let _ = r_tx.send(ret_val);
	});

    let write_alive = alive.clone();
	let write_thread = thread::spawn(move||{
        let alive = write_alive;
		// 自分のスレッド内と親スレッド内の2つの参照が残るので、2以上でループ継続
		while Arc::strong_count(&alive) > 2 {
			match r_rx.recv(){
				Err(_) => {},
				Ok(v) => {
                            let out = stdout();
                            let mut out = BufWriter::new(out.lock());
                            let _ = out.write(v.as_bytes());
                            let _ = stdout().flush();
                         }
			}
		}
	});

	let _ = read_thread.join();
	let _ = write_thread.join();

}
