//! ## Runtime
//!
//! `runtime` contains the runtime functions for pyc core

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

//Deps
extern crate ansi_term;
extern crate ctrlc;
extern crate nix;
extern crate termion;

use ansi_term::Colour;
use std::env;
use std::io::Read;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use sysinfo::{RefreshKind, System, SystemExt, ProcessExt};
use termion::async_stdin;

use crate::config;
use crate::shellenv::process::ShellProcess;
use crate::translator::ioprocessor::IOProcessor;

/// ### process_command
///
/// Process a shell command, converting it to latin and then letting the user interacting with it
/// the command output is converted back to cyrillic
/// This function is used in oneshot mode only

pub fn process_command(
  processor: IOProcessor,
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
  let expr: String = match processor.expression_to_latin(argv.join(" ")) {
    Ok(cmd) => cmd,
    Err(err) => {
      print_err(
        String::from(format!("Bad expression: {:?}", err)),
        config.output_config.translate_output,
        &processor,
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
  //Start shell process
  let mut process = match ShellProcess::exec(argv) {
    Ok(p) => p,
    Err(_) => {
      print_err(
        String::from(format!("Unknown command {}", command)),
        config.output_config.translate_output,
        &processor,
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
    print_err(
      String::from("Could not start signal listener"),
      config.output_config.translate_output,
      &processor,
    );
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
        let input: String = String::from(std::str::from_utf8(input_bytes.as_slice()).unwrap());
        if let Err(err) = process.write(processor.text_to_latin(input)) {
          print_err(
            String::from(err.to_string()),
            config.output_config.translate_output,
            &processor,
          );
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
        print_out(out.unwrap(), config.output_config.translate_output, &processor);
      }
      if err.is_some() {
        //Convert err to cyrillic
        print_err(
          err.unwrap().to_string(),
          config.output_config.translate_output,
          &processor,
        );
      }
    }
    //Fetch signals
    match sig_rx.try_recv() {
      Ok(sig) => {
        //Send signals
        if let Err(_) = process.raise(sig) {
          print_err(
            String::from("Could not send SIGINT to subprocess"),
            config.output_config.translate_output,
            &processor,
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

/// ### shell_exec
///
/// Run pyc in shell mode

pub fn shell_exec(processor: IOProcessor, config: &config::Config, shell: Option<String>) -> u8 {
  //Determine the shell to use
  let shell: String = match shell {
    Some(sh) => sh,
    None => match get_shell_from_proc() {
      Ok(sh) => sh,
      Err(()) => match get_shell_from_env() {
        Ok(sh) => sh,
        Err(()) => {
          print_err(
            String::from("Could not determine the shell to use"),
            config.output_config.translate_output,
            &processor,
          );
          return 255;
        }
      },
    },
  };
  println!("SELECTED SHELL IS {}", shell);
  0
}

/// ### get_shell_from_proc
///
/// Try to get the shell path from parent pid

fn get_shell_from_proc() -> Result<String, ()> {
  //Get PID of current process
  let pid = sysinfo::get_current_pid().unwrap();
  //Create a system istance
  let refresh_kind: RefreshKind = RefreshKind::new();
  let refresh_kind: RefreshKind = refresh_kind.with_processes();
  let system = System::new_with_specifics(refresh_kind);
  //Get current process info
  let process = match system.get_process(pid) {
    Some(p) => p,
    None => return Err(())
  };
  //Get parent pid
  let parent_pid = match process.parent() {
    Some(p) => p,
    None => return Err(())
  };
  //Get parent process info
  let process = match system.get_process(parent_pid) {
    Some(p) => p,
    None => return Err(())
  };
  //Return parent process executable
  let parent_exec: String = match process.exe().to_str() {
    Some(s) => String::from(s),
    None => return Err(())
  };
  Ok(parent_exec)
}

/// ### get_shell_from_env
///
/// Try to get the shell path from SHELL environment variable

fn get_shell_from_env() -> Result<String, ()> {
  if let Ok(val) = env::var("SHELL") {
    Ok(val)
  } else {
    Err(())
  }
}

fn print_err(err: String, to_cyrillic: bool, processor: &IOProcessor) {
  match to_cyrillic {
    true => eprintln!("{}", Colour::Red.paint(processor.text_to_cyrillic(err))),
    false => eprintln!("{}", Colour::Red.paint(err)),
  };
}

fn print_out(out: String, to_cyrillic: bool, processor: &IOProcessor) {
  match to_cyrillic {
    true => print!("{}", processor.text_to_cyrillic(out)),
    false => print!("{}", out),
  };
}
