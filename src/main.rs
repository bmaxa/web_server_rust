#![feature(try_blocks)]
extern crate web_server;
use web_server::*;
use web_server::sockets::*;
use web_server::server::*;
use web_server::service::*;
use std::time::Duration;

fn main() {
    sh();
    ssl_init();
    let ctx:Result<*mut SSL_CTX,String> = try {
        let ctx = SSL_CTX::new()?;
        ctx.load_certificates("92377831_www.example.com.cert","./92377831_www.example.com.key")?;
        ctx
    };
    let mut s = ESocket::new_impl(Box::new(Default::new(true)));
    let mut ssls = ESocket::ssl_new_impl(ctx.unwrap(),Box::new(Default::new(true)));
    let mut c = ESocket::new_impl(Box::new(Control::new()));
    let mut l = ESocket::new_impl(Box::new(Log::new()));
    let dps = DefaultPipeServer::new_exe("./cgi_server".to_string());
    let ps = PipeService::new_ps(dps);
    let mut p = ESocket::new_impl(Box::new(ps));
    let _ = s.listen("8080");
    let _ = ssls.listen("8082");
    let _ = c.listen("6667");
    let _ = l.listen("9090");
    let _ = p.listen("8081");
    let a = vec!(s,c,l,p,ssls);
    server::init();
    Server::start_thread(Some(a));
    Server::can_exit();
}
