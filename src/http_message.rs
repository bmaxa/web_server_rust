use std::collections::HashMap;
use std::sync::Arc;
use std::cell::RefCell;
use std::collections::linked_list::LinkedList;
use std::fmt::Write;

type HTTPChunk = Arc<RefCell<Vec<u8>>>;

#[derive(Debug)]
pub enum HTTPError{
    ParseError(::text_io::Error),
    WrongNumber(i32),
    MissingRequestLine,
}
pub struct HTTPMessage{
        pub headers_: HashMap<String,String>,
        pub body_: LinkedList<HTTPChunk>,
        major_:i32,
        minor_:i32,
}

impl HTTPMessage{
    fn new()->HTTPMessage{
        HTTPMessage{
            headers_:HashMap::new(),
            body_:LinkedList::new(),
            major_:0,
            minor_:0,
        }
    }
}
pub struct HTTPRequest{
        pub m_: HTTPMessage,
        method_: String,
        uri_: String,
        path_:String,
        domain_:String,
        port_:i32,
        scheme_:String,
        query_:HashMap<String,String>,
        cookies_:HashMap<String,String>,
    }
pub struct HTTPResponse{
        pub m_:HTTPMessage,
        pub status_code_:i32,
        pub phrase_:String,
    }
static mut rfcCodeTable:Option<HashMap<i32,&str>> = None;

impl HTTPRequest{
    pub fn new()->HTTPRequest{
        HTTPRequest{
            m_:HTTPMessage::new(),
            method_:String::new(),
            uri_:String::new(),
            path_:String::new(),
            domain_:String::new(),
            port_:80,
            scheme_:String::new(),
            query_:HashMap::new(),
            cookies_:HashMap::new(),
        }
    }
    pub fn parse_request(&mut self,arg:&str)->Result<(),HTTPError> {
        let mut i = arg.lines();
        let arg = i.next();
        if arg.is_none() { return Err(HTTPError::MissingRequestLine) }
        let arg = arg.unwrap();
        while let Some(line) = i.next() {
            if !line.is_empty() {
                let v:Vec<_> = line.splitn(1,':').collect();
                if v.len() == 2 {
                    self.m_.headers_.insert(
                        v[0].trim().to_ascii_lowercase().to_string(),
                        v[1].trim().to_string());
                }
            }
        }
        let mut v:Vec<_> = arg.split_whitespace().collect();
        if v.len() != 3 { return Err(HTTPError::WrongNumber(v.len() as i32 )) }
        self.method_ = v[0].to_string();
        self.uri_ = v[1].to_string();
        println!("{:?}",v);
        let res:Result<(),::text_io::Error> = try {
            try_scan!(v[2].bytes() => "HTTP/{}.{}",self.m_.major_,self.m_.minor_);
        };
        
        let mut res = match res {
           Err(e) =>  return Err(HTTPError::ParseError(e)),

           Ok(_) => Ok(())
        };
        // now parse uri.
        let mut path:String ="".to_string();
        let res1:Result<(),::text_io::Error> =  try {
            try_scan!(self.uri_.bytes() => "http://{}",path);
        };
        self.path_ = if res1.is_err() {
            if let Some(d) = self.m_.headers_.get("host") {
                self.domain_ = d.to_string();
            }
            self.uri_.clone()
        } else {
                let mut itr = path.splitn(1,'/');
                if let Some(s) = itr.next() {
                    self.domain_ = s.to_string();
                }
                if let Some(s) = itr.next() {
                    "/".to_string()+s
                } else {
                    "/".to_string()
                }
        };
        let path = self.path_.clone();
        let dp : Vec<_> = path.splitn(1,'?').collect();
        if dp.len() == 2 {
            self.query_decode(dp[1]);
        }
        self.path_ = { 
            let s = super::utils::remove_dots(dp[0]);
            super::utils::remove_slashes(&s)
        };
        res
        
    }
    pub fn query_decode(&mut self,q:&str){
        let pairs:Vec<_> = q.split('&').collect();
        for i in pairs {
            let pair:Vec<_> = i.splitn(1,'=').collect();
            if pair.len() == 2 {
                self.query_.insert(super::utils::url_decode(pair[0]),super::utils::url_decode(pair[1]));
            }
        }
    }
}

impl HTTPResponse {
    fn new()->Self{
        HTTPResponse{
            m_:HTTPMessage::new(),
            status_code_:0,
            phrase_:String::new(),
        }
    }
    fn format_status_line(&self)->String{
        let rc ;
        rc = format!("HTTP/{}.{} {} {}\r\n",
                     self.m_.major_,
                     self.m_.minor_,
                     self.status_code_,
                     if self.phrase_.is_empty() { 
                         HTTPResponse::table_phrase(self.status_code_)
                     } else {
                         &self.phrase_
                     });
        rc
    }
    fn add_headers(&self)->String {
        unimplemented!();
    }
    fn table_phrase<'a>(p:i32)->&'a str{
        unsafe {
            if rfcCodeTable.is_none() {
                HTTPResponse::table_init();
            }
            let table = super::server::get_lst(&mut rfcCodeTable);
            table[&p]
        }
    }
    fn table_init(){
	  unsafe {
  	  // informational 1xx
	  rfcCodeTable = Some(HashMap::new());
	  let rfcCodeTable_ = super::server::get_lst(&mut rfcCodeTable);
      // informational 1xx
      rfcCodeTable_.insert(100, "Continue");
	  rfcCodeTable_.insert(101, "Switching Protocols");
	  // successful 2xx
	  rfcCodeTable_.insert(200, "OK");
	  rfcCodeTable_.insert(201, "Created");
	  rfcCodeTable_.insert(202, "Accepted");
	  rfcCodeTable_.insert(203, "Non-Authoritative Information");
	  rfcCodeTable_.insert(204, "No Content");
	  rfcCodeTable_.insert(205, "Reset Content");
	  rfcCodeTable_.insert(206, "Partial Content");
	  // redirection 3xx
	  rfcCodeTable_.insert(300, "Multiple Choices");
	  rfcCodeTable_.insert(301, "Moved Permanently");
	  rfcCodeTable_.insert(302, "Found");
	  rfcCodeTable_.insert(303, "See Other");
	  rfcCodeTable_.insert(304, "Not Modified");
	  rfcCodeTable_.insert(305, "Use Proxy");
	  rfcCodeTable_.insert(306, "(Unused)");
	  rfcCodeTable_.insert(307, "Temporary Redirect");
	  // client error 4xx
	  rfcCodeTable_.insert(400, "Bad Request");
	  rfcCodeTable_.insert(401, "Unauthorized");
	  rfcCodeTable_.insert(402, "Payment Required");
	  rfcCodeTable_.insert(403, "Forbidden");
	  rfcCodeTable_.insert(404, "Not Found");
	  rfcCodeTable_.insert(405, "Method Not Allowed");
	  rfcCodeTable_.insert(406, "Not Acceptable");
	  rfcCodeTable_.insert(407, "Proxy Authentication Required");
	  rfcCodeTable_.insert(408, "Request Timeout");
	  rfcCodeTable_.insert(409, "Conflict");
	  rfcCodeTable_.insert(410, "Gone");
	  rfcCodeTable_.insert(411, "Length Required");
	  rfcCodeTable_.insert(412, "Precondition Failed");
	  rfcCodeTable_.insert(413, "Request Entity Too Large");
	  rfcCodeTable_.insert(414, "Request-URI Too Long");
	  rfcCodeTable_.insert(415, "Unsupported Media Type");
	  rfcCodeTable_.insert(416, "Requested Range Not Satisfiable");
	  rfcCodeTable_.insert(417,"Expectation Failed");
	  // server error 5xx
	  rfcCodeTable_.insert(500,"Internal Server Error");
	  rfcCodeTable_.insert(501,"Not Implemented");
	  rfcCodeTable_.insert(502,"Bad Gateway");
	  rfcCodeTable_.insert(503,"Service Unavailable");
	  rfcCodeTable_.insert(504,"Gateway Timeout");
	  rfcCodeTable_.insert(505,"HTTP Version Not Supported");
	  }
    }
}


#[test]
fn try_parse(){
    let mut req = HTTPRequest::new();
    req.parse_request("GET /    HTTP/1.1\r\n").unwrap();
    println!("{} {} {} {}",req.method_,req.uri_,req.m_.major_,req.m_.minor_);
}
