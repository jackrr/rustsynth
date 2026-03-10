use crossbeam_channel::Sender;
use std::net::UdpSocket;

use crate::state::messages::NoteCommand;
use crate::udp::parser::parse_command;

pub fn run_udp_server(tx: Sender<NoteCommand>, sample_rate: f32) {
    let socket = match UdpSocket::bind("0.0.0.0:49161") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("UDP: failed to bind port 49161: {}", e);
            return;
        }
    };

    eprintln!("UDP server listening on port 49161");

    let mut buf = [0u8; 256];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, _addr)) => {
                let data = &buf[..len];
                if let Ok(s) = std::str::from_utf8(data) {
                    for cmd_str in s.split(';') {
                        let trimmed = cmd_str.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        match parse_command(trimmed, sample_rate) {
                            Some(cmd) => {
                                if tx.try_send(cmd).is_err() {
                                    eprintln!("UDP: note command queue full, dropping");
                                }
                            }
                            None => {
                                eprintln!("UDP: unrecognized command: {:?}", trimmed);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("UDP recv error: {}", e);
            }
        }
    }
}
