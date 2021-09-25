use std::io::stderr;
use std::io::Write;

fn main() {
    let args:Vec<_> = std::env::args().collect();
    for i in args {
        let _ = stderr().write(i.as_bytes());
        let _ = stderr().write("\r\n".as_bytes());
    }
    println!("Hello World!");
}
