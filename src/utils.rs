pub fn url_encode(input:&str)->String{
    let mut out = String::new();
    static bhex:[char;16] = ['0','1','2','3','4','5','6','7','8','9','A','B','C','D','E','F'];
    for_loop!((let mut i = input.chars(),let mut c = i.next();
               !c.is_none();
               c = i.next()){
              let cc = c.unwrap();
              if cc.is_alphanumeric() || 
              cc == '-' || cc == '_' || cc == '.' || cc == '!' ||
              cc == '~' || cc == '*' || cc == '(' || cc == ')' ||
              cc == '\'' { out.push(cc) }
              else if cc == ' ' { out.push('+') }
              else {
                  let cc = cc as usize;
                  out.push('%');
                  out.push(bhex[cc >> 4]);
                  out.push(bhex[cc & 0xf]);
              }
    });
    out
}
pub fn url_decode(url: &str)->String {
    let mut out = String::new();

    for_loop!((let mut i=url.chars(),let mut c = i.next();
               !c.is_none();
               c = i.next()){
        let mut cc = c.unwrap();
        if cc == '+' { cc = ' ' }
        else if cc == '%' {
            let cc1 = i.next();
            let cc2 = i.next();
            if cc1.is_none() || cc2.is_none() { break }
            cc = (xdigit(cc1.unwrap()) << 4 | xdigit(cc2.unwrap())) as char;
        }
        out.push(cc);
    });
    out
}
fn xdigit(c:char)->u8{
    static mut table: Option<[u8;256]> = None;
    unsafe {
    if table.is_none() {
        table = Some([0;256]);
        let mut ttable = super::server::get_lst(&mut table);
        ttable['1' as usize]=1;
        ttable['2' as usize]=2;
        ttable['3' as usize]=3;
        ttable['4' as usize]=4;
        ttable['5' as usize]=5;
        ttable['6' as usize]=6;
        ttable['7' as usize]=7;
        ttable['8' as usize]=8;
        ttable['9' as usize]=9;

        ttable['A' as usize]=10;
        ttable['B' as usize]=11;
        ttable['C' as usize]=12;
        ttable['D' as usize]=13;
        ttable['E' as usize]=14;
        ttable['F' as usize]=15;
        ttable['a' as usize]=10;
        ttable['b' as usize]=11;
        ttable['c' as usize]=12;
        ttable['d' as usize]=13;
        ttable['e' as usize]=14;
        ttable['f' as usize]=15;
    }
    table.unwrap()[c as usize]
    }
}
pub fn from_cstr(v:&[u8])->String {
    let mut rc = String::new();
    for i in v {
        if *i == 0 { break }
        rc.push(*i as char);
    }
    rc
}
pub fn remove_dots(i:&str)->String {
    let mut rc:String = i.split("/./").fold(
        String::new(),|v,s| if !v.is_empty() {v+"/"+s} else {v+s});
    println!("{}",rc);
    let idx = rc.ends_with("/.");
    if idx {
        let len = rc.len();
        let _ = rc.drain(len-1..);
    }
    println!("1 {}",rc);
    let mut rc1:String = rc.split("/../").fold(
        String::new(),|v,s| if !v.is_empty() {v+"/"+s} else {v+s});
    println!("2 {}",rc1);
    let idx = rc1.ends_with("/..");
    rc = if idx {
        let len = rc1.len();
        rc1.drain(..len-2).collect()
    } else {
        rc1.clone()
    };
    println!("3 {}",rc1);
    rc
}
pub fn remove_slashes(i:&str)->String {
    let mut rc = String::new();
    let mut i = i.chars();
    while let Some(c) = i.next() {
        if c == '/' {
            rc.push(c);
            while let Some(c) = i.next() {
                 if c == '/' { continue }
                 rc.push(c);
                 break
            }
        } else {
            rc.push(c);
        }
    }
    rc
}
pub fn s_match(patt:&str,string:&str)->bool{
    let (mut c_iter,mut p_iter) = (string.chars(),patt.chars());

    loop {
        match (p_iter.next(),c_iter.next()){
            (None,None) => return true,
            (None,Some(_)) => return false,
            (Some('?'),None) => return false,
            (Some('?'),_) => (),
            (Some('*'),c) => {
                loop {
                    let mut pcc = p_iter.clone().peekable();
                    match pcc.peek() {
                        None => return true,
                        Some(&c) => if c != '*' { break ; }
                        else {p_iter.next();}
                    }
                }
                loop { 
                    println!("{} {}",p_iter.as_str(),c_iter.as_str());
                    if s_match(p_iter.as_str(),c_iter.as_str()) {return true}
                    if let Some(_) = c_iter.next() {continue}
                    else {break}
                }
                return false
            }
            (Some(p),Some(c)) => if p.to_ascii_lowercase() != c.to_ascii_lowercase() {return false}
            (Some(_),None) => return false
        }
    }
}
#[test]
fn check_table(){
    assert_eq!(xdigit('a'),10);
}
#[test]
fn test_url_decode(){
    println!("{}",url_decode("abcd+efgh%"));
}
#[test]
fn test_remove(){
    println!("dots {}",remove_dots("/abc/./../a.def/./.."));
    println!("slashes {}",remove_slashes("///dfs//sfdsd///"));
}
#[test]
fn check_match() {
    println!("match: {}",s_match("a*b?","atetabo"));
    println!("match: {}",s_match("*b*?","atetab"));
    println!("match: {}",s_match("*?*","atetab"));
    println!("match: {}",s_match("?t*b","atetab"));
}
