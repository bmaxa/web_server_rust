use super::service::*;
use super::http_message::*;
use super::sockets::*;
use std::sync::Arc;
use std::cell::RefCell;
type PtrWebService = Option<Arc<WebService>>;
pub trait WebService{
    fn process(&mut self,req:&HTTPRequest)->HTTPResponse;
}
fn new(svc:&str)->PtrWebService {
        unimplemented!();
}

struct WebServer {
    ps_:PtrPipeService,
    s_: State,
    r_:HTTPRequest,
}
#[derive(PartialEq)]
enum State {
    FetchingRequest,
    FetchingBody(Content),
    Processing,
    Sendingresponse,
    Error(i32,String),
}
#[derive(PartialEq)]
enum Content{
    Chunked,
    Whole(usize)
}

impl WebServer{
    fn new()->WebServer{
        WebServer{ps_:None,s_:State::FetchingRequest,r_:HTTPRequest::new()}
    }
    fn need_more_body(&mut self)->bool {
        use self::State::*;
        let mut rc = false;
        if self.s_ == FetchingRequest {
            rc = if !self.r_.m_.headers_.get("transfer-encoding").is_none() {
                self.s_ = FetchingBody(Content::Chunked);
                true
            } else if let Some(l) = self.r_.m_.headers_.get("content-length") {
                if let Ok(n) = l.parse() {
                    self.s_ = FetchingBody(Content::Whole(n));
                    let ps = self.ps_.clone().unwrap();
                    let mut ps = ps.borrow_mut();
                    if ps.buf().len() >= n {
                        false
                    } else {
                        true
                    }
                } else {
                    self.s_ = Processing;
                    false
                }

            } else { false };
        } else {
        }
        rc
    }
}
const MAX_REQ_LEN:usize = 100000;

impl PipeServer for WebServer {
    fn doneReading(&mut self)->bool {
        use self::State::*;
        match self.s_ {
            FetchingRequest => {
                let ps = self.ps_.clone();
                let ps = ps.unwrap();
                let mut ps = ps.borrow_mut();
                let idx =  ps.buf().find("\r\n\r\n");
                if idx.is_none() && ps.buf().len() < MAX_REQ_LEN { return false }
                let s:String = ps.buf().drain(..idx.unwrap()+4).collect();
                if s.len() >= MAX_REQ_LEN {
                    self.s_ = Error(413,"Request to big".to_string())
                } else {
                    self.r_.parse_request(&s);
                    if self.need_more_body() {
                        return false
                    }
                }
            }
            FetchingBody(ref t) => {
                match t {
                    Content::Chunked =>(),
                    Content::Whole(n)=>()
                }
            }
            _ => ()
        };
        true
    }
    fn doneWriting(&mut self)->bool {
        true
    }
    fn end(&mut self)->bool{
        true
    }
    fn prepare_request(&mut self,ps:PtrPipeService,pl:&mut EPoll)->i32{
        self.ps_ = ps.clone();
        if self.doneReading() { -1 } else { 1 }
    }
    fn prepare_response(&mut self,res:&mut String){
    }
    fn perform(&mut self,pl:&mut EPoll)->i32{
        use self::State::*;
        match self.s_ {
            Error(_,_) => return -3,
            FetchingRequest | Processing => (),
            FetchingBody(ref c) => {
                match c {
                    Content::Chunked => (),
                    Content::Whole(n)=> ()
                }
            }
            _ => ()
        };
        -4
    }
    fn reset(&mut self){
        self.ps_ = None;
    }
    fn clone(&mut self)->PtrPipeServer{
        Some(Arc::new(RefCell::new(WebServer::new())))
    }
}
