## rsmcpinger2

basically a discord bot that pings to a minecraft server and checks whether the server is up, and checks changes in online player list

it also tracks online time for each player

#### Used libs
- Discord bot: `poise`
- Serialize/Deserialize: `serde`, `serde-json`
- `tokio` for threadding
- `chrono` for date formatting

### How to install 
1. clone the repo
2. build the app (`cargo build`)
3. set up environment variables (`DISCORD_TOKEN` and `PROXY`)
4. install mcrcon, put it in `/../workingdirectory/mcrcon/mcrcon`
5. run the bot (or put it as a service) 

**For server**
1. go to `server.properties`, edit the following fields: 
    - `enable-rcon` to `true`
    - `rcon-password` to whatever you want
2. when bot is on, invite to server
3. on the channel you wanted to send status messages, run the `/setup` command (admin only)
4. you are all set!
5. in case of editing the server to ping to, just run the `/setup` command again

### Available commands
- `/setup` admin only - setup minecraft server to ping to
- `/playerlist` - check current player list
- `/playtime player` `/playtime leaderboard` - check playtime for a player / leaderboard
- `/ping` - Pong! check latency

### Working demo
https://discord.com/oauth2/authorize?client_id=1490497409065811978 

### Bugs
report them in issues tab! or make a pull request!

### Plans
- use rust's mcrcon 
- ping to mc server as well when `/ping`
- track server uptime
- Suggest more in issues...