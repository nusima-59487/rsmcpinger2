use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use std::time::Duration;

use mc_rcon::RconClient;
use serde_json::Value;
use tokio::time::timeout;

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

async fn read_varint(stream: &mut TcpStream) -> Result<i32, String> {
    let mut val: i32 = 0;
    let mut pos = 0;
    loop {
        let mut buf = [0; 1];
        stream.read(&mut buf).await.map_err(|e| e.to_string())?;
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
        TcpStream::connect(&format!("{server_address}:{server_port}")).await.map_err(|e| Error {
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
        .await
        .map_err(|e| Error {
            cause: ErrorCause::SlpHandshake,
            reason: e.to_string(),
        })?;

    // Request
    let mut request_payload: Vec<u8> = vec![0x00]; // packet id
    request_payload.splice(0..0, stream_varint(request_payload.len() as i32)); // prepend length
    stream
        .write(&request_payload.into_boxed_slice())
        .await
        .map_err(|e| Error {
            cause: ErrorCause::SlpRequest,
            reason: e.to_string(),
        })?;

    // Response
    let _ = read_varint(&mut stream).await.map_err(|e| Error {
        cause: ErrorCause::SlpResponse,
        reason: e.to_string(),
    })? as usize; // packet size
    let _ = stream.read_exact(&mut [0_u8]); // packet id
    let res_json_size = read_varint(&mut stream).await.map_err(|e| Error {
        cause: ErrorCause::SlpResponse,
        reason: e.to_string(),
    })? as usize; // json response size
    let mut res_json_buffer = vec![0_u8; res_json_size].into_boxed_slice();
    stream.read_exact(&mut res_json_buffer).await.map_err(|e| Error {
        cause: ErrorCause::SlpResReadBuf,
        reason: e.to_string(),
    })?;
    if res_json_buffer.is_empty() {
        eprintln!("Received empty response from server!");
        return Err(Error {
            cause: ErrorCause::SlpConn,
            reason: "Received empty response from server".into(),
        });
    }
    let res_json_str = String::from_utf8(res_json_buffer.to_vec()).map_err(|e| Error {
        cause: ErrorCause::SlpResReadUtf,
        reason: e.to_string(),
    })?;
    let res_json: Value = serde_json::from_str(&res_json_str).map_err(|e| Error {
        cause: ErrorCause::SlpResDeserialize,
        reason: format!("Error parsing response JSON: {}\nOriginal JSON: {}", e.to_string(), res_json_str),
    })?;

    Ok(res_json)
}

pub async fn mcrcon(
    server_address: &str,
    rcon_port: u16,
    rcon_password: &str,
    command: String,
) -> Result<String, Error> {
    async fn mcrcon_inner(
        server_address: &str,
        rcon_port: u16,
        rcon_password: &str,
        command: String,
    ) -> Result<String, Error> {
        let client =
            RconClient::connect(format!("{server_address}:{rcon_port}")).map_err(|e| Error {
                cause: ErrorCause::RconHandshake,
                reason: e.to_string(),
            })?;

        client.log_in(rcon_password).map_err(|e| Error {
            cause: ErrorCause::RconAuth,
            reason: format!("{e:?}"),
        })?;

        let result = client.send_command(&command).map_err(|e| Error {
            cause: ErrorCause::RconCommand,
            reason: format!("{e:?}"),
        })?;

        return Ok(result);
    }

    return match timeout(
        Duration::from_secs(RCON_TIME_LIMIT_SECS),
        mcrcon_inner(server_address, rcon_port, rcon_password, command),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(Error {
            cause: ErrorCause::RconHandshake,
            reason: "RCON Connection Timed Out".into(),
        }),
    };
}

/// utilizing mcrcon
pub async fn fetch_player_list(
    server_address: &str,
    rcon_port: u16,
    rcon_password: &str,
) -> Result<Vec<String>, Error> {
    let result = mcrcon(server_address, rcon_port, rcon_password, "list".to_string()).await?; 

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
