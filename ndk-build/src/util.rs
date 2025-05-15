use std::{
    env::var,
    io::{self, IsTerminal, Read, stderr},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread::spawn,
};

use crate::error::NdkError;

#[derive(PartialEq, Eq)]
enum Stream {
    Stderr,
    Stdout,
}

struct Output {
    output: Vec<(Stream, Vec<u8>)>,
}
impl Output {
    pub fn push(&mut self, stream: Stream, data: Vec<u8>) {
        self.output.push((stream, data));
    }

    pub fn stdout(&self) -> Vec<u8> {
        let len = self
            .output
            .iter()
            .filter(|x| x.0 == Stream::Stdout)
            .map(|x| x.1.len())
            .sum();
        let mut val = Vec::with_capacity(len);
        for x in self
            .output
            .iter()
            .filter(|x| x.0 == Stream::Stdout)
            .map(|x| &x.1)
        {
            val.extend_from_slice(x);
        }
        val
    }

    pub fn stderr(&self) -> Vec<u8> {
        let len = self.output.iter().map(|x| x.1.len()).sum();
        let mut val = Vec::with_capacity(len);
        for x in self.output.iter().map(|x| &x.1) {
            val.extend_from_slice(x);
        }
        val
    }
}

pub fn output_error(mut command: Command) -> Result<Vec<u8>, NdkError> {
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let mut process = command.spawn()?;
    let (Some(mut stdout), Some(mut stderr)) = (process.stdout.take(), process.stderr.take())
    else {
        unreachable!();
    };
    let output = Arc::new(Mutex::new(Output { output: vec![] }));

    let (h1, h2) = (
        spawn({
            let output = output.clone();
            move || {
                let mut buf = [0u8; 8192];
                loop {
                    match stdout.read(&mut buf) {
                        Err(why) => break Err(why),
                        Ok(0) => break Ok(()),
                        Ok(l) => output
                            .lock()
                            .unwrap()
                            .push(Stream::Stdout, buf[0..l].to_vec()),
                    }
                }
            }
        }),
        spawn({
            let output = output.clone();
            move || {
                let mut buf = [0u8; 8192];
                loop {
                    match stderr.read(&mut buf) {
                        Err(why) => break Err(why),
                        Ok(0) => break Ok(()),
                        Ok(l) => output
                            .lock()
                            .unwrap()
                            .push(Stream::Stderr, buf[0..l].to_vec()),
                    }
                }
            }
        }),
    );

    h1.join().map_err(|_| io::Error::other("join error"))??;
    h2.join().map_err(|_| io::Error::other("join error"))??;

    if process.wait()?.success() {
        Ok(output.lock().unwrap().stdout())
    } else {
        Err(NdkError::CmdFailed(
            command,
            io::Error::other(String::from_utf8_lossy(&output.lock().unwrap().stderr())),
        ))
    }
}

pub fn color() -> bool {
    if var("ALWAYS_COLOR").is_ok() {
        true
    } else if var("NO_COLOR").is_ok() {
        false
    } else {
        stderr().is_terminal()
    }
}
