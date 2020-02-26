/*
*
*   Copyright (C) 2020 Christian Visintin - christian.visintin1997@gmail.com
*
* 	This file is part of "Pyc"
*
*   Pyc is free software: you can redistribute it and/or modify
*   it under the terms of the GNU General Public License as published by
*   the Free Software Foundation, either version 3 of the License, or
*   (at your option) any later version.
*
*   Pyc is distributed in the hope that it will be useful,
*   but WITHOUT ANY WARRANTY; without even the implied warranty of
*   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
*   GNU General Public License for more details.
*
*   You should have received a copy of the GNU General Public License
*   along with Pyc.  If not, see <http://www.gnu.org/licenses/>.
*
*/

//TODO: shell format function
//TODO: cd to previous directory

const PYC_VERSION: &str = "0.1.0";
const PYC_BUILD: &str = "??";

//Crates
extern crate ctrlc;
extern crate dirs;
extern crate getopts;
extern crate nix;
extern crate termion;

//External modules
use dirs::home_dir;
use getopts::Options;
use std::env;
use std::io::Read;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use termion::{async_stdin, color, style};

//Internal modules
mod config;
mod shellenv;
mod translator;

/// ### print_usage
///
/// Print usage

fn print_usage(program: &String, opts: Options) {
    let brief = format!("Усаж: {} [ОПТИОНС]... [КОММАНД]...", program);
    print!("{}", opts.usage(&brief));
}

/// ### str_to_language
///
/// Convert CLI option language string to Language enum

fn str_to_language(lang: String) -> translator::Language {
    match lang.as_str() {
        "ru" | "рус" => translator::Language::Russian,
        _ => {
            eprintln!(
                "{}Укноун лангуаж: '{}'; Дэфаултинг то русский{}",
                color::Fg(color::Red),
                lang,
                color::Fg(color::Reset)
            );
            translator::Language::Russian
        }
    }
}

/// ### process_command
///
/// Process a shell command, converting it to latin and then letting the user interacting with it
/// the command output is converted back to cyrillic

fn process_command(
    translator: &Box<dyn translator::Translator>,
    config: &config::Config,
    mut argv: Vec<String>,
) -> u8 {
    if argv.len() == 0 {
        //Prevent empty commands
        return 255;
    }
    //Process arg 0
    match config.get_alias(&argv[0]) {
        Some(resolved) => argv[0] = resolved,
        None => {}
    };
    //Join tokens
    let expr: String = argv.join(" ");
    let expr: String = match translator.to_latin(expr) {
        Ok(s) => s,
        Err(err) => {
            //TODO: rewrite command back to stdin and let user to complete / fix it (if possible)
            println!(
                "{}Сынтакс эррор: {:?}{}",
                color::Fg(color::Red),
                err,
                color::Fg(color::Reset)
            );
            return 255;
        }
    };
    //Convert expression back to argv
    let mut argv: Vec<String> = Vec::with_capacity(expr.matches(" ").count() + 1);
    for arg in expr.split_whitespace() {
        argv.push(String::from(arg));
    }
    let command: String = argv[0].clone();
    if command == "cd" {
        //@! Handle cd command
        let path: std::path::PathBuf = if argv.len() > 1 {
            let mut pathbuf = std::path::PathBuf::new();
            pathbuf.push(std::path::Path::new(argv[1].as_str()));
            pathbuf
        } else {
            match home_dir() {
                Some(path) => {
                    let mut pathbuf = std::path::PathBuf::new();
                    pathbuf.push(std::path::Path::new(path.as_path()));
                    pathbuf
                }
                None => {
                    let mut pathbuf = std::path::PathBuf::new();
                    pathbuf.push(std::path::Path::new("~"));
                    pathbuf
                }
            }
        };
        match std::env::set_current_dir(path.as_path()) {
            Ok(()) => return 0,
            Err(_) => {
                let message: String = String::from(format!(
                    "The directory '{}' does not exist",
                    path.to_str().unwrap_or("?")
                ));
                if config.output_config.translate_output {
                    eprintln!(
                        "{}{}{}",
                        color::Fg(color::Red),
                        translator.to_cyrillic(message),
                        color::Fg(color::Reset)
                    );
                } else {
                    eprintln!(
                        "{}{}{}",
                        color::Fg(color::Red),
                        message,
                        color::Fg(color::Reset)
                    );
                }
                return 255;
            }
        };
    }
    //Start shell process
    let mut process = match shellenv::ShellProcess::exec(argv) {
        Ok(p) => p,
        Err(_) => {
            println!(
                "{}Укноун комманд '{}'{}",
                color::Fg(color::Red),
                command,
                color::Fg(color::Reset)
            );
            return 255;
        }
    };
    //Create input stream
    let mut stdin = async_stdin().bytes();
    let mut input_bytes: Vec<u8> = Vec::new();
    let running = Arc::new(Mutex::new(true));
    let (sig_tx, sig_rx) = mpsc::channel::<nix::sys::signal::Signal>();
    let sig_running = Arc::clone(&running);
    //Start signal handler
    if let Err(_) = ctrlc::set_handler(move || {
        let mut terminate: bool = false;
        while !terminate {
            {
                //Inside this block, otherwise does never go out of scope
                let current_state = sig_running.lock().unwrap();
                if *current_state == false {
                    terminate = true;
                }
            }
            if let Err(_) = sig_tx.send(nix::sys::signal::Signal::SIGINT) {
                break;
            }
            sleep(Duration::from_millis(50));
        }
    }) {
        eprintln!(
            "{}Коулд нот старт сигнал листенер{}",
            color::Fg(color::Red),
            color::Fg(color::Reset)
        )
    }
    //@! Loop until process has terminated
    while process.is_running() {
        //Read user input
        if let Some(Ok(i)) = stdin.next() {
            input_bytes.push(i);
        //TODO: pass characters at each input to stdin?
        } else {
            //Buffer is empty, if len > 0, send input to program, otherwise there's no input
            if input_bytes.len() > 0 {
                //Convert bytes to UTF-8 string
                let input: String =
                    String::from(std::str::from_utf8(input_bytes.as_slice()).unwrap());
                if let Err(err) = process.write(input) {
                    if config.output_config.translate_output {
                        eprintln!(
                            "{}{}{}",
                            color::Fg(color::Red),
                            translator.to_cyrillic(err.to_string()),
                            color::Fg(color::Reset)
                        );
                    } else {
                        eprintln!(
                            "{}{}{}",
                            color::Fg(color::Red),
                            err.to_string(),
                            color::Fg(color::Reset)
                        );
                    }
                }
                //Reset input buffer
                input_bytes = Vec::new();
            }
        }
        /*
        let mut input: String = String::new();
        stdin.read_to_string(&mut input);
        if input.len() > 0 {
            println!("INPUT: {}", input);
        }
        */
        //Read program stdout
        if let Ok((out, err)) = process.read() {
            if out.is_some() {
                //Convert out to cyrillic
                let out: String = if config.output_config.translate_output {
                    translator.to_cyrillic(out.unwrap())
                } else {
                    out.unwrap()
                };
                print!("{}", out);
            }
            if err.is_some() {
                //Convert err to cyrillic
                let err: String = if config.output_config.translate_output {
                    translator.to_cyrillic(err.unwrap())
                } else {
                    err.unwrap()
                };
                eprint!(
                    "{}{}{}",
                    color::Fg(color::Red),
                    translator.to_cyrillic(err.to_string()),
                    color::Fg(color::Reset)
                );
            }
        }
        //Fetch signals
        match sig_rx.try_recv() {
            Ok(sig) => {
                //Send signals
                if let Err(_) = process.raise(sig) {
                    eprintln!(
                        "{}Коулд нот сенд сигнал {} то субпросес!{}",
                        color::Fg(color::Red),
                        sig,
                        color::Fg(color::Reset)
                    );
                }
            }
            Err(_) => {}
        }
        sleep(Duration::from_millis(10)); //Sleep for 10ms
    }
    //Terminate sig hnd
    let mut sig_term = running.lock().unwrap();
    *sig_term = true;
    drop(sig_term); //Otherwise the other thread will never read the state
                    //Return exitcode
    process.exit_status.unwrap_or(255)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program: String = args[0].clone();
    //Program CLI options
    let config_file: String;
    //let shell: String;
    let oneshot: bool;
    let language: translator::Language;
    let mut opts = Options::new();
    opts.optopt("c", "конфиг", "Спесифы конфигуратён YAML филе", "<конфиг>");
    opts.optopt("l", "ланг", "Спесифы сырилик лангуажэ", "<ru|рус>");
    opts.optflag("", "ссср", "");
    opts.optflag("v", "версён", "");
    opts.optflag("h", "хелп", "принт хелп меню");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            println!(
                "{}{}{}",
                color::Fg(color::Red),
                f.to_string(),
                color::Fg(color::Reset)
            );
            std::process::exit(255);
        }
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        std::process::exit(255);
    }
    if matches.opt_present("v") {
        eprintln!(
            "{}рус - {} ({}) - Developed by Кристиан Визинтин{}",
            style::Bold,
            PYC_VERSION,
            PYC_BUILD,
            style::Reset
        );
        std::process::exit(255);
    }
    //Set translator language
    language = match matches.opt_str("l") {
        Some(lang) => str_to_language(lang),
        None => translator::Language::Russian,
    };
    //Set config file to '-c' file or to default file
    config_file = match matches.opt_str("c") {
        Some(cfg_override) => cfg_override,
        None => {
            //Default path
            let home: String = match home_dir() {
                Some(path) => String::from(path.to_str().unwrap()),
                None => String::from("~"),
            };
            String::from(home + "/.config/pyc/pyc.yml")
        }
    };
    //Check if oneshot and get args
    let argv: Vec<String> = matches.free.clone();
    oneshot = argv.len() > 0;
    //Parse configuration
    let config: config::Config = match config::Config::parse_config(config_file.clone()) {
        Ok(cfg) => cfg,
        Err(err) => match err.code {
            config::ConfigErrorCode::NoSuchFileOrDirectory => {
                eprintln!(
                    "{}Но суч филэ ор директоры {}; усинг дефаулт конфигуратион{}",
                    color::Fg(color::Red),
                    config_file,
                    color::Fg(color::Reset)
                );
                config::Config::default()
            }
            _ => panic!(
                "{}Коулд нот парсэ YAML конфигуратион: {}{}",
                color::Fg(color::Red),
                err,
                color::Fg(color::Reset)
            ),
        },
    };
    //Set up translator
    let translator: Box<dyn translator::Translator> = translator::new_translator(language);
    let mut rc: u8 = 0;
    if oneshot {
        rc = process_command(&translator, &config, argv);
    } else {
        panic!("Interactive mode hasn't been IMPLEMENTED YET!");
        //TODO: implement loop
        //TODO: catch signals
    }
    std::process::exit(rc as i32);
}
