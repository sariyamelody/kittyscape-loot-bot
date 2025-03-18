# KittyScape Loot Bot

A Discord bot for tracking Old School RuneScape drops and collection log entries, with a point-based ranking system.

## Project Structure

```
kittyscape-loot-bot/
├── src/
│   ├── main.rs           # Bot initialization and event handling
│   ├── commands.rs       # Command handlers and Discord interactions
│   ├── prices.rs         # OSRS price data management
│   └── collection_log.rs # Collection log data management
├── migrations/
│   ├── 20240316000000_initial.sql        # Initial database schema
│   └── 20240316000001_collection_log.sql # Collection log table
└── Cargo.toml            # Project dependencies
```

## Core Components

### PriceManager
- Fetches and maintains OSRS item prices from the wiki API
- Updates prices every 5 minutes
- Provides item suggestions for autocomplete
- Used by `/drop` command to calculate points (1 point per 100,000 gp)

### CollectionLogManager
- Fetches and maintains collection log completion rates from the wiki
- Provides item suggestions for autocomplete
- Used by `/clog` command to calculate points based on item rarity
- Points calculation tiers:
  - ≤5%: Exponential scaling
  - 5-20%: Linear scaling
  - \>20%: Base points

### Database Schema
- `users`: Stores user points and total drops
- `drops`: Records individual drops with values
- `collection_log_entries`: Records collection log items
- `rank_thresholds`: Defines point thresholds for ranks

## Commands

### `/drop <item>`
- Records a tradeable item drop
- Calculates points based on item value (1 point per 100,000 gp)
- Updates user's total drops and points
- Shows rank-up progress

### `/clog <item>`
- Records a collection log entry
- Calculates points based on item rarity
- Prevents duplicate entries
- Shows rank-up progress

### `/points`
- Shows user's current points and total drops

### `/leaderboard`
- Displays top 10 users by points
- Shows points and total drops for each user

### `/stats`
- Shows detailed user profile in an embed
- Displays:
  - User's display name and avatar
  - Current rank
  - Total points and drops
  - Progress to next rank

## Rank System
- Points thresholds defined in `rank_thresholds` table
- Example thresholds:
  ```sql
  INSERT INTO rank_thresholds (points, role_name) VALUES
  (1000000, 'Bronze Hunter'),
  (10000000, 'Silver Hunter'),
  (100000000, 'Gold Hunter'),
  (1000000000, 'Platinum Hunter');
  ```

## Setup
1. Create a Discord bot and get its token
2. Create a `.env` file with:
   ```
   DISCORD_TOKEN=your_discord_bot_token_here
   DATABASE_URL=sqlite:kittyscape.db
   ```
3. Run migrations to set up the database
4. Start the bot with `cargo run`

## Dependencies
- serenity: Discord bot framework
- sqlx: Database operations
- reqwest: HTTP requests for wiki API
- tokio: Async runtime
- tracing: Logging
- anyhow: Error handling
- serde: JSON serialization
- html-escape: HTML entity decoding 