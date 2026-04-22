use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::Command;

use serde_json::Value;

use crate::RCON_TIME_LIMIT_SECS;
use crate::err::{Error, ErrorCause};

fn stream_varint(i: i32) -> Vec<u8> {
    let mut val: i32 = i;
    let mut bytes: Vec<u8> = Vec::new();
    for _ in 0..5 {
        bytes.push((val | 0x80) as u8);
        val = val >> 7;
    }
    // println!("{:?}", bytes);

    while let Some(last) = bytes.last()
        && *last == 0b1000_0000_u8
    {
        bytes.pop();
    }

    if let Some(last) = bytes.last_mut() {
        *last ^= 0b1000_0000_u8;
    };

    bytes
}

fn stream_str(s: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    let length = s.len();
    bytes.append(&mut stream_varint(length as i32));
    bytes.append(&mut s.as_bytes().to_vec());
    return bytes;
}

fn read_varint(stream: &mut TcpStream) -> Result<i32, String> {
    let mut val: i32 = 0;
    let mut pos = 0;
    loop {
        let mut buf = [0; 1];
        stream.read(&mut buf).map_err(|e| e.to_string())?;
        val |= ((buf[0] & 0x7f) as i32) << pos;

        if buf[0] & 0x80 == 0 {
            break;
        }

        pos += 7;

        if pos >= 32 {
            return Err("Value is too big for an i32!".into());
        }
    }
    return Ok(val);
}

pub async fn ping(server_address: &str, server_port: u16) -> Result<Value, Error> {
    let mut stream =
        TcpStream::connect(&format!("{server_address}:{server_port}")).map_err(|e| Error {
            cause: ErrorCause::SlpConn, 
            reason: e.to_string(),
        })?;

    // Initial Handshake
    let mut handshake_payload: Vec<u8> = vec![0x00]; // packet id
    handshake_payload.append(&mut stream_varint(-1)); // Client Protocol Version
    handshake_payload.append(&mut stream_str(server_address)); // target server address
    handshake_payload.append(&mut server_port.to_be_bytes().to_vec()); // target server port
    handshake_payload.append(&mut vec![0x01]); // intent
    handshake_payload.splice(0..0, stream_varint(handshake_payload.len() as i32)); // prepend length
    stream
        .write(&handshake_payload.into_boxed_slice())
        .map_err(|e| Error {
            cause: ErrorCause::SlpHandshake, 
            reason: e.to_string(),
        })?;

    // Request
    let mut request_payload: Vec<u8> = vec![0x00]; // packet id
    request_payload.splice(0..0, stream_varint(request_payload.len() as i32)); // prepend length
    stream
        .write(&request_payload.into_boxed_slice())
        .map_err(|e| Error {
            cause: ErrorCause::SlpRequest, 
            reason: e.to_string(),
        })?;

    // Response
    let _ = read_varint(&mut stream).map_err(|e| Error {
            cause: ErrorCause::SlpResponse, 
            reason: e.to_string()
        })? as usize; // packet size
    let _ = stream.read_exact(&mut [0_u8]); // packet id
    let res_json_size = read_varint(&mut stream).map_err(|e| Error {
        cause: ErrorCause::SlpResponse, 
        reason: e.to_string(),
    })? as usize; // json response size
    let mut res_json_buffer = vec![0_u8; res_json_size].into_boxed_slice();
    stream.read_exact(&mut res_json_buffer).map_err(|e| Error {
        cause: ErrorCause::SlpResReadBuf, 
        reason: e.to_string(),
    })?;
    let res_json_str = String::from_utf8(res_json_buffer.to_vec()).map_err(|e| Error {
        cause: ErrorCause::SlpResReadUtf, 
        reason: e.to_string(),
    })?;
    let res_json: Value = serde_json::from_str(&res_json_str).map_err(|e| Error {
        cause: ErrorCause::SlpResDeserialize, 
        reason: format!("Error parsing response JSON: {}", e.to_string()),
    })?;

    Ok(res_json)
}

pub async fn mcrcon(
    server_address: &str,
    rcon_port: u16,
    rcon_password: &str,
    command: String,
) -> Result<String, Error> {
    println!("./mcrcon/mcrcon -cH {:?} -p {:?} {:?}", server_address, rcon_password, command); 

    let result = Command::new("./mcrcon/mcrcon")
        .arg("-cH")
        .arg(server_address)
        // .arg("-P")
        // .arg(&rcon_port.to_string())
        .arg("-p")
        .arg(rcon_password)
        .arg(command)
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                let output_msg = String::from_utf8_lossy(&output.stdout).to_string();
                return Ok(output_msg);
            } else {
                // TODO: output.status.code()
                let output_msg = String::from_utf8_lossy(&output.stderr).to_string();
                return Err(Error { cause: ErrorCause::RconCommand, reason: output_msg }); 
            }
        }
        Err(why) => {
            println!("Failed to execute mcrcon command: {why}");
            return Err(Error {
                cause: ErrorCause::RconProcess, 
                reason: why.to_string()
            }); 
        }
    }
}

/// utilizing mcrcon
pub async fn fetch_player_list(
    server_address: &str,
    rcon_port: u16,
    rcon_password: &str,
) -> Result<Vec<String>, Error> {

    
    let rcon_time_limit = tokio::time::Duration::from_secs(RCON_TIME_LIMIT_SECS);

    let result = match tokio::time::timeout(rcon_time_limit, mcrcon(server_address, rcon_port, rcon_password, "list".to_string())).await {
        Ok(result) => result?,
        Err(_) => {
            eprintln!("RCON command timed out!");
            return Err(Error { cause: ErrorCause::RconCommand, reason: "RCON command timed out".into() })
        },
    };

    // let result = mcrcon(server_address, rcon_port, rcon_password, "list".to_string()).await?;
    // match result {
    //     Ok(msg) => {
    let players_str: &str = &result
        .trim_end_matches("\x1b\x5b\x30\x6d")
        .split(": ")
        .nth(1)
        .unwrap_or("");
    if players_str.is_empty() {
        return Ok(vec![]);
    }
    let players: Vec<String> = players_str.split(", ").map(|s| s.trim().into()).collect();
    return Ok(players);
    //     }
    //     Err((code, msg)) => {
    //         let toreturn = match code {
    //             Some(t) => format!("**An error occured: (Status code {})\n {}", t, msg),
    //             None => format!("**An error occured:\n {}", msg),
    //         };
    //         return Err(Error { reason: toreturn });
    //     }
    // }
}
