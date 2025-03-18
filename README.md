# KittyScape Loot Bot

A Discord bot for tracking Old School RuneScape drops and managing clan rankings based on loot value.

## Features

- Track valuable drops from OSRS
- Automatically calculate points based on drop values
- View personal points and total drops
- Leaderboard system
- Automatic rank-up notifications

## Setup

1. Install Rust and Cargo (https://rustup.rs/)
2. Clone this repository
3. Create a new Discord bot and get its token:
   - Go to https://discord.com/developers/applications
   - Create a New Application
   - Go to the "Bot" section
   - Click "Add Bot"
   - Copy the token
4. Create a `.env` file in the project root with:
   ```
   DISCORD_TOKEN=your_discord_bot_token_here
   DATABASE_URL=sqlite:kittyscape.db
   ```
5. Run the bot:
   ```bash
   cargo run
   ```

## Commands

- `!drop <item_name> <value>` - Record a new drop (e.g., `!drop "Twisted bow" 1100000000`)
- `!points` - Check your current points
- `!leaderboard` - View the top 10 looters
- `!help` - Show available commands

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