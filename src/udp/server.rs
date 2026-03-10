use crossbeam_channel::Sender;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};

use crate::state::messages::NoteCommand;
use crate::udp::parser::parse_command;

/// Current state of the UDP server, shared with the TUI for display.
#[derive(Debug, Clone)]
pub enum UdpStatus {
    Starting,
    Bound { addr: String },
    Failed { reason: String },
}

pub fn run_udp_server(
    tx: Sender<NoteCommand>,
    sample_rate: f32,
    status: Arc<Mutex<UdpStatus>>,
) {
    let log_path = "/tmp/rustsynth-udp.log";

    let socket = match UdpSocket::bind("0.0.0.0:49161") {
        Ok(s) => {
            let addr = s.local_addr().map(|a| a.to_string()).unwrap_or_else(|_| "0.0.0.0:49161".into());
            log(log_path, &format!("Bound successfully on {}", addr));
            *status.lock().unwrap() = UdpStatus::Bound { addr };
            s
        }
        Err(e) => {
            let reason = format!("bind(0.0.0.0:49161) failed: {}", e);
            log(log_path, &reason);
            *status.lock().unwrap() = UdpStatus::Failed { reason };
            return;
        }
    };

    let mut buf = [0u8; 256];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, _addr)) => {
                if let Ok(s) = std::str::from_utf8(&buf[..len]) {
                    for cmd_str in s.split(';') {
                        let trimmed = cmd_str.trim();
                        if trimmed.is_empty() { continue; }
                        match parse_command(trimmed, sample_rate) {
                            Some(cmd) => {
                                if tx.try_send(cmd).is_err() {
                                    log(log_path, "note command queue full, dropping");
                                }
                            }
                            None => {
                                log(log_path, &format!("unrecognized command: {:?}", trimmed));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log(log_path, &format!("recv error: {}", e));
            }
        }
    }
}

fn log(path: &str, msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[rustsynth-udp] {}", msg);
    }
}
