#[macro_export]
macro_rules! for_loop{
    ((;;$(;$lbl:tt)*)$bl:block)=>{
        $($lbl:)* loop {$bl}
    };
    ((;$cond:expr;$(;$lbl:tt)*)$bl:block)=>{
        $($lbl:)* while $cond {$bl}
    };
    ((;$cond:expr;$inc:expr$(;$lbl:tt)*)$bl:block)=>{
        $($lbl:)* while $cond {
            $bl;
            $inc;
        }
    };
    (($($vars:stmt),*;$cond:expr;$(;$lbl:tt)*)$bl:block)=>{
        $(
            $vars;
        )*
        $($lbl:)* while $cond {
            $bl;
        }
    };
    (($($vars:stmt),*;$cond:expr;$inc:expr$(;$lbl:tt)*)$bl:block) => {{
        $(
            $vars;
        )*
        $($lbl:)* while $cond {
            $bl;
            $inc;
        }
    }};
}

#[test]
fn test(){
    for_loop!((let mut a=0,let b=10,let mut c=2;a<b;{a+=c;c+=1}){ 
        println!("{} {} {}",a,b,c); 
    });
    for_loop!((let (mut a,b,mut c)=(0,10,2);a<b;{a+=c;c+=1}){ 
        println!("{} {} {}",a,b,c); 
    });
    let (mut a,b,mut c)=(0,10,2);
    for_loop!((;a<b;{a+=c;c+=1}){
        println!("{} {} {}",a,b,c); 
    });
    for_loop! ((let mut i = 0;i<10;i+=1){
        println!("{}",i);
    });
    for_loop!((;a>-20;;'outer){
        println!("outer");
        for_loop!((;;){
            println!("{} {} {}",a,b,c); 
            if a < 1 { break 'outer}
            a-=c;
            continue 'outer;
        })
    })
}
