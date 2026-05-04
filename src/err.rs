use std::error::Error as Error_Trait;

use poise::serenity_prelude::{Colour, CreateEmbed};

#[derive(Debug, Clone)]
pub enum ErrorCause {
    SlpConn, 
    SlpHandshake, 
    SlpRequest, 
    SlpResponse, 
    SlpResReadBuf, 
    SlpResReadUtf, 
    SlpResDeserialize, 
    RconHandshake, 
    RconAuth, 
    RconCommand, 
    ServerDataSerialize,
    ServerDataSave, 
    ServerDataRead,
    ServerDataDeserialize,
    ReadRootDir,
}

impl ErrorCause {
    /// hihi
    pub fn to_string (&self) -> String {
        match self {
            ErrorCause::SlpConn => String::from("Server Connecton Failed"),
            ErrorCause::SlpHandshake => String::from("Server Handshake Failed"),
            ErrorCause::SlpRequest => String::from("Server Connection Failed"),
            ErrorCause::SlpResponse => String::from("Server Response Failed"),
            ErrorCause::SlpResReadBuf => String::from("Failed Reading Received Data"),
            ErrorCause::SlpResReadUtf => String::from("Failed Decoding Received Data"),
            ErrorCause::SlpResDeserialize => String::from("Failed Deserializing Received Data"),
            ErrorCause::RconHandshake => String::from("Failed connecting to RCON"),
            ErrorCause::RconAuth => String::from("RCON Authentication Failed"),
            ErrorCause::RconCommand => String::from("RCON Command Failed"),
            ErrorCause::ServerDataSerialize => String::from("Failed Serializing Server Data"),
            ErrorCause::ServerDataSave => String::from("Failed Saving Server Data"),
            ErrorCause::ServerDataRead => String::from("Failed Retrieving Server Data"),
            ErrorCause::ServerDataDeserialize => String::from("Failed Deserializing Server Data"),
            ErrorCause::ReadRootDir => String::from("Failed Retrieving Server Datas"),
        }
    }
}


/// own error type
#[derive(Debug, Clone)]
pub struct Error {
    pub cause: ErrorCause, 
    pub reason: String,
}

impl Error_Trait for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl Error {
    pub fn get_embed (&self) -> CreateEmbed {
        let desc = self.cause.to_string(); 
        CreateEmbed::new()
            .colour(Colour::DARK_RED)
            .title("Oops! An error occured")
            .description(format!("{}\n```{}```", desc, self.reason))
    }
}