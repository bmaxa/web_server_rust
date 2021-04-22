use super::*;
use std::collections::linked_list::LinkedList;
use std::collections::HashSet;
use super::service::Service;
use super::sockets::*;
use super::sockets::Socket;
use std::thread::*;
use std::time::*;
use std::sync::Arc;
use std::cell::RefCell;
use std::io::Write;
use std::panic::UnwindSafe;
use super::threads::*;

pub struct StatusControl {
pub    svcid: usize,
pub    shutdown: bool,
pub    serviced: usize,
pub    npending: usize,
pub    accepted: usize
}
impl StatusControl{
    fn new()->StatusControl{
        StatusControl{
            svcid : 0,
            shutdown: false,
            serviced: 0,
            npending: 0,
            accepted: 0
        }
    }
}
pub static mut lstCntrlM_:Mutex = Mutex{m_:0};
pub static mut lstCntrl_ : Option<Vec<*mut StatusControl>> = None;
static mut lstSvcM_:Mutex = Mutex{m_:0};
static mut lstSvc_ : Option<LinkedList<PtrSocket>> = None;
static mut lstSvcLowM_:Mutex = Mutex{m_:0};
static mut lstSvcLow_ : Option<LinkedList<PtrSocket>> = None;
static mut lstSocketM_:Mutex = Mutex{m_:0};
static mut lstSocket_ : Option<LinkedList<PtrSocket>> = None;
static mut threadsAvailM_:Mutex = Mutex{m_:0};
static mut threadsAvail_ : u32 = 0;
pub fn init(){
    unsafe {
    lstCntrl_ = Some(Vec::new());
    lstSvc_ = Some(LinkedList::new());
    lstSvcLow_ = Some(LinkedList::new());
    lstSocket_ = Some(LinkedList::new());
    }
}
/*
__gshared static DList!StatusControl lstCntrl_;
__gshared static Mutex lstCntrlM_;
__gshared static DList!Service lstSvc_;
__gshared static DList!Service lstSvcLow_;
__gshared static Mutex lstSvcM_;
__gshared static DList!Socket lstSocket_;
__gshared static Mutex lstSocketM_;
__gshared static uint threadsAvail_;
__gshared static Mutex availM_;
*/                                    
pub struct Server {
    pl_:Box<sockets::EPoll>,
    serviced_: i32,
    accepted_: usize,
    cntrl_:StatusControl
}

pub struct ServiceInfo {
    serviced_: usize,
    io_serviced_:usize,
    accepted_:usize,
    tv_:Duration,
    start_:Instant,
    l:Mutex
}
impl ServiceInfo {
    pub fn instance()->&'static mut ServiceInfo {
        static mut rc: Option<ServiceInfo> = None;
        unsafe {
        if rc.is_none() {
            rc = Some(ServiceInfo {
                serviced_:0,
                io_serviced_:0,
                accepted_:0,
                tv_:Duration::from_millis(0),
                start_:Instant::now(),
                l: Mutex{m_:0}
            });
        }
        get_lst(&mut rc)
        }
    }
    pub fn io_serviced(&mut self,i:usize)->usize{
            let guard = AutoLock::new(&mut self.l);
            self.io_serviced_ += i;
            self.io_serviced_
    }
    pub fn serviced(&mut self,i:usize)->usize{
            let guard = AutoLock::new(&mut self.l);
            self.serviced_ += i;
            self.serviced_
    }
    pub fn accepted(&mut self,i:usize)->usize{
            let guard = AutoLock::new(&mut self.l);
            self.accepted_ += i;
            self.accepted_
    }
    pub fn accum(&mut self,t:Duration){
            let guard = AutoLock::new(&mut self.l);
            self.tv_ = self.tv_.checked_add(t).unwrap();
            self.serviced_ += 1;
    }
    pub fn avgtime(&mut self)->f64 {
            let guard = AutoLock::new(&mut self.l);
            if self.tv_ == Duration::from_millis(0) || self.serviced_ == 0 { return 0f64; }
            return self.serviced_ as f64 / self.tv_.as_secs() as f64;
    }
    pub fn avgreqs(&mut self)->f64 {
            let guard = AutoLock::new(&mut self.l);
            let tmp = self.start_.elapsed();
            if tmp == Duration::from_millis(0) { return self.serviced_ as f64; }
            return self.serviced_ as f64 / tmp.as_secs() as f64;
    }
}

impl sockets::Callback for Server {
    fn conn(&mut self,s:PtrSocket){
        self.accepted_ += 1;
        ServiceInfo::instance().accepted(1);
    }
    fn call(&mut self,s:PtrSocket){
        unsafe {
        if self.pl_.s_pending(s.clone()) { return }
        ServiceInfo::instance().io_serviced(1);
        let lsm = AutoLock::new(&mut lstSvcM_);
        let mut lst = get_lst(&mut lstSvc_);
//        println!("added to lstsvc {}",s.borrow().id_);
        lst.push_back(s);
        }
    }
}

use std::sync::atomic::{AtomicUsize, Ordering,ATOMIC_USIZE_INIT};

pub fn get_lst<T>(lst: &mut Option<T>)->&mut T {
    match lst{
        Some(ref mut m1) => {
            m1
        },
        None => panic!("data is null!")
    }
}

fn inc_threads_avail()->u32 {
    unsafe {
    let vm = AutoLock::new(&mut threadsAvailM_);
    threadsAvail_ += 1;
    threadsAvail_
    }
}
fn dec_threads_avail()->u32 {
    unsafe {
    let vm = AutoLock::new(&mut threadsAvailM_);
    threadsAvail_ -= 1;
    threadsAvail_
    }
}
pub fn threads_avail()->u32 {
    unsafe {
    let vm = AutoLock::new(&mut threadsAvailM_);
    threadsAvail_ 
    }
}
impl Drop for Server {
    fn drop(&mut self){
        println!("drop thread {:?} pending {}",std::thread::current().id(),self.pl_.pending());
        println!("serviced {}",self.serviced_);
    }
}

static mut rtM_:Mutex = Mutex{m_:0};
static mut rt : i32 = 0;
use std::panic::AssertUnwindSafe;

impl Server {
    fn new(l:Option<Vec<ESocket>>)->Server {
        let mut rc = Server{
            pl_:EPoll::new(4000),
            serviced_:0,
            accepted_:0,
            cntrl_:StatusControl::new()
        };
        if !l.is_none() {
            for i in l.unwrap() {
                let _=rc.pl_.accept(Arc::new(RefCell::new(i)));
            }
        }
        println!("server new!");
        rc
    }
    fn call(&mut self) {
        unsafe {
            let rtl = AutoLock::new(&mut rtM_);
            rt += 1;
        }
        let mut perfcounter=0;
        let mut needed = true;
        {
            unsafe{
            let guard = AutoLock::new(&mut lstCntrlM_);
            let mut cntrl =  get_lst(&mut lstCntrl_);
            self.cntrl_ = StatusControl{
                svcid : self as *const _ as usize,
                shutdown: false,
                serviced: self.serviced_ as usize,
                npending: self.pl_.pending(),
                accepted: self.accepted_
            };
            cntrl.push(&mut self.cntrl_);
            }
        }
        while needed {
            let mut s=None;
            let mut more = true;
            while more {
                {
                    unsafe {
                    let lm = AutoLock::new(&mut lstSvcM_);
                    let mut lstsvc = get_lst(&mut lstSvc_);
                    let ll = AutoLock::new(&mut lstSvcLowM_);
                    let mut lstsvclow = get_lst(&mut lstSvcLow_);
                    let rtl = AutoLock::new(&mut rtM_);
                    if !lstsvc.is_empty() {
                        s = lstsvc.pop_front();
                    } else if (rt < 100 || threads_avail() > 3) && !lstsvclow.is_empty() {
                        s = lstsvclow.pop_front();
                    } else {
                        more = false;
                        s = None;
                        continue;
                    }
                    }
                }
                if !s.is_none() {
                    {
                        unsafe {
                        let lcm = AutoLock::new(&mut lstCntrlM_);
                        let cntrl = get_lst(&mut lstCntrl_);
                        if !self.cntrl_.shutdown && dec_threads_avail() == 0 {
                            let rtm = AutoLock::new(&mut rtM_);
                            if rt < 100 {
                                Server::start_thread(None);
                            }
                        } else if threads_avail() > 3 && self.serviced_ > 2000 {
                            self.cntrl_.shutdown = true;
                        }
                        }
                    }
                    let mut finished = false;
                    let start = Instant::now();
                    let mut s = s.unwrap();
                    let ss = s.get_service().unwrap().service(s.clone(),&mut self.pl_) ;
                    let end = start.elapsed();
                    ServiceInfo::instance().accum(end);
                    unsafe {
                    let lcm = AutoLock::new(&mut lstCntrlM_);
                    let cntrl = get_lst(&mut lstCntrl_);
                    if !self.cntrl_.shutdown { inc_threads_avail(); }
                    }
                    self.serviced_ += 1;
                    perfcounter += 1;
                    if !ss.is_none() {
                        unsafe {
                        let llm = AutoLock::new(&mut lstSvcLowM_);
                        let mut lstsvclow = get_lst(&mut lstSvcLow_);
                        let ss = ss.unwrap();
                        lstsvclow.push_back(ss);
                        }
                    } else {
                        unsafe {
                        let lsm = AutoLock::new(&mut lstSocketM_);
                        let mut lst = get_lst(&mut lstSocket_);
                        self.pl_.put(&mut lst);
                        }
                    }
                }
            }
            {
                unsafe {
                let lsm = AutoLock::new(&mut lstSocketM_);
                let mut lst = get_lst(&mut lstSocket_);
                self.pl_.get(&mut lst,false);
                }
            }
            unsafe {
            {
            let lcm = AutoLock::new(&mut lstCntrlM_);
            let cntrl = get_lst(&mut lstCntrl_);
            self.cntrl_.serviced = self.serviced_ as usize;
            self.cntrl_.npending = self.pl_.pending();
            self.cntrl_.accepted = self.accepted_;
            }
            let this:*mut Server = self;
            let _ = (*this).pl_.run(1,Some(self),1000,false);
            {
            let lcm = AutoLock::new(&mut lstCntrlM_);
            let cntrl = get_lst(&mut lstCntrl_);
            self.cntrl_.serviced = self.serviced_ as usize;
            self.cntrl_.npending = self.pl_.pending();
            self.cntrl_.accepted = self.accepted_;
            }
            {
                let lsm = AutoLock::new(&mut lstSocketM_);
                let mut lst = get_lst(&mut lstSocket_);
                self.pl_.put(&mut lst);
            }
            {
                let lsm = AutoLock::new(&mut lstSvcM_);
                let lst = get_lst(&mut lstSvc_);
                if !lst.is_empty() {
                    self.pl_.wakeAll();
                }
            }
            {
                let lcm = AutoLock::new(&mut lstCntrlM_);
                let mut lst = get_lst(&mut lstCntrl_);
                if self.cntrl_.shutdown {
                    if self.pl_.pending() > 0 {
                        let lsm = AutoLock::new(&mut lstSocketM_);
                        let mut lsts = get_lst(&mut lstSocket_);
                        self.pl_.put(&mut lsts);
                    }
                    needed = false;
                }
            }
            }
        }
    }
    fn fini(&mut self) {
        {
            unsafe {
            let lcm = AutoLock::new(&mut lstCntrlM_);
            let mut lst = get_lst(&mut lstCntrl_);
            lst.iter().position(|e| (**e).svcid == self as *const _ as usize).map(|e| lst.remove(e));
            }
        }
        self.pl_.wakeAll();
        {
            unsafe {
            let rtm = AutoLock::new(&mut rtM_);
            rt -= 1;
            }
        }
        assert!(self.pl_.lst_done.is_empty());
    }
    pub fn start_thread(l:Option<Vec<ESocket>>) {
        sh();
        inc_threads_avail();
        std::thread::spawn(|| { 
            let mut s = Server::new(l);
            let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
                s.call();
            }));
            if res.is_err() {
                let _ = std::io::stderr().write(format!("error from thread: {:?}\r\n",res).as_bytes());
            }
            s.fini();
        });
    }
    pub fn can_exit() {
        unsafe {
        std::thread::sleep(Duration::from_secs(1));
        let mut pl = EPoll::new(1000);
        let mut lcm = AutoLock::new(&mut lstCntrlM_);
        let mut lst = get_lst(&mut lstCntrl_);
        while !lst.is_empty() {
            lcm.unlock();
            let _ = pl.perform(false,1000);
            lcm.lock();
        }
        }
    }
}
