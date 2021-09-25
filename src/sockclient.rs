#![feature(slice_patterns)]
extern crate web_server;
use web_server::*;
use web_server::sockets::*;
use std::env;
use std::time::*;
use std::str::FromStr;
//use std::str;
//use std::string;
use std::sync::Arc;
use std::cell::RefCell;
type PtrCount = Arc<RefCell<i32>>;
type PtrClosed = Arc<RefCell<i32>>;
type PtrErrors = Arc<RefCell<i32>>;

#[derive(Clone)]
struct Hello(String,Instant,PtrCount,PtrClosed,PtrErrors);

impl SocketInterface for Hello{
    fn clone(&self)->Box<SocketInterface>
    {
        Box::new(Clone::clone(self))
    }
    fn done_connected(&mut self,s:PtrSocket,p:&mut EPoll)->Result<Option<PtrSocket>,String>
    {
        let _ = p.write(s,Some("GET / HTTP/1.0\r\n\r\n".to_string().into_bytes()));
//        p.write(s,"quit\r\n".to_string().into_bytes())
        Ok(None)
    }
    fn done_reading(&mut self,s:PtrSocket,p:&mut EPoll)->Result<Option<PtrSocket>,String>
    {
        unsafe {
            let s = s.socket();
            self.0.push_str(std::str::from_utf8_unchecked(&s.rd_buf[0..s.rd_buf_pos]));
        }

        if self.0 != "Close".to_string() { let _ = p.read(s,1024); }
        Ok(None)
    }
    fn done_writing(&mut self,s:PtrSocket,p:&mut EPoll)->Result<Option<PtrSocket>,String>
    {
        let _ = p.read(s,1024);
        Ok(None)
    }
    fn done_closed(&mut self,_:PtrSocket,_:&mut EPoll)
    {
        let res = match self.0.find("Server:") {
            Some(x) => {
                let y = self.0[x..].find("\r\n").unwrap();
                &self.0[x..x+y]
            },
            None => "server unkown"
        };
//        println!("data len {} {}",self.0.len(),"got ".to_string() + res);
    }
    fn handle_error(&mut self,_:PtrSocket,_:&mut EPoll,e:String)->Result<Option<PtrSocket>,String>
    {
        println!("{}",e);
        *self.4.borrow_mut() += 1;
        Ok(None)
    }
    fn tick(&mut self)->bool {
        *self.2.borrow_mut() += 1;
        if self.1.elapsed().as_secs() > 10 {
            self.0 = "Close".to_string();
            *self.3.borrow_mut() += 1;
            return  true }
        if *self.2.borrow() % 10000 == 0 {
            println!("ticks {}",self.2.borrow());
        }
        false
    }
}

fn main()
{
    let addr;
    let mut i:i32;
    let c:u32;
    let sl:Vec<String> = env::args().collect();
    match &sl[..] {
        &[_,ref host,ref port,ref conc,ref n] => {
            let (name,res) = Socket::host_by_name(&host.clone()).unwrap();
            println!("host name: {}",name);
            for s in res.iter(){
                println!("addr: {}",*s)
            }
            addr = res[0].clone() + ":" + &port;
            i = i32::from_str(n).unwrap();
            c = u32::from_str(conc).unwrap();
        }
        &[ref prog,..] => {
            let st: String = prog.clone();
            println!("{}",st+" host port conc niter");
            return;
        }
        &[] => return
    }
    let mut epoll = EPoll::new(1000);
    let n = i as f64;
    let start = Instant::now();
    let var = Arc::new(RefCell::new(0));
    let var1 = Arc::new(RefCell::new(0));
    let var2 = Arc::new(RefCell::new(0));
    loop {
        let k = i as u32;
        let con = if c > k { k } else { c };
        if epoll.pending() < con as usize {
        for _ in 0..con as usize - epoll.pending() {
            let s = Arc::new(RefCell::new(
                    ESocket::new_impl(Box::new(Hello("".to_string(),
                    Instant::now(),
                    var.clone(),
                    var1.clone(),
                    var2.clone())))));
            match epoll.connect(s,&addr) {
                Err(s) => panic!(s),
                _ => { i-= 1; }
            }
        }
        }
        let _ = epoll.run(1,None,1000,true); // run once
        if i <= 0 {break;}
    }
    let _ = epoll.run(-1,None,1000,true); // do the rest
    let end = start.elapsed();
    let diff = (end.as_secs()*1000000000+end.subsec_nanos()as u64) as f64;
    let rs = (n/(diff/1000000000.0)) as u32;
    println!("{}",rs.to_string()+" recs/sec");
    println!("Errors {}",*var2.borrow());
    println!("Timeouted {} \n",*var1.borrow());
}
