# KittyScape Loot Bot

A Discord bot for tracking Old School RuneScape drops and collection log entries, with a point-based ranking system.

## Project Structure

```
kittyscape-loot-bot/
├── src/
│   ├── main.rs                    # Bot initialization and event handling
│   ├── command_handler/
│   │   ├── mod.rs                 # Command handler module and utilities
│   │   ├── utils.rs               # Shared formatting utilities
│   │   └── commands/              # Individual command implementations
│   │       ├── drop.rs            # Drop command handler
│   │       ├── clog.rs            # Collection log command handler
│   │       ├── stats.rs           # Stats command handler
│   │       ├── leaderboard.rs     # Leaderboard command handler
│   │       ├── drop_remove.rs     # Drop remove command handler
│   │       └── clog_remove.rs     # Collection log remove command handler
│   ├── rank_manager.rs            # Rank management and notifications
│   ├── prices.rs                  # OSRS price data management
│   ├── collection_log.rs          # Collection log data management
│   └── logger.rs                  # Action logging to log channel
├── migrations/
│   ├── 20240316000000_initial.sql        # Initial database schema
│   ├── 20240316000001_collection_log.sql # Collection log table
│   └── 20240317000000_rank_tiers.sql     # Default rank tiers
└── Cargo.toml                     # Project dependencies
```

## Core Components

### PriceManager
- Fetches and maintains OSRS item prices from the wiki API
- Updates prices every 5 minutes
- Provides item suggestions for autocomplete
- Used by `/drop` command to calculate points (1 point per 100,000 gp)
- Maintains mappings of item names to IDs and latest prices

### CollectionLogManager
- Fetches collection log completion rates from the wiki API
- Parses HTML data to extract item names and completion rates
- Provides item suggestions for autocomplete
- Used by `/clog` command to calculate points based on item rarity
- Points calculation tiers:
  - ≤5%: Mega-rare items (exponential scaling)
    - 5% → 500 points
    - 3% → 1000 points
    - 1% → 15000 points
    - 0.5% → 30000 points
  - 5-20%: Moderately rare items (linear interpolation)
    - 20% → 200 points
    - 5% → 500 points
  - \>20%: Common items (linear scaling)
    - Points = 100 - (completion_rate * 0.5)

### RankManager
- Manages user points and rank progression
- Handles rank-up notifications:
  - Sends notification to moderator channel when users rank up
  - Displays celebration message with firework emojis in the command channel when a user ranks up
- Rank tiers:
  - 0-999: Small Fry
  - 1k-2,999: Purrveyor
  - 3,000-7,999: Journeycat
  - 8,000-14,999: Meowster
  - 15,000-29,999: Pawfficer
  - 30,000-49,999: Mewtenant
  - 50,000-74,999: Admeowral
  - 75,000-99,999: Grandmeowster
  - 100,000+: Prestige Grandmeowster I-V (10k increments)
  - 150,000+: Exalted Grandmeowster I-V (10k increments)
  - 200,000+: Divine Grandmeowster I-V (10k increments)
  - 250,000+: Eternal Grandmeowster

### Database Schema
- `users`: Stores user points and total drops
- `drops`: Records individual drops with values
- `collection_log_entries`: Records collection log items
- `rank_thresholds`: Defines point thresholds for ranks

### Logger
- Logs all user actions to a dedicated channel
- Records timestamps, user info, and action details for auditing
- Useful for tracking changes to the database and investigating issues
- Actions logged include:
  - Adding drops and collection log entries
  - Removing drops and collection log entries
  - Rank up/down events

## Commands

### `/drop <item> [quantity]`
- Records a tradeable item drop
- Optional quantity parameter (default: 1)
- Calculates points based on total item value (1 point per 100,000 gp)
- Updates user's total drops and points
- Shows rank-up progress
- Displays celebratory message with firework emojis when ranking up
- Uses formatted numbers for better readability
- Falls back to high alchemy value if market price unavailable
- Example: `/drop dragon platelegs 7` for 7x dragon platelegs

### `/drop_remove [id]`
- Removes a mistakenly added drop entry
- Features autocomplete that shows recent drops with their details
- Shows a list of most recent drops when used without parameters
- When providing an ID, removes the specific drop and deducts the points
- Updates user's total drops and points
- Shows rank-down notification if user loses ranks
- Transaction-based to ensure data consistency

### `/clog <item>`
- Records a collection log entry
- Calculates points based on item rarity
- Prevents duplicate entries
- Shows rank-up progress
- Displays celebratory message with firework emojis when ranking up
- Uses formatted numbers for better readability

### `/clog_remove [id]`
- Removes a mistakenly added collection log entry
- Features autocomplete that shows recent entries with their point values
- Shows a list of most recent entries when used without parameters
- When providing an ID, removes the specific entry and deducts the points
- Shows rank-down notification if user loses ranks
- Transaction-based to ensure data consistency

### `/stats`
- Shows detailed user profile in an embed
- Displays:
  - User's display name and avatar
  - Current rank
  - Total points (formatted)
  - Total drops (formatted)
  - Collection log count (formatted)
  - Progress to next rank with percentage

### `/leaderboard`
- Displays top 10 users by points
- Shows top 5 droppers for the past 30 days:
  - Total drops and value
  - Most valuable item dropped
- Shows top 5 collection loggers for the past 30 days:
  - Total entries and points
  - Rarest/most valuable item logged
- Uses formatted values for better readability

## Utilities

### Number Formatting
- `format_points`: Formats point values with commas and "pts" suffix
- `format_number`: Formats numbers with commas
- `format_gp`: Formats gold piece values with commas and "gp" suffix

## Setup
1. Create a Discord bot and get its token
2. Create a `.env` file with:
   ```
   DISCORD_TOKEN=your_discord_bot_token_here
   DATABASE_URL=sqlite:kittyscape.db
   MOD_CHANNEL_ID=your_mod_channel_id_here
   BOT_LOG_CHANNEL_ID=your_log_channel_id_here
   ```
3. Run migrations to set up the database
4. Start the bot with `cargo run`

## Required Bot Permissions
- View Channels
- Send Messages (including in the mod channel specified by MOD_CHANNEL_ID)
- Use Slash Commands

## Dependencies
- serenity: Discord bot framework
- sqlx: Database operations
- reqwest: HTTP requests for wiki API
- tokio: Async runtime
- tracing: Logging and debugging
- anyhow: Error handling
- serde: JSON serialization
- html-escape: HTML entity decoding
- scraper: HTML parsing for wiki data 