use super::*;
use downcast_rs::Downcast;
use super::sockets::*;
use super::server::*;
use std::time::*;
use std::string;
use std::sync::Arc;
use std::cell::RefCell;
use std::sync::Mutex;
use std::collections::VecDeque;
use std::str::from_utf8_unchecked;
use super::threads::*;

pub trait Service:SocketInterface {
    fn service(&mut self,s:PtrSocket,pl:&mut EPoll)->Option<PtrSocket>;
    fn dont(&self)->bool { return false; }
    fn socket(&mut self)->Option<PtrSocket>;
}
impl_downcast!(Service);

pub struct Default {
    l:bool,
    tm:Instant
}
impl Default {
    pub fn new(l:bool)->Self {
        Default{tm:Instant::now(),l:l}
    }
}
impl Service for Default {
    fn socket(&mut self)->Option<PtrSocket> {
        None
    }
    fn service(&mut self,s:PtrSocket,pl:&mut EPoll)->Option<PtrSocket>{
//        pl.write(s,Some(format!("Hello World!!!\r\n").into_bytes()));
        let msg = format!("<HTML>Hello World !!!</HTML>\r\n");
        pl.write(s,Some(format!("HTTP/1.1 200 OK\r\n\
        Connection: close\r\n\
        Content-Length: {}\r\n\
        Server: bmaxa\r\n\
        Content-Type: text/html; charset=utf-8\r\n\r\n{}",
        msg.len(),msg).into_bytes()));
        //std::thread::sleep(Duration::from_millis(1));
        return None;
    }
}
impl SocketInterface for Default {
    fn clone(&self)->Box<SocketInterface> {
        Box::new(Default::new(false))
    }
    fn get_service(&mut self)->Option<&mut Service> {
        Some(self)
    }
    fn done_connected(&mut self,s:PtrSocket,pl:&mut EPoll)->Result<Option<PtrSocket>,String> {
        pl.read(s,1024);
        Ok(None)
    }
    fn done_reading(&mut self,s:PtrSocket,pl:&mut EPoll)->Result<Option<PtrSocket>,String> {
        let rs = s.socket();
        unsafe {
        if !cfg!(feature = "service") {
            pl.write(s.clone(),Some(format!("Hello World!!!\r\n").into_bytes()));
        }
        Log::log(&format!("remote: {}, {}, \r\n{}\r\n",c_time().trim_right(),get_peer_name(rs.sd),from_utf8_unchecked(&rs.rd_buf[0..rs.rd_buf_pos])));
        }
        if cfg!(feature = "service") {
            Ok(Some(s.clone()))
        } else {
            Ok(None)
        }
    }
    fn done_writing(&mut self,s:PtrSocket,pl:&mut EPoll)->Result<Option<PtrSocket>,String>{
        Ok(None)
    }
    fn tick(&mut self)->bool {
        if !self.l && self.tm.elapsed().as_secs() > 1 { return true; }
        false
    }
}
pub struct Control {
    res_:String
}
impl Control {
    pub fn new()->Control {
        Control{res_:String::new() }
    }
}
impl SocketInterface for Control {
    fn clone(&self)->Box<SocketInterface>{
        Box::new(Control{res_:String::new()})
    }
    fn get_service(&mut self)->Option<&mut Service> {
        Some(self)
    }
    fn done_connected(&mut self,s:PtrSocket,pl:&mut EPoll)->Result<Option<PtrSocket>,String> {
        let peer;
        {
        let rs = s.socket();
        peer = get_peer_name(rs.sd);
        }
        pl.write(s.clone(),Some(format!("Hello {} from control service\r\n",peer).into_bytes()));
        Ok(None)
    }
    fn done_reading(&mut self,s:PtrSocket,pl:&mut EPoll)->Result<Option<PtrSocket>,String> {
        let rs = s.socket();
        self.res_.push_str(string::String::from_utf8_lossy(&rs.rd_buf[0..rs.rd_buf_pos]).trim_right());
        Ok(Some(s.clone()))
    }
    fn done_writing(&mut self,s:PtrSocket,pl:&mut EPoll)->Result<Option<PtrSocket>,String> {
        self.res_.clear();
        pl.read(s,1024);
        Ok(None)
    }
}
impl Service for Control {
    fn socket(&mut self)->Option<PtrSocket> {
        None
    }
    fn service(&mut self,s:PtrSocket,pl:&mut EPoll)->Option<PtrSocket> {
        let mut howmany=0;
        let mut shutdown = false;
        {
            unsafe {
            let lcm = AutoLock::new(&mut lstCntrlM_);
            let mut lst = get_lst(&mut lstCntrl_);
            let (mut sum,mut sum1)=(0,0);
            let tmp:String;
            {
            let inp:Vec<_> = self.res_.split(' ').collect();
            if inp.len() > 0 {
                if inp[0] == "start" {
                    howmany = inp[1].parse().unwrap();
                } else if inp[0] == "shutdown" {
                    shutdown = true;
                }
            }
            }
            for cntrl in lst.iter_mut() {
                if shutdown { (**cntrl).shutdown = true; }
                self.res_ += format!("thread: {} serviced: {} pending: {} accepted: {}\r\n",
                                (**cntrl).svcid,(**cntrl).serviced,(**cntrl).npending,(**cntrl).accepted).as_str();
                sum += (**cntrl).accepted;
                sum1 += (**cntrl).serviced;
            }
            self.res_ += &format!("threads avail: {}\r\n",threads_avail());
            self.res_ += &format!("total accepted in active threads: {}\r\n",sum);
            self.res_ += &format!("total accepted: {}\r\n",ServiceInfo::instance().accepted(0));
            self.res_ += &format!("total serviced in active threads: {}\r\n",sum1);
            self.res_ += &format!("total serviced: {}\r\n",ServiceInfo::instance().serviced(0));
            self.res_ += &format!("total io serviced: {}\r\n",ServiceInfo::instance().io_serviced(0));
            self.res_ += &format!("avgspeed (reqs/sec): {}\r\n",ServiceInfo::instance().avgtime());
            self.res_ += &format!("avgreqs: {}\r\n",ServiceInfo::instance().avgreqs());
            if shutdown { self.res_ += "shutting down\r\n"; }
            }
        }
        for i in 0..howmany {
            Server::start_thread(None);
        }
        if !shutdown { 
            let tmp = self.res_.clone();
            pl.write(s,Some(tmp.into_bytes())); 
        }
        None
    }
}

pub struct Log;
static mut buf_ : Option<Mutex<VecDeque<(i32,String)>>> = None;

impl Log {
    pub fn new()->Log {
        Log
    }
    fn log(s:&str) {
        unsafe {
        if buf_.is_none() {
            buf_ = Some(Mutex::new(VecDeque::new()));
        }
        let mut buf = get_lst(&mut buf_).lock().unwrap();
        if buf.len() > 0 {
            if buf.back().unwrap().1 == s {
                buf.back_mut().unwrap().0 += 1;
                return;
            }
        }
        if buf.len() > 1000 { buf.pop_front(); }
        buf.push_back((0,s.into()));
        }
    }
}
impl SocketInterface for Log {
    fn clone(&self)->Box<SocketInterface> {
        Box::new(Log::new())
    }
    fn get_service(&mut self)->Option<&mut Service> {
        None
    }
    fn done_connected(&mut self,s:PtrSocket,pl:&mut EPoll)->Result<Option<PtrSocket>,String> {
        pl.read(s,1024);
        Ok(None)
    }
    fn done_reading(&mut self,s:PtrSocket,pl:&mut EPoll)->Result<Option<PtrSocket>,String> {
        let mut buf = String::new();
        unsafe {
        if buf_.is_none() {
            buf_ = Some(Mutex::new(VecDeque::new()));
        }
        let mut logbuf_ = get_lst(&mut buf_).lock().unwrap();
        for (i,v) in logbuf_.iter() {
            buf += &format!("{} {}",i,v);
        }
        pl.write(s,Some(buf.into_bytes()));
        }
        Ok(None)
    }
    fn done_writing(&mut self,s:PtrSocket,pl:&mut EPoll)->Result<Option<PtrSocket>,String>{
        Ok(None)
    }
}

pub trait PipeServer{
    fn doneReading(&mut self)->bool;
    fn doneWriting(&mut self)->bool;
    fn prepare_request(&mut self,ps:PtrPipeService,pl:&mut EPoll)->i32;
    fn prepare_response(&mut self,res:&mut String);
    fn perform(&mut self,pl:&mut EPoll)->i32;
    fn end(&mut self)->bool;
    fn clone(&mut self)->PtrPipeServer;
    fn reset(&mut self);
}

trait IGetPtrPipeServer {
    fn doneReading(&self)->bool ;
    fn doneWriting(&self)->bool ;
    fn prepare_request(&self,ps:PtrPipeService,pl:&mut EPoll)->i32 ;
    fn prepare_response(&self,res:&mut String);
    fn perform(&self,pl:&mut EPoll)->i32;
    fn end(&self)->bool;
    fn clone(&self)->PtrPipeServer;
    fn reset(&self);
}
impl IGetPtrPipeServer for PtrPipeServer{
    fn doneReading(&self)->bool {
        let mut rc:*mut PipeServer;
        {
            match self {
                Some (ref s)=>{
                    rc = &mut *s.borrow_mut();
                },
                None => panic!("is none")
            }
        }
        unsafe {(*rc).doneReading()}
    }
    fn doneWriting(&self)->bool {
        let mut rc:*mut PipeServer;
        {
            match self {
                Some (ref s)=>{
                    rc = &mut *s.borrow_mut();
                },
                None => panic!("is none")
            }
        }
        unsafe {(*rc).doneWriting()}
    }
    fn prepare_request(&self,ps:PtrPipeService,pl:&mut EPoll)->i32 {
        let mut rc:*mut PipeServer;
        {
            match self {
                Some (ref s)=>{
                    rc = &mut *s.borrow_mut();
                },
                None => panic!("is none")
            }
        }
        unsafe {(*rc).prepare_request(ps,pl)}
    }
    fn prepare_response(&self,res:&mut String){
        let mut rc:*mut PipeServer;
        {
            match self {
                Some (ref s)=>{
                    rc = &mut *s.borrow_mut();
                },
                None => panic!("is none")
            }
        }
        unsafe {(*rc).prepare_response(res)}
    }
    fn perform(&self,pl:&mut EPoll)->i32{
        let mut rc:*mut PipeServer;
        {
            match self {
                Some (ref s)=>{
                    rc = &mut *s.borrow_mut();
                },
                None => panic!("is none")
            }
        }
        unsafe {(*rc).perform(pl)}
    }
    fn end(&self)->bool{
        let mut rc:*mut PipeServer;
        {
            match self {
                Some (ref s)=>{
                    rc = &mut *s.borrow_mut();
                },
                None => panic!("is none")
            }
        }
        unsafe {(*rc).end()}
    }
    fn clone(&self)->PtrPipeServer{
        let mut rc:*mut PipeServer;
        {
            match self {
                Some (ref s)=>{
                    rc = &mut *s.borrow_mut();
                },
                None => panic!("is none")
            }
        }
        unsafe {(*rc).clone()}
    }
    fn reset(&self){
        let mut rc:*mut PipeServer;
        {
            match self {
                Some (ref s)=>{
                    rc = &mut *s.borrow_mut();
                    unsafe {(*rc).reset()}
                },
                None => ()
            }
        }
    }
}

impl IGetPtrPipe for PtrPipe {
    fn prepare(&self,pl:&mut EPoll){
        let mut rc:*mut Pipe;
        {
            match self {
                Some (ref s)=>{
                    rc = &mut *s.borrow_mut();
                },
                None => panic!("is none")
            }
        }
        unsafe {(*rc).prepare(pl)}
    }

    fn set_ps(&self,ps:PtrPipeService){
        let mut rc:*mut Pipe;
        {
            match self {
                Some (ref s)=>{
                    rc = &mut *s.borrow_mut();
                },
                None => panic!("is none")
            }
        }
        unsafe {(*rc).set_ps(ps)}
    }

}

trait IGetPtrPipe {
    fn prepare(&self,pl:&mut EPoll);
    fn set_ps(&self,ps:PtrPipeService);
}

pub type PtrPipeService = Option<Arc<RefCell<PipeService>>>;
type PtrPipe = Option<Arc<RefCell<Pipe>>>;
pub type PtrPipeServer = Option<Arc<RefCell<PipeServer>>>;

pub trait Pipe:Service{
    fn prepare(&mut self,pl:&mut EPoll);
    fn set_ps(&mut self,ps:PtrPipeService);
}
#[derive(Clone)]
pub struct PipeService{
    s_: Option<PtrSocket>,
    pipe_: PtrPipe,
    ps_: PtrPipeServer,
    toRead_: i32,
    buf_: String
}

impl PipeService {
    pub fn new()->PipeService {
        PipeService{ 
            s_: None,
            pipe_:None,
            ps_:None,
            toRead_:0,
            buf_:String::new()
        }

    }
    pub fn new_ps<T:'static+PipeServer>(ps:T)->PipeService{
        PipeService{ps_:Some(Arc::new(RefCell::new(ps))),..PipeService::new()}
    }
    fn clone_ptr(&self)->PtrPipeService {
        Some(Arc::new(RefCell::new(Clone::clone(self))))
    }
    fn cloneServer(&self)->PtrPipeServer{
        IGetPtrPipeServer::clone(&self.ps_)
    }
    pub fn setServer(&mut self,ps:PtrPipeServer){
        self.ps_ = ps;
    }
    pub fn setPipe(&mut self,pipe:PtrPipe){
        self.pipe_ = pipe;
    }
    pub fn setToRead(&mut self, tr:i32){
        self.toRead_ = tr;
    }
    pub fn buf(&mut self)->&mut String{
        &mut self.buf_
    }
    pub fn write(&mut self,buf:Vec<u8>){
        let s = self.s_.clone().unwrap();
        s.borrow_mut().write(Some(buf));
    }
}
impl Service for PipeService {
    fn socket(&mut self)->Option<PtrSocket> {
        self.s_.clone()
    }
    fn service(&mut self,s:PtrSocket,pl:&mut EPoll)->Option<PtrSocket>{
        if !self.ps_.is_none() {
            if self.toRead_ == -1 {
                self.toRead_ = self.ps_.perform(pl);
                if self.toRead_ == -4 || self.toRead_ == -3 {
                    println!("performed {:?} {}",std::thread::current().id(),self.toRead_);
                    pl.write(s,None);
                } else if self.toRead_ == -1 {
                    return Some(s);
                } else if self.toRead_ > 0 {
                    pl.read(s,1024);
                }
            } else if self.toRead_ == 0 {
                println!("preparing ");
                self.toRead_ = self.ps_.prepare_request(self.clone_ptr(),pl);
                if self.toRead_ == -4 || self.toRead_ == -3 {
                    pl.write(s,None);
                } else if self.toRead_ == -1{
                    return Some(s);
                } else if self.toRead_ > 0 {
                    pl.read(s,1024);
                }
            }
        } else {
            pl.write(s,None);
        }
        None
    }
}

impl SocketInterface for PipeService{
    fn clone(&self)->Box<SocketInterface> {
        let mut rc = Box::new(Clone::clone(self));
        rc.setServer(self.cloneServer());
        rc
    }
    fn get_service(&mut self)->Option<&mut Service> {
        Some(self)
    }
    fn done_connected(&mut self, s:PtrSocket,pl:&mut EPoll)->Result<Option<PtrSocket>,String>{
        self.s_ = Some(s.clone());
        println!("connected {} sd:{}",s.socket().id_,s.socket().sd);
        pl.read(s,1024);
        Ok(None)
    }
    fn done_closed(&mut self,s:PtrSocket,pl:&mut EPoll){
        if !self.pipe_.is_none() {
            self.pipe_.set_ps(None);
        }
    }
    fn handle_error(&mut self,s:PtrSocket,pl:&mut EPoll,e:String)->Result<Option<PtrSocket>,String>{
        if !self.pipe_.is_none() {
            self.pipe_.set_ps(None);
        }
        Ok(None)
    }
    fn done_reading(&mut self, s:PtrSocket,pl:&mut EPoll)->Result<Option<PtrSocket>,String>{
        if self.toRead_ == -3 { return Ok(None) }
        println!("done reading {}",self.toRead_);
        if self.toRead_ == -2 && !self.pipe_.is_none() {
            self.pipe_.prepare(pl);
            return Ok(None)
        }
        if !self.ps_.is_none() {
            if self.toRead_ > 0 {
                {
                    let rs = s.socket();
                    self.buf_ += unsafe { from_utf8_unchecked(&rs.rd_buf[0..rs.rd_buf_pos]) };
                }
                if self.buf_.len() < self.toRead_ as usize {
                    pl.read(s.clone(),1024);
                } else {
                    if !self.ps_.doneReading() {
                        pl.read(s.clone(),1024);
                    } else {
                        self.toRead_ = -1;
                    }
                }
            } else {
                let rs = s.socket();
                self.buf_ = unsafe { from_utf8_unchecked(&rs.rd_buf[0..rs.rd_buf_pos]).to_string() };
            }
        }
        println!("Ok(Some({})) sd {}",s.socket().id_,s.socket().sd);
        Ok(Some(s))
    }
    fn done_writing(&mut self, s:PtrSocket,pl:&mut EPoll)->Result<Option<PtrSocket>,String>{
        if self.toRead_ == -3 {
            println!("ending connection -3");
            self.s_ = None;
            self.ps_.reset();
            self.ps_ = None;
            return Ok(None)
        }
        if self.toRead_ == -2 && !self.pipe_.is_none() {
            self.pipe_.prepare(pl);
            return Ok(None)
        }
        if self.toRead_ == -4 && !self.ps_.is_none() && !self.ps_.doneWriting() {
            pl.write(s,None);
            return Ok(None)
        }
        let mut rc = if !self.ps_.is_none() && !self.ps_.end() {
            Ok(Some(s.clone()))
        } else {
            println!("ending connection");
            self.s_ = None;
            self.ps_.reset();
            self.ps_ = None;
            Ok(None)
        };
        self.toRead_ = 0;
        self.buf_.clear();
        if let Ok(Some(_)) = rc.clone() { 
            pl.read(s,1024); 
        } 
        rc
    }
    fn keep(&mut self)->bool {
        true
    }
}

pub struct DefaultPipeServer {
    ps_: PtrPipeService,
    inp_: String,
    exe_: String,
    end_ : bool
}

impl DefaultPipeServer {
    pub fn new()->DefaultPipeServer {
        DefaultPipeServer{
            ps_:None,
            inp_:String::new(),
            exe_:String::new(),
            end_:false
        }
    }
    pub fn new_exe(exe:String)->DefaultPipeServer {
        DefaultPipeServer{exe_:exe,..DefaultPipeServer::new()}
    }
}

use std::process::{Command, Stdio};
use std::io::Write;
use std::io::{stdout,stderr};

impl PipeServer for DefaultPipeServer {
    fn doneReading(&mut self)->bool{
        true
    }
    fn doneWriting(&mut self)->bool{
        true
    }
    fn prepare_request(&mut self,ps:PtrPipeService,pl:&mut EPoll)->i32{
        //std::thread::sleep(std::time::Duration::from_millis(1000));
        self.ps_ = ps;
        let ps = self.ps_.clone().unwrap();
        self.inp_= ps.borrow().buf_.clone();
//      ps.borrow_mut().write("Hello World!!!\r\n".to_string().into_bytes());
        -1
    }
    fn prepare_response(&mut self,res:&mut String){
    }
    fn perform(&mut self,pl:&mut EPoll)->i32{
//        std::thread::sleep(std::time::Duration::from_millis(10));
        println!("perform thread {:?} pending {}",std::thread::current().id(),pl.pending());
        println!("entered perform before unwrap, end {}",self.end_);
        let ps = self.ps_.clone().unwrap();
        let mut ps = ps.borrow_mut();
        let s = ps.socket().unwrap();
        println!("entered perform {}",s.socket().id_);
		let mut child = Command::new(self.exe_.clone())
                                    .arg(get_peer_name(s.socket().sd))
                                    .arg(self.inp_.clone())
            					    .stdin(Stdio::piped())
			            		    .stdout(Stdio::piped())
			            		    .stderr(Stdio::piped())
            					    .spawn()
			            		    .expect("failed to execute child");

//		{
		    // limited borrow of stdin
//		    let stdin = child.stdin.as_mut().expect("failed to get stdin");
//		    stdin.write_all(b"test").expect("failed to write to stdin");
//		}

		let output = child
		    .wait_with_output()
		    .expect("failed to wait on child"); 
        let _ = stderr().write(&output.stderr);
        ps.write(output.stdout);
        -4
	}
    fn end(&mut self)->bool{
        let ps = self.ps_.clone().unwrap();
        let mut ps = ps.borrow_mut();
        let s = ps.socket().unwrap();
        println!("perform thread {:?} ending {}",std::thread::current().id(),s.socket().id_);
        self.inp_.clear();
        true
    }
    fn reset(&mut self){
        self.ps_ = None;
        self.end_ = true;
    }
    fn clone(&mut self)->PtrPipeServer{
        Some(Arc::new(RefCell::new(DefaultPipeServer{exe_:self.exe_.clone(),..DefaultPipeServer::new()})))
    }
}
