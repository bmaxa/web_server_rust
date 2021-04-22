// build.rs

use std::process::Command;
use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    // note that there are a number of downsides to this approach, the comments
    // below detail how to improve the portability of these commands.
/*    Command::new("gcc").args(&["src/hello.c", "-c", "-fPIC", "-o"])
                       .arg(&format!("{}/hello.o", out_dir))
                       .status().unwrap();
*/
    Command::new("fasm").arg("src/mutex.asm").arg(out_dir.clone()+"/mutex.o").status().unwrap();
    Command::new("gcc").arg("src/macro_wrapper.c").
        arg("-c").
        arg("-o").
        arg(out_dir.clone()+"/macro_wrapper.o").status().unwrap();

    Command::new("ar").args(&["crus", "libaux.a", "mutex.o", "macro_wrapper.o"])
                      .current_dir(&Path::new(&out_dir))
                      .status().unwrap();
    println!("cargo:rustc-link-search=native={}", out_dir.clone());
    println!("cargo:rustc-link-lib=static=aux");
    println!("cargo:rustc-link-lib=dylib=ssl");
    println!("cargo:rustc-link-lib=dylib=crypto");
    println!("cargo:rustc-link-lib=dylib=atomic");
}
