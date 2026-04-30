use std::{collections::HashMap, fs::{read_dir, read_to_string}, path::Path};

use chrono::Utc;
use poise::serenity_prelude::MessageId;
use serde::{Serialize, Deserialize};

use crate::{RCON_TIME_LIMIT_SECS, err::{Error, ErrorCause}, pinger};


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerData {
    /// timestamp in rfc3339
    pub last_seen: String, 
    pub is_online: bool, 
    pub total_online_seconds: u32, 
}

impl PlayerData {
    fn secs_since_player_join (&self) -> Option<u32> {
        if !self.is_online { return None; }
        let current_time = Utc::now(); 
        let player_join_time = chrono::DateTime::parse_from_rfc3339(&self.last_seen)
            .ok()?
            .with_timezone(&Utc); 
        let play_duration = current_time - player_join_time; 
        // if this cast fails i would blame naureen for being online too long
        let play_duration_secs = play_duration.num_seconds() as u32; 
        return Some(play_duration_secs); 
    }

    pub fn get_current_online_secs (&self) -> u32 {
        return self.total_online_seconds + self.secs_since_player_join().unwrap_or(0); 
    }

    pub fn set_online (&mut self, is_online: bool) {
        if self.is_online == is_online {return;}
        if !is_online {
            self.total_online_seconds = self.get_current_online_secs(); 
        }
        self.last_seen = Utc::now().to_rfc3339(); 
        self.is_online = is_online;
    }
}

impl Default for PlayerData {
    fn default() -> Self {
        Self { last_seen: Utc::now().to_rfc3339(), is_online: Default::default(), total_online_seconds: Default::default() }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerData {
    pub server_address: String, 
    pub server_port: u16, 
    pub rcon_port: u16, 
    pub rcon_password: String, 
    pub status_channel_id: u64, 
    pub status_message_id: Option<MessageId>, 
    pub is_online: bool, 
    pub player_data: HashMap<String, PlayerData>,
    #[serde(skip)]
    pub filepath: String, // for now
}

impl ServerData {
    pub fn get_player_data (&self, player_name: &str) -> Option<&PlayerData> {
        return self.player_data.get(player_name); 
    }
    /// doesn't save for you
    pub fn set_player_data (&mut self, player_name: &str, player_data: PlayerData) {
        let _ = self.player_data.insert(player_name.to_string(), player_data); 
    }
    /// doesn't save for you
    pub fn set_online (&mut self, is_online: bool) {
        self.is_online = is_online; 
        if !is_online {
            for player_data in self.player_data.values_mut() {
                player_data.set_online(false); 
            }
        }
    }

    /// using mcrcon
    pub async fn fetch_online_players_list (&self) -> Result<Vec<String>, Error> {
        pinger::fetch_player_list(&self.server_address, self.server_port, &self.rcon_password).await
    }
    /// using fetch_slp
    pub async fn fetch_online_players_count (&self) -> Result<u64, Error> {
        let res_json = self.fetch_slp().await?; 
        
        let players_online = &res_json["players"]["online"]
            .as_u64()
            .ok_or_else(|| Error { 
                cause: ErrorCause::SlpResDeserialize, 
                reason: "Error parsing player count from response JSON!".into(),
            })?;

        Ok(*players_online)
    }

    /// server list ping
    /// 
    /// if error, server is prolly offline, you needa call the function yourself tho
    pub async fn fetch_slp (&self) -> Result<serde_json::Value, Error>  {
        let rcon_time_limit = tokio::time::Duration::from_secs(RCON_TIME_LIMIT_SECS);

        return match tokio::time::timeout(rcon_time_limit, pinger::ping(&self.server_address, self.server_port)).await {
            Ok(result) => result,
            Err(_) => {
                eprintln!("RCON command timed out!");
                return Err(Error { cause: ErrorCause::RconCommand, reason: "RCON command timed out".into() })
            },
        };
    }
    pub async fn mcrcon (&self, command: String) -> Result<String, Error> {
        pinger::mcrcon(&self.server_address, self.rcon_port, &self.rcon_password, command).await
    }

    /// doesn't save for you :)
    pub async fn update_online_players_data (&mut self) -> Result<(), Error> {
        // TODO: optimization: retrieve entire playerdata hashmap at once, then add new ones
        let online_player_list = self.fetch_online_players_list().await?; 
        let mut server_player_list: Vec<_> = self.player_data.keys().cloned().collect(); 
        server_player_list.extend(online_player_list.clone());
        for player in server_player_list {
            let mut player_data = self.get_player_data(&player)
                .map(|data| data.clone())
                .unwrap_or_default(); 
            let is_player_currently_online = if online_player_list.contains(&player) { true } else { false }; 
            player_data.set_online(is_player_currently_online);
            self.set_player_data(&player, player_data); 
        }; 

        return Ok(()); 
    }

    pub fn save (&self) -> Result<(), Error> {
        let json_str = serde_json::to_string(self)
            .map_err(|e| Error {
                cause: ErrorCause::ServerDataSerialize, 
                reason: e.to_string()
            })?; 
        let filename_path: &Path = self.filepath.as_ref(); 
        filename_path.parent().inspect(|e| {
            let _ = std::fs::create_dir_all(e); 
        }); 
        let save_result = std::fs::write(&self.filepath, json_str); 
        return save_result
            .map_err(|e| Error {
                cause: ErrorCause::ServerDataSave, 
                reason: e.to_string()
            }); 
    }

    fn read_from_path (path: &str) -> Result<Self, Error> {
        let file_content = read_to_string(path)
            .map_err(|e| Error {
                cause: ErrorCause::ServerDataRead, 
                reason: e.to_string()
            })?; 
        let content: Result<Self, _> = serde_json::from_str(&file_content); 
        let mut toreturn = content.map_err(|e| Error {
            cause: ErrorCause::ServerDataDeserialize, 
            reason: e.to_string()
        })?; 
        toreturn.filepath = path.to_string(); 
        return Ok(toreturn); 
    }

    pub fn read (root_dir: &str, guild_id: u64) -> Result<Self, Error> {
        let filepath = format!("{root_dir}/{guild_id}.json"); 
        return Self::read_from_path(&filepath); 
    }

    pub fn new(server_address: String, server_port: u16, rcon_port: u16, rcon_password: String, root_dir: &str, status_channel_id: u64, guild_id: u64) -> Self {
        let filename = format!("{root_dir}/{guild_id}.json"); 
        Self { 
            server_address, 
            server_port, 
            rcon_port, 
            rcon_password, 
            is_online: false, 
            status_channel_id, 
            status_message_id: None, 
            player_data: HashMap::new(), 
            filepath: filename, 
        }
    }
}

pub fn get_all_server_data(root_dir: &str) -> Result<HashMap<u64, ServerData>, Error> {
    let _ = std::fs::create_dir_all(root_dir);
    let dir_items  = read_dir(root_dir)
        .map_err(|e| Error {
            cause: ErrorCause::ReadRootDir, 
            reason: e.to_string()
        })?;
    let mut map_to_return: HashMap<u64, ServerData> = HashMap::new(); 
    for entry in dir_items.filter(|e| e.is_ok()) {
        let filename = entry.unwrap().file_name(); // err ones get filtered alr
        if let Some(filename) = filename.to_str() {
            let server_data_result = ServerData::read_from_path(&format!("{root_dir}/{filename}")); 
            if let Err(why) = server_data_result {
                eprintln!("ServerData skipped due to error ({}): {} ({})", filename, why.to_string(), why.cause.to_string()); 
                continue;
            }
            let server_data = server_data_result.unwrap(); 

            let Some(guild_id) = filename.strip_suffix(".json") else { eprintln!("filename strip fail: {filename:?}"); continue; }; 
            let Ok(guild_id) = guild_id.parse::<u64>() else { eprintln!("guild_id parse fail: {guild_id:?}"); continue; }; 
            map_to_return.insert(guild_id, server_data);
        }; 
    }; 
    return Ok(map_to_return); 
}