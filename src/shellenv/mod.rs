//! ## Shellenv
//!
//! `shellenv` is the module which takes care of processing the shell environment and the process execution

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

extern crate nix;
extern crate subprocess;

//I/O
use std::io::{Read, Write};
//UNIX stuff
use nix::sys::signal;
use nix::unistd::Pid;
//Subprocess
use subprocess::{ExitStatus, Popen, PopenConfig, PopenError, Redirection};

/// ShellProcess represents a shell process execution instance
/// it contains the command and the arguments passed at start and the process pipe

pub struct ShellProcess {
    pub command: String,
    pub args: Vec<String>,
    pub exit_status: Option<u8>,
    process: Popen,
}

impl ShellProcess {
    /// Start a new process and returns a ShellProcess struct
    /// If process failed to start, returns a PopenError
    pub fn exec(argv: Vec<String>) -> Result<ShellProcess, PopenError> {
        if argv.len() == 0 {
            return Err(PopenError::from(std::io::Error::from(
                std::io::ErrorKind::InvalidInput,
            )));
        }
        let p = Popen::create(
            &argv,
            PopenConfig {
                stdin: Redirection::Pipe,
                stdout: Redirection::Pipe,
                stderr: Redirection::Pipe,
                detached: false,
                ..Default::default()
            },
        );
        let process: Popen = match p {
            Ok(p) => p,
            Err(err) => return Err(err),
        };
        let command: String = String::from(&argv[0]);
        let mut args: Vec<String> = Vec::with_capacity(argv.len() - 1);
        if argv.len() > 1 {
            for arg in &argv[1..] {
                args.push(String::from(arg));
            }
        }
        Ok(ShellProcess {
            command: command,
            args: args,
            process: process,
            exit_status: None,
        })
    }

    /// Read process output
    pub fn read(&mut self) -> std::io::Result<(Option<String>, Option<String>)> {
        //NOTE: WHY Not communicate? Well, because the author of this crate,
        //arbitrary decided that it would have been a great idea closing
        //the stream after calling communicate, so you can't read/write twice or more times to the process
        //match self.process.communicate(Some("")) {
        //    Ok((stdout, stderr)) => Ok((stdout, stderr)),
        //    Err(err) => Err(err),
        //}
        let mut stdout: &std::fs::File = &self.process.stdout.as_ref().unwrap();
        let mut output_byte: [u8; 8192] = [0; 8192];
        if let Err(err) = stdout.read(&mut output_byte) {
            return Err(err);
        }
        let raw_output: String = match std::str::from_utf8(&output_byte) {
            Ok(s) => String::from(s),
            Err(_) => return Err(std::io::Error::from(std::io::ErrorKind::InvalidData)),
        };
        //Trim null terminators
        let output = String::from(raw_output.trim_matches(char::from(0)));
        Ok((Some(output), None))
    }

    /// Write input string to stdin
    pub fn write(&mut self, input: String) -> std::io::Result<()> {
        if self.process.stdin.is_none() {
            panic!("Stdin is None");
        }
        let mut stdin: &std::fs::File = &self.process.stdin.as_ref().unwrap();
        stdin.write_all(input.as_bytes())
    }

    /// Returns whether the process is still running or not
    pub fn is_running(&mut self) -> bool {
        if self.exit_status.is_some() {
            return false; //Don't complicate it if you already know the result
        }
        match self.process.poll() {
            None => true,
            Some(exit_status) => {
                match exit_status {
                    //This is fu***** ridicoulous
                    ExitStatus::Exited(rc) => {
                        self.exit_status = Some(rc as u8);
                    }
                    ExitStatus::Signaled(rc) => {
                        self.exit_status = Some(rc);
                    }
                    ExitStatus::Other(rc) => {
                        self.exit_status = Some(rc as u8);
                    }
                    ExitStatus::Undetermined => {
                        self.exit_status = None;
                    }
                };
                false
            }
        }
    }

    /// Send a signal to the running process
    pub fn raise(&mut self, signal: signal::Signal) -> Result<(), ()> {
        match self.process.pid() {
            Some(pid) => {
                let unix_pid: Pid = Pid::from_raw(pid as i32);
                match signal::kill(unix_pid, signal) {
                    Ok(_) => {
                        //Wait timeout
                        match self
                            .process
                            .wait_timeout(std::time::Duration::from_millis(100))
                        {
                            Ok(exit_status_opt) => match exit_status_opt {
                                Some(exit_status) => match exit_status {
                                    //This is fu***** ridicoulous
                                    ExitStatus::Exited(rc) => {
                                        self.exit_status = Some(rc as u8);
                                    }
                                    ExitStatus::Signaled(rc) => {
                                        self.exit_status = Some(rc);
                                    }
                                    ExitStatus::Other(rc) => {
                                        self.exit_status = Some(rc as u8);
                                    }
                                    ExitStatus::Undetermined => {
                                        self.exit_status = None;
                                    }
                                },
                                None => {}
                            },
                            Err(_) => return Err(()),
                        }
                        Ok(())
                    }
                    Err(_) => Err(()),
                }
            }
            None => Err(()),
        }
    }

    /// Kill using SIGKILL the sub process
    pub fn kill(&mut self) -> Result<(), ()> {
        match self.process.kill() {
            Ok(_) => {
                match self.process.wait() {
                    Ok(exit_status) => match exit_status {
                        //This is fu***** ridicoulous
                        ExitStatus::Exited(rc) => {
                            self.exit_status = Some(rc as u8);
                        }
                        ExitStatus::Signaled(rc) => {
                            self.exit_status = Some(rc);
                        }
                        ExitStatus::Other(rc) => {
                            self.exit_status = Some(rc as u8);
                        }
                        ExitStatus::Undetermined => {
                            self.exit_status = None;
                        }
                    },
                    Err(_) => return Err(()),
                }
                Ok(())
            }
            Err(_) => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_subprocess_output_only() {
        let argv: Vec<String> = vec![
            String::from("echo"),
            String::from("foo"),
            String::from("bar"),
        ];
        let mut process: ShellProcess = match ShellProcess::exec(argv) {
            Ok(p) => p,
            Err(error) => panic!("Could not start process 'echo foo bar': {}", error),
        };
        //We do not expect any input, go straight with the output
        loop {
            //Read stdout
            match process.read() {
                Ok((stdout, _)) => match stdout {
                    Some(output) => {
                        println!("Echo Output: '{}'", output);
                        assert_eq!(output, String::from("foo bar\n"));
                    }
                    None => {}
                },
                Err(error) => {
                    panic!("Could not read process stdout: {}", error);
                }
            }
            //If process is not running, exit
            if !process.is_running() {
                break;
            }
        }
        println!(
            "Process exited with exit status: {}",
            process.exit_status.unwrap()
        );
        assert_eq!(process.exit_status.unwrap(), 0); //Should be 0
    }

    #[test]
    fn test_subprocess_io() {
        //the best and simplest example with this is CAT command :D
        let argv: Vec<String> = vec![String::from("cat")]; //No extra arg
        let mut process: ShellProcess = match ShellProcess::exec(argv) {
            Ok(p) => p,
            Err(error) => panic!("Could not start process 'cat': {}", error),
        };
        //Check if running and waiting
        assert!(process.is_running());
        println!("cat process started");
        //Write something, that should be echoed
        let input: String = String::from("Hello World!\n");
        if let Err(err) = process.write(input.clone()) {
            panic!("Could not write to cat stdin: {}", err);
        }
        println!("Wrote {}", input.clone());
        //Read, output should be equal to input
        match process.read() {
            Ok((stdout, _)) => match stdout {
                Some(output) => {
                    println!("Cat Output: '{}'", output);
                    assert_eq!(output, input);
                }
                None => {
                    panic!("No input from cat");
                }
            },
            Err(error) => {
                panic!("Could not read process stdout: {}", error);
            }
        }
        //Process should still be running
        assert!(process.is_running());
        //Write something else
        let input: String = String::from("I don't care if monday's blue!\nTuesday's gray and Wednesday too\nThursday I don't care about you\nIt's Friday I'm in love\n");
        if let Err(err) = process.write(input.clone()) {
            panic!("Could not write to cat stdin: {}", err);
        }
        println!("Wrote {}", input.clone());
        //Read, output should be equal to input
        match process.read() {
            Ok((stdout, _)) => match stdout {
                Some(output) => {
                    println!("Cat Output: '{}'", output);
                    assert_eq!(output, input);
                }
                None => {
                    panic!("No input from cat");
                }
            },
            Err(error) => {
                panic!("Could not read process stdout: {}", error);
            }
        }
        //Finally Send SIGINT
        if let Err(err) = process.raise(signal::Signal::SIGINT) {
            panic!("Could not send SIGINT to cat process: {:?}", err);
        }
        //Process should be terminated
        assert!(!process.is_running());
        //Exit code should be 2
        assert_eq!(process.exit_status.unwrap(), 2);
    }

    #[test]
    fn test_kill() {
        let argv: Vec<String> = vec![
            String::from("read"),
            String::from("-n"),
            String::from("8"),
            String::from("-p"),
            String::from("\">> \""),
        ];
        let mut process: ShellProcess = match ShellProcess::exec(argv) {
            Ok(p) => p,
            Err(error) => panic!("Could not start process 'read': {}", error),
        };
        //Check if running and waiting
        assert!(process.is_running());
        println!("read process started");
        //Kill process
        if let Err(err) = process.kill() {
            panic!("Could not kill 'read' process: {:?}", err);
        }
        assert!(!process.is_running());
        //Exit code should be 9
        assert_eq!(process.exit_status.unwrap(), 9);
    }
}
