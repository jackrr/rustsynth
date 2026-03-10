use crossbeam_channel::Sender;
use tokio::net::UdpSocket;

use crate::state::messages::NoteCommand;
use crate::udp::parser::parse_command;

pub async fn run_udp_server(tx: Sender<NoteCommand>, sample_rate: f32) {
    let socket = UdpSocket::bind("0.0.0.0:49161")
        .await
        .expect("Failed to bind UDP socket on port 49161");

    eprintln!("UDP server listening on port 49161");

    let mut buf = [0u8; 256];
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, _addr)) => {
                let data = &buf[..len];
                if let Ok(s) = std::str::from_utf8(data) {
                    // Split on semicolons to support multiple commands per packet
                    for cmd_str in s.split(';') {
                        let trimmed = cmd_str.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        match parse_command(trimmed, sample_rate) {
                            Some(cmd) => {
                                if tx.try_send(cmd).is_err() {
                                    eprintln!("Note command queue full, dropping command");
                                }
                            }
                            None => {
                                eprintln!("Failed to parse command: {:?}", trimmed);
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
