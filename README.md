# KittyScape Loot Bot

A Discord bot for tracking Old School RuneScape drops and collection log entries, with a point-based ranking system.

## Features

- Track valuable drops and collection log entries
- Point-based ranking system
- Automatic rank-up notifications
- Leaderboard tracking
- Beautiful formatting and embeds

## Setup

1. Create a new Discord application and bot at https://discord.com/developers/applications
2. Get your bot token
3. Invite the bot to your server with these permissions:
   - View Channels
   - Send Messages
   - Use Slash Commands
4. Create a `.env` file with:
   ```
   DISCORD_TOKEN=your_discord_bot_token_here
   DATABASE_URL=sqlite:kittyscape.db
   MOD_CHANNEL_ID=your_mod_channel_id_here
   RUNELITE_CHANNEL_ID=your_runelite_channel_id_here
   BOT_LOG_CHANNEL_ID=your_log_channel_id_here
   RANK_REQUEST_CHANNEL_ID=your_rank_channel_id_here
   ```
5. Make sure the bot has "View Channel" and "Send Messages" permissions in the channels specified by MOD_CHANNEL_ID, RUNELITE_CHANNEL_ID, and BOT_LOG_CHANNEL_ID
6. Run migrations: `sqlx database setup`
7. Start the bot: `cargo run`

### Environment Variables

- `DISCORD_TOKEN`: Your Discord bot token (required)
- `DATABASE_URL`: SQLite database path (required)
- `MOD_CHANNEL_ID`: Channel for moderation notifications (required)
- `RUNELITE_CHANNEL_ID`: Channel where RuneLite plugin messages are posted (optional, but required for automatic tracking)
- `BOT_LOG_CHANNEL_ID`: Channel where drop/clog add commands are logged for monitoring (optional)

## Commands

- `/drop <item> [quantity]` - Record a valuable drop
- `/clog <item>` - Record a collection log entry
- `/stats` - View your stats and rank progress
- `/leaderboard` - View top players

## Automatic RuneLite Integration

This bot includes functionality to automatically track RuneScape drops and collection log entries from the RuneLite Discord plugin. Players can link their RuneScape usernames to their Discord accounts, and the bot will automatically add drops and collection log entries when detected in a specified channel.

### Setup

1. Set the `RUNELITE_CHANNEL_ID` environment variable to the ID of the Discord channel where RuneLite plugin messages are posted
2. Users need to link their RuneScape accounts with `/rsname username:RSName`

### Commands

- `/rsname <username>` - Link a RuneScape username to your Discord account
- `/rsname_remove <username>` - Unlink a RuneScape username from your Discord account  
- `/rsnames` - List all RuneScape accounts linked to your Discord account

### Development Utilities

The repo includes several utilities for development:

- `scripts/analyze-runelite.sh` - Analyze messages in the RuneLite channel to help with format detection
  ```
  ./scripts/analyze-runelite.sh --channel-id YOUR_CHANNEL_ID --limit 50
  ```

- `cargo run --bin migrate` - Run database migrations without starting the bot
- `cargo run --bin analyze_runelite` - Analyze RuneLite messages (using RUNELITE_CHANNEL_ID env var)

The RuneLite integration works with these plugins:
- [Discord Rare Drop Notificater](https://runelite.net/plugin-hub/show/discord-rare-drop-notificater)
- [Discord Collection Logger](https://runelite.net/plugin-hub/show/discord-collection-logger)

## Development

See [claude.md](claude.md) for detailed documentation about the project structure, components, and implementation details, oriented towards feeding context to LLMs to make modifying the project easy.

## Database Setup

The bot automatically creates the following tables:
- `users`: Stores user points and total drops
- `drops`: Records individual drops
- `rank_thresholds`: Defines point thresholds for ranks

To set up rank thresholds, use SQL to insert values:
```sql
INSERT INTO rank_thresholds (points, role_name) VALUES
(1000000, 'Bronze Hunter'),
(10000000, 'Silver Hunter'),
(100000000, 'Gold Hunter'),
(1000000000, 'Platinum Hunter');
```
For convenience, a default set of rank thresholds have been provided in `migrations/`.