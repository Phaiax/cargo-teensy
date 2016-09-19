
extern crate rustc_serialize;
extern crate docopt;
extern crate toml;

use docopt::Docopt;
use std::process::{self, ExitStatus, Command};
use std::fs::{File, DirBuilder};
use std::io::Read;
use std::io::Write;

const USAGE: &'static str = "
Teensy in one command.

Usage:
  cargo teensy upload [options]
  cargo teensy new <name>
  cargo teensy (-h | --help)
  cargo teensy --version

Options:
  -r --hard-reboot     teensy_loader_cli: Use hard reboot if device not online
  -s --soft-reboot     teensy_loader_cli: Use soft reboot if device not online (Teensy3.x only)
  -n --no-reboot       teensy_loader_cli: No reboot after programming
  -v --verbose         Show commands before executing
  -h --help            Show this screen.
  --version            Show version.
";

const ABIJSON: &'static [u8] = br#"{
    "arch": "arm",
    "cpu": "cortex-m4",
    "data-layout": "e-m:e-p:32:32-i64:64-v128:64:128-a:0:32-n32-S64",
    "disable-redzone": true,
    "executables": true,
    "llvm-target": "thumbv7em-none-eabi",
    "morestack": false,
    "os": "none",
    "relocation-model": "static",
    "target-endian": "little",
    "target-pointer-width": "32",
    "no-compiler-rt": true,
    "pre-link-args": [
        "-mcpu=cortex-m4", "-mthumb",
        "-Tlayout.ld"
    ],
    "post-link-args": [
        "-lm", "-lgcc", "-lnosys"
    ]
}
"#;

const EXAMPLEMAIN: &'static [u8] = br#"
#![feature(plugin, start)]
#![no_std]
#![plugin(macro_zinc)]

extern crate zinc;

use core::option::Option::Some;

use zinc::hal::cortex_m4::systick;
use zinc::hal::k20::{pin, watchdog};
use zinc::hal::pin::Gpio;

/// Wait the given number of SysTick ticks
pub fn wait(ticks: u32) {
  let mut n = ticks;
  // Reset the tick flag
  systick::tick();
  loop {
    if systick::tick() {
      n -= 1;
      if n == 0 {
        break;
      }
    }
  }
}

#[zinc_main]
pub fn main() {
  zinc::hal::mem_init::init_stack();
  zinc::hal::mem_init::init_data();
  watchdog::init(watchdog::State::Disabled);

  // Pins for MC HCK (http://www.mchck.org/)
  let led1 = pin::Pin::new(pin::Port::PortC, 5, pin::Function::Gpio, Some(zinc::hal::pin::Out));

  systick::setup(systick::ten_ms().unwrap_or(480000));
  systick::enable();
  loop {
    led1.set_high();
    wait(10);
    led1.set_low();
    wait(10);
  }
}
"#;

const MANIFESTADD: &'static str = r#"
[features]
default = ["mcu_k20"]
mcu_k20 = ["zinc/mcu_k20"] # also enables the mcu_k20 feature in the zinc crate

[dependencies]
zinc = { path =  "../zinc" }
macro_zinc = { path = "../zinc/macro_zinc" }
rust-libcore = "*"
"#;

const CARGOCONFIG: &'static [u8] = br#"
[build]
target = "thumbv7em-none-eabi"

[target.thumbv7em-none-eabi]
linker = "arm-none-eabi-gcc"
ar = "arm-none-eabi-ar"
"#;


#[derive(Debug, RustcDecodable)]
struct Args {
    flag_version: bool,
    flag_soft_reboot: bool,
    flag_hard_reboot: bool,
    flag_no_reboot: bool,
    flag_verbose: bool,
    cmd_upload: bool,
    cmd_new: bool,
    arg_name: String,
}

fn execute(mut command : Command, args: &Args) -> (ExitStatus, String) {
    let cmd_str = format!("{:?}", command);
    if args.flag_verbose {
        println!(">> {}", cmd_str);
    }
    let mut child = command.spawn().unwrap_or_else(|e| panic!("{}", e));
    let exit_status = child.wait().unwrap_or_else(|e| panic!("{}", e));
    (exit_status, cmd_str)
}

fn manifest() -> Result<toml::Table, String> {
    let mut f = try!(File::open("Cargo.toml").map_err(|ioerr| format!("{:?}", ioerr)));
    let mut s = String::new();
    try!(f.read_to_string(&mut s).map_err(|ioerr| format!("{:?}", ioerr)));
    let mut parser = toml::Parser::new(&s);
    parser.parse().ok_or("Could not parse Cargo.toml".into())
}

fn binname(manifest : &toml::Table) -> String {
    manifest.get("package").unwrap().as_table().unwrap()
        .get("name").unwrap().as_str().unwrap().into()
}

fn build(args: &Args) -> (ExitStatus, String) {
    let mut command = Command::new("cargo");
    command.arg("build")
        .arg("--verbose")
        .arg("--release")
        .arg("--target=thumbv7em-none-eabi")
        .arg("--features").arg("mcu_k20");
    execute(command, &args)
}

fn make_hex(args: &Args, binname : &str) -> ((ExitStatus, String), String) {
    let hexfile = format!("target/thumbv7em-none-eabi/release/{}.hex", binname);
    let mut command = Command::new("arm-none-eabi-objcopy");
    command.arg("-O").arg("ihex")
        .arg("-R").arg(".eeprom")
        .arg(&format!("target/thumbv7em-none-eabi/release/{}", binname))
        .arg(&hexfile);
    (execute(command, &args) , hexfile)
}

fn upload(args: &Args, hexfile : &str) -> (ExitStatus, String) {
    let mut command = Command::new("teensy_loader_cli");
    command.arg("-w")
        .arg("--mcu").arg("mk20dx256");
    if args.flag_no_reboot {
        command.arg("-n");
    }
    if args.flag_hard_reboot {
        command.arg("-r");
    }
    if args.flag_soft_reboot {
        command.arg("-s");
    }
    command.arg(&hexfile);
    execute(command, &args)
}

fn exit_on_fail(result : (ExitStatus, String)) {
    if result.0.success() {
        return;
    } else if let Some(code) = result.0.code() {
        println!("Failed command: {}", result.1);
        process::exit(code);
    }
}

fn cargo_new(args : &Args) -> (ExitStatus, String) {
    let mut command = Command::new("cargo");
    command.arg("new")
        .arg(&args.arg_name)
        .arg("--bin");
    execute(command, &args)
}

fn write_abi(_ : &Args) {
    let mut f = File::create("thumbv7em-none-eabi.json").unwrap();
    f.write_all(ABIJSON).unwrap();
}

fn write_main(_ : &Args) {
    let mut f = File::create("src/main.rs").unwrap();
    f.write_all(EXAMPLEMAIN).unwrap();
}

fn update_manifest(manifest : &mut toml::Table) {
    
    let mut parser = toml::Parser::new(MANIFESTADD);
    let mut addition = parser.parse().unwrap();

    manifest.append(&mut addition);

    let mut f = File::create("Cargo.toml").unwrap();
    f.write_all(format!("{}", toml::Value::Table(manifest.clone())).as_bytes()).unwrap();
}

fn write_cargo_helper(_: &Args) {
    DirBuilder::new().recursive(true).create(".cargo").unwrap();
    let mut f = File::create(".cargo/config").unwrap();
    f.write_all(CARGOCONFIG).unwrap();    
}

fn main() {
    let args: Args = Docopt::new(USAGE)
                            .and_then(|d| { d.decode() })
                            .unwrap_or_else(|e| e.exit());

    if args.cmd_upload {
        let manifest = manifest().unwrap();
        let binname = binname(&manifest);

        exit_on_fail(build(&args));


        let (result, hexfile) = make_hex(&args, &binname);
        exit_on_fail(result);

        println!("UPLOAD (waiting for reset)");
        exit_on_fail(upload(&args, &hexfile));

        println!("Upload successful");
    } else if args.cmd_new {

        cargo_new(&args);
        std::env::set_current_dir(&args.arg_name).unwrap();

        write_abi(&args);
        write_main(&args);
        write_cargo_helper(&args);

        let mut manifest = manifest().unwrap();
        update_manifest(&mut manifest);
        
    }
}
