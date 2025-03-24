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
   ```
5. Make sure the bot has "View Channel" and "Send Messages" permissions in the channel specified by MOD_CHANNEL_ID
6. Run migrations: `sqlx database setup`
7. Start the bot: `cargo run`

## Commands

- `/drop <item> [quantity]` - Record a valuable drop
- `/clog <item>` - Record a collection log entry
- `/stats` - View your stats and rank progress
- `/leaderboard` - View top players

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