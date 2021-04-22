use super::*;
use super::server::*;
use std::collections::linked_list::LinkedList;

use std::ffi::{CString, CStr};
use std::str;
use std::str::FromStr;
//use std::vec;
use std::collections::HashMap;
use std::mem;
use std::io::Write;
use super::libc::{size_t,c_int,c_char,time_t};
use downcast_rs::Downcast;
use super::service::Service;
use std::cell::RefCell;
use std::sync::Arc;

static SIGPIPE:i32 = 13;

static EINPROGRESS:i32 = 115;
static EAGAIN:i32 = 11;

static AF_LOCAL:i32 = 1;
static AF_INET:i32 = 2;
static SOCK_STREAM:i32 = 1;
static INADDR_ANY:i32 = 0;

static SOL_SOCKET:i32 = 1;
static SO_REUSEADDR:i32 = 2;
static SO_ERROR:i32 = 4;

static EPOLL_CTL_ADD:i32 = 1;
static EPOLL_CTL_DEL:i32 = 2;
static EPOLL_CTL_MOD:i32 = 3;

static EPOLLIN:u32  = 0x001;
static EPOLLOUT:u32 = 0x004;
static EPOLLONESHOT:u32 = 1u32 << 30;
static EPOLLET:u32 = 1u32 << 31;

static F_GETFL : i32 = 3;
static F_SETFL : i32 = 4;
static O_NONBLOCK: i32 = 04000;

extern "C" { 
    fn close(sd: i32);
    fn read(fd:i32,buf:*mut u8,sz:size_t)->i64;
    fn write(fd:i32,buf:*const u8,sz:size_t)->i64;
    fn socket(domain:i32,tp:i32,protocol:i32)->i32;
    fn socketpair(domain:i32, tp:i32, protocol:i32, sv:*mut i32)->i32;
    fn setsockopt(sockfd:i32,level:i32,optname:i32,optval:*mut sockaddr_in,optlen:i32)->i32;
    fn getsockopt(sockfd:i32,level:i32,optname:i32,optval:*mut i32,optlen:*mut i32)->i32;
    fn gethostbyname(addr:*const c_char)->*const hostent;
    fn gethostbyaddr(addr:*const in_addr,len:i32,t:i32)->*const hostent;
    fn getpeername(sd:i32,addr:*mut sockaddr_in,len:*mut i32)->i32;
    fn getsockname(sd:i32,addr:*mut sockaddr_in,len:*mut i32)->i32;
    fn htons(s:u16)->u16;
    fn htonl(s:i32)->i32;
    fn inet_aton(cp:*const i8, inp:*const in_addr)-> i32;
    fn inet_ntoa(addr:in_addr)-> *const c_char;
    fn bind(sd:i32,sa:*const sockaddr_in,len:i32)->i32;
    fn listen(sd:i32,back:i32)->i32;
    fn accept(sd:i32,sa:*mut sockaddr_in,len:*mut u32)->i32;
    fn connect(sd:i32,sa:*const sockaddr_in,len:i32)->i32;
    fn epoll_create(sz:i32)->i32;
    fn epoll_ctl(epfd:i32,op:i32,fd:i32,ev:*const epoll_event)->i32;
    fn epoll_wait(epfd:i32,evs:*mut epoll_event,max_evs:i32,time_out:i32)->i32;
    fn fcntl(sd: i32,op:i32,...)->c_int;
//    fn set_nonblocking(sd: i32);
    fn puts(buf:*const c_char);
    fn ctime_r(t: *const time_t,buf:*mut u8)->*const c_char;
    fn time(t:*mut time_t)->time_t;
    fn signal(s:c_int,f:extern fn (i32))->fn(i32);
}
extern fn handler(s:i32){
//    panic!("SIGPIPE");
    unsafe { signal(s,handler); }
//    std::io::stderr().write("caught signal!".as_bytes());
}

#[repr(C)]
struct sockaddr_in {
    sin_family : u16,   // e.g. AF_INET
    sin_port: u16,     // e.g. htons(3490)
    sin_addr:in_addr,     // see struct in_addr, below
    sin_zero: u64  // zero this if you want to
}

#[derive(Copy,Clone)]
#[repr(C)]
struct in_addr {
    s_addr: i32  // load with inet_aton()
}

#[derive(Copy,Clone)]
#[repr(C)]
struct hostent {
    h_name: *const c_char,            /* official name of host */
    h_aliases:*const *const c_char,         /* alias list */
    h_addrtype:c_int,        /* host address type */
    h_length:c_int,          /* length of address */
    h_addr_list:*const *const in_addr,       /* list of addresses */
}

#[derive(PartialEq,Eq,Clone)]
enum State{
    NotInitialized,
    Initialized,
    Idle,
    Listening,  
    Connecting,
    Connected,
    Writing,
    Reading,
    PeerClosed,
    Error(String)
}

pub struct S;
struct ReadPipe;
pub struct WritePipe{
    sd: i32
}

impl WritePipe{
    pub fn write(&self)
    {
        unsafe {
        let buf = ['a' as u8];
        let buf = buf.as_ptr();
        write(self.sd,buf,1);
        }
    }
}

pub type PtrSocket= Arc<RefCell<ESocket>>;
static mut seq_:u64 = 0;
pub struct Socket{
    pub sd: i32,
    state: State,
    pub rd_buf: Vec<u8>,
    pub rd_buf_pos: usize,
    wr_buf: Vec<u8>,
    wr_buf_pos: usize,
    pub id_:u64,
    pub pimpl: Box<SocketInterface>,
    freed: bool
}

const SSL_ERROR_NONE:i32 = 0;
const SSL_ERROR_WANT_READ:i32 = 2;
const SSL_ERROR_WANT_WRITE:i32 = 3;
const SSL_ERROR_ZERO_RETURN:i32 = 6;
const SSL_FILETYPE_PEM:i32 = 1;

//#[link(name="crypto")]
//#[link(name="ssl")]
extern "C" {
    fn rust_SSL_library_init();
    fn rust_OpenSSL_add_all_algorithms();
    fn rust_SSL_load_error_strings();
    fn SSL_CTX_new(method:*const SSL_METHOD)->*mut SSL_CTX;
    fn SSL_CTX_free(ctx:*mut SSL_CTX);
    fn rust_SSLv23_method()->*const SSL_METHOD;
    fn SSL_new(ctx:*mut SSL_CTX)->*mut SSL;
    fn SSL_free(ssl:*mut SSL);
    fn SSL_read(ssl:*mut SSL,buf:*mut u8,num: i32)->i32;
    fn SSL_write(ssl:*mut SSL,buf:*const u8,num: i32)->i32;
    fn SSL_set_accept_state(ssl:*mut SSL);
    fn SSL_set_connect_state(ssl:*mut SSL);
    fn SSL_get_error(ssl:*mut SSL,rc:i32)->i32;
    fn SSL_set_fd(ssl:*mut SSL,fd:i32)->i32;
    fn ERR_error_string_n(e:usize,buf:*mut u8,len:usize);
    fn ERR_get_error()->usize;

    fn SSL_CTX_load_verify_locations(ctx:*mut SSL_CTX,cert:*const i8,key:*const i8)->i32;
    fn SSL_CTX_set_default_verify_paths(ctx:*mut SSL_CTX)->i32;
    fn SSL_CTX_use_certificate_file(ctx:*mut SSL_CTX,cert:*const i8,typ:i32)->i32;
    fn SSL_CTX_use_PrivateKey_file(ctx:*mut SSL_CTX,cert:*const i8,typ:i32)->i32;
    fn SSL_CTX_check_private_key(ctx:*mut SSL_CTX)->i32;
}

pub fn ssl_init() {
    unsafe{
    rust_SSL_library_init();
    rust_OpenSSL_add_all_algorithms();
    rust_SSL_load_error_strings();
    }
}


#[repr(C)]
pub struct SSL_METHOD;

#[repr(C)]
pub struct SSL_CTX;

impl SSL_CTX{
    pub fn new()->Result<*mut SSL_CTX,String>{
        unsafe { 
            let rc = SSL_CTX_new(rust_SSLv23_method());
            if rc.is_null() {
                Err(ssl_get_error())
            } else {
                Ok(rc)
            }
        }
    }
    pub fn load_certificates(self:*mut SSL_CTX,cert:&str,key:&str)->Result<(),String>{
        unsafe {
            let cert = CString::new(cert).unwrap();
            let key = CString::new(key).unwrap();
        if SSL_CTX_load_verify_locations(self,cert.as_ptr(),key.as_ptr()) != 1 {
            return Err(ssl_get_error())
        }
        if SSL_CTX_set_default_verify_paths(self) != 1 {
            return Err(ssl_get_error())
        }
        if SSL_CTX_use_certificate_file(self,cert.as_ptr(),SSL_FILETYPE_PEM) != 1 {
            return Err(ssl_get_error())
        }
        if SSL_CTX_use_PrivateKey_file(self,key.as_ptr(),SSL_FILETYPE_PEM) != 1 {
            return Err(ssl_get_error())
        }
        if SSL_CTX_check_private_key(self) != 1 {
            return Err(ssl_get_error())
        }
        }
        Ok(())
    }
}

fn ssl_get_error()->String {
    unsafe {
    let error = ERR_get_error();
    let mut buf = [0;1024];
    ERR_error_string_n(error,buf.as_mut_ptr(),buf.len());
    utils::from_cstr(&buf)
    }
}
#[repr(C)]
pub struct SSL;
pub enum ESocket{
    Socket{s_:Socket},
    SSLSocket{
        s_: Socket,
        ctx_:*mut SSL_CTX,
        ssl_:*mut SSL,
        rc_: bool,
        rccode_: i32
    }
}
unsafe impl Send for ESocket{}
unsafe impl Send for Socket {}
impl Drop for Socket{
    fn drop(&mut self)
    {
        assert!(!self.freed);
        self.close();
        self.freed = true;
    }
}

impl Drop for ESocket{
    fn drop(&mut self){
        match *self {
            ESocket::SSLSocket{ssl_,..} => unsafe{
                SSL_free(ssl_);
            }
            _ => ()
        }
    }
}

pub fn sh(){
    unsafe { signal(SIGPIPE,handler); }
}
pub fn c_time()-> String {
    unsafe {
        let mut raw = 0;
        time(&mut raw);
        let mut buf = [0;256];
        CStr::from_ptr(ctime_r(&raw,buf.as_mut_ptr())).to_string_lossy().into_owned()
    }
}

fn set_nonblocking(sd: i32) {
    unsafe {
        let flag = fcntl(sd,F_GETFL);
        fcntl(sd,F_SETFL,flag | O_NONBLOCK);
    }
}

pub fn get_peer_name(sd:i32)->String
{
    unsafe{
        let mut addr = sockaddr_in{
            sin_family : 0,
            sin_port : 0,
            sin_addr : in_addr{ s_addr: 0 },
            sin_zero : 0
        };
        let mut len : i32 = 16;
        if getpeername(sd,&mut addr,&mut len) < 0 {
            "error:".to_string()+&strerror(errno())
        }
        else {
            CStr::from_ptr(inet_ntoa(addr.sin_addr)).to_string_lossy().into_owned()
        }
    }
}

pub fn get_sock_name(sd:i32)->String
{
    unsafe{
        let mut addr = sockaddr_in{
            sin_family : 0,
            sin_port : 0,
            sin_addr : in_addr{ s_addr: 0 },
            sin_zero : 0
        };
        let mut len : i32 = 16;
        if getsockname(sd,&mut addr,&mut len) < 0 {
            "error:".to_string()+&strerror(errno())
        }
        else {
            CStr::from_ptr(inet_ntoa(addr.sin_addr)).to_string_lossy().into_owned()
        }
    }
}

pub fn get_host_by_addr(addr:&str)->String {
    unsafe {
        let ia = in_addr{ s_addr:0};
        let top = CString::new(addr).unwrap();
        inet_aton(top.as_ptr(),&ia);
        let ent = gethostbyaddr(&ia,4,AF_INET);
        if ent as usize == 0 {
            h_strerror(h_errno()).to_owned()
        } else {
            CStr::from_ptr((*ent).h_name).to_string_lossy().into_owned()
        }
    }
}

impl ESocket {
    pub fn new()->ESocket {
        ESocket::Socket{s_:Socket::new()}
    }
    pub fn new_impl(pimpl: Box<SocketInterface>)->ESocket{
        ESocket::Socket{s_:Socket::new_impl(pimpl)}
    }
    pub fn ssl_new_impl(ctx:*mut SSL_CTX,pimpl: Box<SocketInterface>)->ESocket{
        let mut rc = ESocket::SSLSocket{
            s_:Socket::new_impl(pimpl),
            ctx_:ctx,
            ssl_: std::ptr::null_mut(),
            rc_:false,
            rccode_:0
        };
        if let ESocket::SSLSocket{ref mut ssl_,..} = rc {
           *ssl_ = unsafe { SSL_new(ctx) };
            if (*ssl_).is_null() { panic!("ssl allocation failed"); }
        }
        rc
    }
    fn new_args(sd:i32,state: State,pimpl: Box<SocketInterface>)->ESocket{
        ESocket::Socket{s_:Socket::new_args(sd,state,pimpl)}
    }
    fn ssl_new_args(ctx:*mut SSL_CTX,sd:i32,state: State,pimpl: Box<SocketInterface>)->ESocket{
        let mut rc = ESocket::SSLSocket{
            s_:Socket::new_args(sd,state,pimpl),
            ctx_:ctx,
            ssl_: std::ptr::null_mut(),
            rc_:false,
            rccode_:0
        };
        if let ESocket::SSLSocket{ref mut ssl_,ref s_,ctx_,..} = rc {
            unsafe {*ssl_ = SSL_new(ctx) };
            if (*ssl_).is_null() { panic!("ssl allocation failed"); }
            if ctx_.is_null() { panic!("ctx is null!"); }
            unsafe { SSL_set_fd(*ssl_, s_.sd); }
        }
        rc
    }
    pub fn set_impl(&mut self, pimpl: Box<SocketInterface>){
        match self {
            ESocket::Socket{s_,..} => s_.pimpl = pimpl,
            ESocket::SSLSocket{s_,..} => s_.pimpl = pimpl,
        }
    }
    fn socket(&self)->&Socket{
        match *self {
            ESocket::Socket{ref s_,..} => unsafe {
                let ptr:*const _;
                ptr = s_;
                &*ptr
            },
            ESocket::SSLSocket{ref s_,..} => unsafe {
                let ptr:*const _;
                ptr = s_;
                &*ptr
            },
        }
    }
    pub fn connect(&mut self,addr:&str)->Result<bool,String>{
        match self {
            ESocket::Socket{s_,..} => s_.connect(addr),
            ESocket::SSLSocket{s_,..} => s_.connect(addr),
        }
    }
    pub fn listen(&mut self,port: &str)->Result<(),String>{
        match self {
            ESocket::Socket{s_,..} => s_.listen(port),
            ESocket::SSLSocket{s_,..} => s_.listen(port),
        }
    }
    pub fn accept(&self)->Result<PtrSocket,String>{
    unsafe {
        let this = self.socket();
        if this.state != State::Listening {
            return Err("accept: not listening".to_string());
        }
        let mut svraddr : sockaddr_in = sockaddr_in{
            sin_family : 0,
            sin_port : 0,
            sin_addr : in_addr{ s_addr: 0},
            sin_zero : 0
        };
        let mut len:u32 = 16;
        let sd = accept(this.sd,&mut svraddr,&mut len);
        if sd < 0 {
            Err("accept error:".to_string()+&strerror(errno()))
        } else {
            set_nonblocking(sd);
            match *self {
                ESocket::Socket{ref s_,..} => 
                    Ok(Arc::new(RefCell::new(ESocket::new_args(sd,State::Connected,s_.pimpl.clone())))),
                ESocket::SSLSocket{ref s_,ctx_,..} => {
                    Ok(Arc::new(RefCell::new(ESocket::ssl_new_args(ctx_,sd,State::Connected,s_.pimpl.clone()))))
                },
            }
        }
    }
    }
    pub fn read(&mut self,size: usize){
        match self {
            ESocket::Socket{s_,..} => s_.read(size),
            ESocket::SSLSocket{s_,..} => s_.read(size),
        }
    }
    pub fn write(&mut self,buf: Option<Vec<u8>>){
        match self {
            ESocket::Socket{s_,..} => s_.write(buf),
            ESocket::SSLSocket{s_,..} => s_.write(buf),
        }
    }
    pub fn close(&mut self){
        match self {
            ESocket::Socket{s_,..} => s_.close(),
            ESocket::SSLSocket{s_,..} => s_.close(),
        }
    }
    fn initialize(&mut self, family: i32, tp: i32)->Result<(),String>{
        match self {
            ESocket::Socket{s_,..} => s_.initialize(family,tp),
            ESocket::SSLSocket{s_,..} => s_.initialize(family,tp),
        }
    }
}

impl Socket {
    pub fn new()->Socket
    {
        unsafe {
        seq_ += 1;
        Socket{
            sd: -1,
            state: State::NotInitialized,
            rd_buf: vec![0;512],
            rd_buf_pos: 0,
            wr_buf: vec![],
            wr_buf_pos: 0,
            id_ : seq_,
            pimpl: Box::new(S),
            freed : false
        }
        }
    }
    pub fn new_impl(pimpl: Box<SocketInterface>)->Socket
    {
        unsafe {
        seq_ += 1;
        Socket{
            sd: -1,
            state: State::NotInitialized,
            rd_buf: vec![0;512],
            rd_buf_pos: 0,
            wr_buf: vec![],
            wr_buf_pos: 0,
            id_ : seq_,
            pimpl: pimpl,
            freed: false
        }
        }
    }
    fn new_args(sd:i32,state: State,pimpl: Box<SocketInterface>)->Socket
    {
        unsafe {
        seq_ += 1;
        Socket{
            sd: sd,
            state: state,
            rd_buf: vec![0;512],
            rd_buf_pos: 0,
            wr_buf: vec![],
            wr_buf_pos: 0,
            id_ : seq_,
            pimpl: pimpl,
            freed: false
        }
        }
    }
    pub fn set_impl(&mut self, pimpl: Box<SocketInterface>)
    {
        self.pimpl = pimpl;
    }
    pub fn pair()->Result<(Box<Socket>,WritePipe),String>
    {
        unsafe {
            let mut buf = [0,0];
            let p = buf.as_mut_ptr();
            if socketpair(AF_LOCAL,SOCK_STREAM,0,p) < 0
            {
                Err(strerror(errno()).to_owned())
            }
            else
            {
                let rs = Socket::new_args(buf[0],State::Reading,Box::new(ReadPipe));
                set_nonblocking(rs.sd);
                let wp = WritePipe{ sd:buf[1] };
                set_nonblocking(wp.sd);
                Ok((Box::new(rs),wp))
            }
        }
    }
    pub fn pair_sd()->Result<[i32;2],String>
    {
        unsafe {
            let mut buf = [0,0];
            let p = buf.as_mut_ptr();
            if socketpair(AF_LOCAL,SOCK_STREAM,0,p) < 0
            {
                Err(strerror(errno()).to_owned())
            }
            else
            {
                set_nonblocking(buf[0]);
                set_nonblocking(buf[1]);
                Ok(buf)
            }
        }
    }
    pub fn host_by_name(name:&str)->Result<(String,Vec<String>),String>{
        unsafe {
        let tmp = CString::new(name).unwrap();
        let s = tmp.as_ptr();
            let host:*const hostent = gethostbyname(s);
            if host as usize == 0 
            { 
                Err(h_strerror(h_errno()).to_owned())
            }
            else
            {
                let host = *host;
                let name = CStr::from_ptr(host.h_name);
                let mut res = vec![];
                for i in (0..).step_by(mem::size_of::<*const in_addr>()) {
                    let index = (host.h_addr_list as usize + i) as *const *const in_addr;
                    if *index as usize == 0 {
                        break;
                    }
                    let index = **index;
                    res.push(CStr::from_ptr(inet_ntoa(index)).to_string_lossy().into_owned());
                }
                Ok((name.to_string_lossy().into_owned(),res))
            }
        }
    }
    pub fn connect(&mut self,addr:&str)->Result<bool,String>
    {
    unsafe {
        match self.initialize(AF_INET,SOCK_STREAM)
        {
            Err(s) => return Err(s),
            Ok(_) => ()
        }
        let mut svraddr = sockaddr_in{
            sin_family : AF_INET as u16,
            sin_port : 0,
            sin_addr : in_addr{ s_addr: INADDR_ANY },
            sin_zero : 0
        };
        let ap:Vec<&str> = addr.split(':').collect();
        match &ap[..] {
            &[a,p] => {
                let s = CString::new(a).unwrap();
                    if inet_aton(s.as_ptr(),&svraddr.sin_addr) == 0
                    {
                        Err("connect: invalid address: ".to_string()+&strerror(errno()))
                    }
                    else
                    {
                        svraddr.sin_port = htons(u16::from_str(p).unwrap());
                        if connect(self.sd,&svraddr,16) < 0{
                            if errno() as i32 != EINPROGRESS {
                                self.state = State::Error(strerror(errno()));
                                Err("connect : ".to_string()+&strerror(errno()))
                            }
                            else {
                                self.state = State::Connecting;
                                Ok(false)
                            }
                        }
                        else
                        {
                            self.state = State::Connected;
                            Ok(true)
                        }
                    }
            }
            _ => Err("connect: Invalid address".to_string())
        }
    }
    }
    pub fn listen(&mut self,port: &str)->Result<(),String>
    {
    unsafe{
        match self.initialize(AF_INET,SOCK_STREAM)
        {
            Err(s) => return Err(s),
            Ok(_) => ()
        }
        let mut svraddr = sockaddr_in{
            sin_family : AF_INET as u16,
            sin_port : htons(u16::from_str(port).unwrap()),
            sin_addr : in_addr{ s_addr: INADDR_ANY },
            sin_zero : 0
        };
        if setsockopt(self.sd,SOL_SOCKET,SO_REUSEADDR,&mut svraddr,16) < 0{
            return Err("listen: setsockopt failed:".to_string()+&strerror(errno()));
        }
        if bind(self.sd,&svraddr,16) < 0 {
            return Err("listen: bind failed:".to_string()+&strerror(errno()));
        }
        if listen(self.sd,128) < 0 {
            return Err("listen: listen failed:".to_string()+&strerror(errno()));
        }
        self.state = State::Listening;
        Ok(())
    }
    }
    pub fn read(&mut self,size: usize)
    {
        self.rd_buf.resize(size,0);
        self.rd_buf_pos = 0;
        self.state = State::Reading;
    }
    pub fn write(&mut self,buf:Option<Vec<u8>>)
    {
        if !buf.is_none() {
            self.wr_buf = buf.unwrap();
        }
        self.wr_buf_pos = 0;
        self.state = State::Writing;
    }
    pub fn close(&mut self)
    {
        unsafe{
            if self.state != State::NotInitialized {
                close(self.sd);
            }
            self.state = State::NotInitialized;
            self.sd = -1;
        }
    }
    fn initialize(&mut self, family: i32, tp: i32)->Result<(),String>
    {
        unsafe{
            if self.state != State::NotInitialized{
                close(self.sd);
            }
            let sd = socket(family,tp,0);
            if sd < 0 {
                Err("initialize failed ".to_string()+&strerror(errno()))
            }
            else {
                set_nonblocking(sd);
                self.sd = sd;
                self.state = State::Initialized;
                Ok(())
            }
        }
    }
}

pub trait SocketInterface:Downcast{
    fn clone(&self)->Box<SocketInterface>;
    fn get_service(&mut self)->Option<&mut Service> {
        None
    }
    fn done_connected(&mut self,s:PtrSocket,_:&mut EPoll)->Result<Option<PtrSocket>,String>
    {
        Ok(Some(s))
    }
    fn done_reading(&mut self,s:PtrSocket,_:&mut EPoll)->Result<Option<PtrSocket>,String>
    {
        Ok(Some(s))
    }
    fn done_writing(&mut self,s:PtrSocket,_:&mut EPoll)->Result<Option<PtrSocket>,String>
    {
        Ok(Some(s))
    }
    fn handle_error(&mut self,s:PtrSocket,_: &mut EPoll,e:String)->Result<Option<PtrSocket>,String>
    {
        Err(e)
    }
    fn done_closed(&mut self,_:PtrSocket,_:&mut EPoll)
    {
    }
    fn tick(&mut self)->bool {
        return false;
    }
    fn keep(&mut self)->bool { return false; }
}
impl_downcast!(SocketInterface);
pub trait IGetPtr {
    fn done_connected(&self,_:&mut EPoll)->Result<Option<PtrSocket>,String>;
    fn done_reading(&self,_:&mut EPoll)->Result<Option<PtrSocket>,String>;
    fn done_writing(&self,_:&mut EPoll)->Result<Option<PtrSocket>,String>;
    fn handle_error(&self,_: &mut EPoll)->Result<Option<PtrSocket>,String>;
    fn done_closed(&self,_:&mut EPoll);
    fn tick(&self)->bool;
    fn keep(&self)->bool;
    fn get_service(&self)->Option<&mut Service>;
    fn socket(&self)->&mut Socket;
}

impl IGetPtr for PtrSocket {
    fn socket(&self)->&mut Socket{
        let mut s = self.borrow_mut();
        match *s {
            ESocket::Socket{ref mut s_,..} => unsafe {
                let ptr:*mut _;
                ptr = s_;
                &mut *ptr
            },
            ESocket::SSLSocket{ref mut s_,..} => unsafe {
                let ptr:*mut _;
                ptr = s_;
                &mut *ptr
            },
        }
    }
    fn done_connected(&self,epoll:&mut EPoll)->Result<Option<PtrSocket>,String>{
        let this:*mut Socket;
        this = self.socket();
        unsafe { (*this).pimpl.done_connected(self.clone(),epoll) }
    }
    fn done_reading(&self,epoll:&mut EPoll)->Result<Option<PtrSocket>,String>{
        let this:*mut Socket;
        this = self.socket();
        unsafe { (*this).pimpl.done_reading(self.clone(),epoll) }
    }
    fn done_writing(&self,epoll:&mut EPoll)->Result<Option<PtrSocket>,String>{
        let this:*mut Socket;
        this = self.socket();
        unsafe { (*this).pimpl.done_writing(self.clone(),epoll) }
    }
    fn handle_error(&self,epoll: &mut EPoll)->Result<Option<PtrSocket>,String>{
        let this:*mut Socket;
        this = self.socket();
        unsafe {
        let e = match (*this).state {
            State::Error(ref e) => e.clone(),
            _ => "None".to_string()
        };
        (*this).pimpl.handle_error(self.clone(),epoll,e) }
    }
    fn done_closed(&self,epoll:&mut EPoll){
        let this:*mut Socket;
        this = self.socket();
        unsafe { (*this).pimpl.done_closed(self.clone(),epoll) }
    }
    fn get_service(&self)->Option<&mut Service>{
        let this:*mut Socket;
        this = self.socket();
        unsafe { (*this).pimpl.get_service() }
    }
    fn tick(&self)->bool{
        self.socket().pimpl.tick()
    }
    fn keep(&self)->bool{
        self.socket().pimpl.keep()
    }
}
impl SocketInterface for S{
    fn clone(&self)->Box<SocketInterface>
    {
        Box::new(S)
    }
}
impl SocketInterface for ReadPipe{
    fn clone(&self)->Box<SocketInterface>
    {
        Box::new(ReadPipe)
    }
    fn done_reading(&mut self,s:PtrSocket,p:&mut EPoll)->Result<Option<PtrSocket>,String>
    {
        p.read(s,512);
        Ok(None)
    }
    fn handle_error(&mut self,_:PtrSocket,_:&mut EPoll,e:String)->Result<Option<PtrSocket>,String>
    {
        Err(e)
    }
}

#[repr(C)]
#[repr(packed)]
#[derive(Clone,Copy)]
struct epoll_event{
    events:u32,      /* Epoll events */
    data:u64       /* User data variable */
}
use std::sync::Mutex;

static mut lstEPollM_:threads::Mutex = threads::Mutex{m_:0};
static mut lstEPoll_: Option<Vec<*mut EPoll>> = None;
pub struct EPoll{
    maxdone:usize,
    epollfd:i32,
    done: Vec<epoll_event>,
    wake_ : [i32;2],
    pub lst_done: LinkedList<PtrSocket>,
    lst_pending: HashMap<u64,PtrSocket>,
}

pub trait Callback {
    fn conn(&mut self,s:PtrSocket);
    fn call(&mut self,s:PtrSocket);
}

impl EPoll{
    pub fn new(maxdone:usize)->Box<EPoll>{
        unsafe {
        let mut rc = EPoll{
            maxdone:maxdone,
            epollfd:epoll_create(10),
            done: vec![epoll_event{events:0,data:0};maxdone],
            wake_:Socket::pair_sd().unwrap(),
            lst_done: LinkedList::new(),
            lst_pending: HashMap::new(),
        };
        if lstEPoll_.is_none() { lstEPoll_ = Some(Vec::new()); }
        let m = threads::AutoLock::new(&mut lstEPollM_);
        let mut lst = server::get_lst(&mut lstEPoll_);
        let ev = epoll_event {events:EPOLLIN,data:rc.wake_[1] as u64};
        epoll_ctl(rc.epollfd,EPOLL_CTL_ADD,rc.wake_[1],&ev);
        let mut rc = Box::new(rc);
        lst.push(&mut *rc);
        rc
        }
    }
    pub fn pending(&self)->usize{
        self.lst_pending.len()
    }
    pub fn s_pending(&self,s:PtrSocket)->bool{
        let s = s.socket();
        self.lst_pending.contains_key(&(s.sd as u64))
    }
    pub fn accept(&mut self,sock:PtrSocket)->Result<(),String>{
    unsafe {
        let rsock = sock.socket();
        let mut ev = epoll_event{
            events:0,
            data: rsock.sd as u64
        };
        ev.events = match rsock.state {
            State::Listening
            | State::Reading => EPOLLIN,
            State::Writing => EPOLLOUT,
            _ => EPOLLIN | EPOLLOUT
        };
        match *sock.borrow() {
            ESocket::SSLSocket{rc_,rccode_,..} => {
                if rc_ {
                    ev.events = match rccode_ {
                        SSL_ERROR_WANT_WRITE => EPOLLOUT,
                        SSL_ERROR_WANT_READ => EPOLLIN,
                        _ => EPOLLIN | EPOLLOUT,
                    }
                }
            },
            _ => (),
        }
        ev.events |= EPOLLET | EPOLLONESHOT;
        let mut rc = epoll_ctl(self.epollfd,EPOLL_CTL_MOD,rsock.sd,&ev);
        
        if rc < 0 {
            rc = epoll_ctl(self.epollfd,EPOLL_CTL_ADD,rsock.sd,&ev);
            
            if rc < 0 {
                return Err("Epoll accept: failed :".to_string()+&strerror(errno()));
            }
        }
        self.lst_pending.insert(rsock.sd as u64,sock.clone());
        Ok(())
    }
    }
    pub fn connect(&mut self,sock:PtrSocket,addr:&str)->Result<(),String>
    {
        let res;
        {
            let mut rsock = sock.borrow_mut();
            res = rsock.connect(addr);
        }
        match res {
            Ok(r) => if !r { self.accept(sock) }
                     else { self.lst_done.push_back(sock); Ok(()) },
            Err(s) => Err(s)         
        }
    }
    pub fn read(&mut self,sock:PtrSocket,size:usize)->Result<(),String>
    {
        {
            let mut rsock = sock.borrow_mut();
            rsock.read(size);
        }
        match self.read_imm(sock) {
            Some(sock) => self.accept(sock),
            None => Ok(())
        }
    }
    pub fn write(&mut self,sock:PtrSocket,buf:Option<Vec<u8>>)->Result<(),String>
    {
        {
            let mut rsock = sock.borrow_mut();
            rsock.write(buf);
        }
        match self.write_imm(sock) {
          Some(sock) => self.accept(sock),
          None => Ok(())
        }
    }
    pub fn done(&mut self)->Option<PtrSocket>
    {
        self.lst_done.pop_front()
    }
    pub fn remove(&mut self,sock:PtrSocket)
    {
        unsafe {
        let rsock = sock.socket();
        let ev = epoll_event { events: 0, data: 0 };
        epoll_ctl(self.epollfd,EPOLL_CTL_DEL,rsock.sd,&ev);
        self.lst_pending.remove(&(rsock.sd as u64));
        }
    }
    fn push_done(&mut self,sock:PtrSocket)
    {
        unsafe {
        let rsock = sock.socket();
        let ev = epoll_event { events: 0, data: rsock.sd as u64 };
        epoll_ctl(self.epollfd,EPOLL_CTL_DEL,rsock.sd,&ev);
        self.lst_done.push_back(sock.clone());
        }
    }
    pub fn perform(&mut self, ret:bool,to:i32)->Result<bool,String>
    {
    unsafe {
        if ret && self.lst_pending.is_empty() { return Ok(false); }
        let np = epoll_wait(self.epollfd,self.done.as_mut_ptr(),self.maxdone as i32,to);
        if np < 0{
            return Err("epoll failed:".to_string()+&strerror(errno()));
        }
        for i in 0..np
        {
            let sd = self.done[i as usize].data;
            if sd as i32 == self.wake_[1] {
                let mut buf = [0;256];
                while read(self.wake_[1],buf.as_mut_ptr(),256)>0 {}
                continue;
            }
            let sock = self.lst_pending.remove(&sd).unwrap();
            let state = sock.socket().state.clone();
            match state{
                State::Listening => {
                    loop {
                        match sock.borrow_mut().accept() {
                            Ok(s) => {
                                match *s.borrow_mut() {
                                    ESocket::SSLSocket{ssl_,..} => SSL_set_accept_state(ssl_),
                                    _ => (),
                                }
                                self.lst_done.push_back(s);
                            }
                            Err(_) =>{ break; }
                        }
                    }
                    let _ = self.accept(sock);
                },
                State::Connecting => {
                    let mut len:i32 = 4;
                    let mut error:i32 = 0;
                    let sd;
                    {
                        sd = sock.socket().sd;
                    }
                    sock.socket().state = if getsockopt(sd, SOL_SOCKET, SO_ERROR,&mut error,&mut len) < 0
                                 || error > 0
                                {
                                    State::Error(strerror(error))
                                }
                                else
                                {
                                    match *sock.borrow_mut() {
                                        ESocket::SSLSocket{ref s_,ssl_,..} => {
                                            SSL_set_fd(ssl_,s_.sd);
                                            SSL_set_connect_state(ssl_);
                                        }
                                        _ => ()
                                    };
                                    State::Connected

                                };
                    self.push_done(sock);
                },
                State::Reading => {
                    match self.read_imm(sock) {
                        Some(sock) => { let _ = self.accept(sock); }
                        None => ()
                    }
                },
                State::Writing => {
                    match self.write_imm(sock) {
                        Some(sock) => { let _ = self.accept(sock); }
                        None => ()
                    }
                },
                _ => {
                    sock.socket().state = State::Idle;
                    self.push_done(sock);
                }
            };
        }
        let sds:Vec<_> = self.lst_pending.iter().map(|(k,v)| *k).collect();
        for sd in sds {
            let mut rc;
            {
                let mut s = self.lst_pending.get_mut(&sd).unwrap();
                rc = s.tick();
            }
            if rc {
                let s = self.lst_pending.remove(&sd).unwrap();
                self.push_done(s);
            }
        }
        Ok(true)
    }
    }
    fn write_imm(&mut self, sock:PtrSocket)->Option<PtrSocket>
    {
        let rp:*mut _;
        {
            rp = &mut *sock.borrow_mut();
        }
        unsafe {
        match *rp {
        ESocket::Socket{s_:ref mut rsock,..} => unsafe {
            let buf = (*rsock).wr_buf.as_ptr();
            let buf = (buf as usize + (*rsock).wr_buf_pos) as *const u8;
            let r = write((*rsock).sd,buf,(*rsock).wr_buf.len()-(*rsock).wr_buf_pos);
                    
            if r < 0 {
                if errno() != EAGAIN {
                    (*rsock).state = State::Error(strerror(errno()));
                    self.push_done(sock);
                    return None;
                }
            } else {
                if r > 0 {
                    (*rsock).wr_buf_pos += r as usize;
                };
                if (*rsock).wr_buf_pos == (*rsock).wr_buf.len() || r == 0 {
                    (*rsock).wr_buf = vec![];
                    (*rsock).wr_buf_pos = 0;
                    self.push_done(sock);
                    return None;
                }
            }
        }    
        ESocket::SSLSocket{s_:ref mut rsock,ssl_,ref mut rc_,ref mut rccode_,..}=>
            unsafe{
            let buf = (*rsock).wr_buf.as_ptr();
            let buf = (buf as usize + (*rsock).wr_buf_pos) as *const u8;
            let rc = SSL_write(ssl_,buf, ((*rsock).wr_buf.len() - (*rsock).wr_buf_pos) as i32);
                let error = SSL_get_error(ssl_,rc);
                match error {
                    SSL_ERROR_NONE => {
                        (*rsock).wr_buf_pos += rc as usize;
                        *rc_ = false;
                    },
                    SSL_ERROR_WANT_READ | SSL_ERROR_WANT_WRITE => {
                        *rc_ = true;
                        *rccode_ = error;
                    },
                    SSL_ERROR_ZERO_RETURN => {
                        (*rsock).state = State::PeerClosed;
                        *rc_ = false;
                    },
                    _ => {
                        let mut buf = vec![0;1024];
                        ERR_error_string_n(error as usize,buf.as_mut_ptr(),buf.len());
                        (*rsock).state = State::Error(std::string::String::from_utf8_unchecked(buf));
                        *rc_ = false;
                    },
                }
                if !*rc_ {
                    self.push_done(sock);return None
                }
            }
        }
        }
        Some(sock)
    }
    fn read_imm(&mut self,sock:PtrSocket)->Option<PtrSocket>
    {
        let rp:*mut _;
        {
            rp = &mut *sock.borrow_mut();
        }
        unsafe {
        match *rp {
            ESocket::Socket{s_:ref mut rsock,..} => unsafe {
                let r = read((*rsock).sd,(*rsock).rd_buf.as_mut_ptr(), (*rsock).rd_buf.len());
                let errno = match r {
                    0 => { (*rsock).state = State::PeerClosed; 0 }
                    y if y > 0 => { (*rsock).rd_buf_pos += r as usize; 0 }
                    _ => { 
                        if errno() != EAGAIN as i32 { (*rsock).state = State::Error(strerror(errno()));0 }
                        else { errno() }
                    }
                };
                if errno == EAGAIN as i32 { Some(sock) }
                else { self.push_done(sock);None }
            },
            ESocket::SSLSocket{s_:ref mut rsock,ssl_,ref mut rc_,ref mut rccode_,..}=>unsafe {
                let buf = (*rsock).rd_buf.as_mut_ptr();
                let buf = (buf as usize + (*rsock).rd_buf_pos) as *mut u8;
                let rc = SSL_read(ssl_,buf,((*rsock).rd_buf.len() - (*rsock).rd_buf_pos) as i32);
                let error = SSL_get_error(ssl_,rc);
                match error {
                    SSL_ERROR_NONE => {
                        (*rsock).rd_buf_pos += rc as usize;
                        *rc_ = false;
                    },
                    SSL_ERROR_WANT_READ | SSL_ERROR_WANT_WRITE => {
                        *rc_ = true;
                        *rccode_ = error;
                    },
                    SSL_ERROR_ZERO_RETURN => {
                        (*rsock).state = State::PeerClosed;
                        *rc_ = false;
                    },
                    _ => {
                        let mut buf = vec![0;1024];
                        ERR_error_string_n(error as usize,buf.as_mut_ptr(),buf.len());
                        (*rsock).state = State::Error(std::string::String::from_utf8_unchecked(buf));
                        *rc_ = false;
                    },
                }
                if *rc_ {
                    Some(sock)
                } else {
                    self.push_done(sock);None
                }
            },
        }}
    }
    pub fn put(&mut self,l:&mut LinkedList<PtrSocket>) {
        let sds:Vec<_> = self.lst_pending.iter().map(|(k,v)| *k).collect();
        for i in sds {
            if !self.lst_pending.get_mut(&i).unwrap().keep() {
                let mut s = self.lst_pending.remove(&i).unwrap();
                self.remove(s.clone());
                l.push_back(s);
            }
        }
    }
    pub fn get(&mut self,l:&mut LinkedList<PtrSocket>,all:bool) {
        let size = self.lst_pending.len();
        while all || self.lst_pending.len() < self.maxdone + size {
            if l.is_empty() { break }
            let s = l.pop_front().unwrap();
            self.accept(s);
        }
    }
    fn wake(&self) {
        unsafe{ write(self.wake_[0],"1".as_ptr(),1); }
    }
    pub fn wakeAll(&self) {
        unsafe {
        let m = threads::AutoLock::new(&mut lstEPollM_);
        let lst = server::get_lst(&mut lstEPoll_);
        for i in lst.iter() {
            if (*i) as *const _ != self as *const _ {
                (**i).wake();
            }
        }}
    }
    fn lp(slf:&mut EPoll,mut c:&mut Option<&mut Callback>) {
            loop {
            match slf.done() {
                Some(s) =>{
                    let state = s.socket().state.clone();
                    let res = match state {
                        State::Connected => {
                            if !c.is_none() { get_lst(&mut c).conn(s.clone()); }
                            s.done_connected(slf)
                        },
                        State::Reading => s.done_reading(slf),
                        State::Writing => s.done_writing(slf),
                        State::Error(_) => s.handle_error(slf),
                        _ => { 
                            slf.remove(s.clone());
                            s.done_closed(slf);
                            Ok(None)
                        }
                    };
                    match res {
                        Err(s) => { std::io::stderr().write((s+"\r\n").as_bytes()); },
                        Ok(s) => {
                            match s {
                               Some(s) => if !c.is_none() { get_lst(&mut c).call(s) } ,
                               None => continue
                            }
                        }
                    }
                },
                None => break
            }
            }
        }
    pub fn run(&mut self, n:i32,mut c:Option<&mut Callback>,to:i32,pend:bool)->Result<(),String>
    {
        let mut cont = true;
        let mut count  = if n < 0 { -n } else { n };
        while cont && count > 0{
            EPoll::lp(self,&mut c);
            let res = self.perform(pend,to);
            EPoll::lp(self,&mut c);

            match res {
                Ok(s) => cont = s,
                Err(s) => return Err(s)
            }
            if n > 0 { count -= 1; }
        }
        Ok(())
    }

}

//#[unsafe_destructor]
impl Drop for EPoll{
    fn drop(&mut self)
    {
        unsafe {
        close(self.epollfd);
        close(self.wake_[0]);
        close(self.wake_[1]);
        let m = threads::AutoLock::new(&mut lstEPollM_);
        let mut lst = server::get_lst(&mut lstEPoll_);
        lst.iter().position(|e| *e as *const _ == self as *const _).
            map(|e| lst.remove(e));
        }
    }
}

extern "C" {
  fn __xpg_strerror_r(errnum: c_int, buf: *const libc::c_char, len: size_t) -> c_int;
  fn __errno_location() -> *const c_int;
  fn __h_errno_location() -> *const c_int;
  fn hstrerror(errnum:c_int)->*const c_char;
}

fn errno() -> i32 {
  unsafe { *__errno_location() as i32 }
}
fn h_errno() -> i32 {
  unsafe { *__h_errno_location() as i32 }
}

fn strerror(errnum: i32) -> String {
  let bufv: Vec<u8> = vec![0;1024];
  let len = bufv.len();

  unsafe {
    let buf : *const libc::c_char = bufv.as_ptr() as *const c_char;
    let r = __xpg_strerror_r(errnum as c_int, buf, len as size_t);

    if r > 0 {
      panic!(format!("strerror failed [errno={}]", errno()));
    }

    CStr::from_ptr(buf).to_string_lossy().into_owned()
  }
}
fn h_strerror(errnum: i32) -> String {
    unsafe { CStr::from_ptr(hstrerror(errnum)).to_string_lossy().into_owned() }
}

#[test]
fn test() {
  // FIXME chokes on localized messages
  assert_eq!(strerror(22) , "Invalid argument");
  assert_eq!(strerror(0) , "Success");
}

