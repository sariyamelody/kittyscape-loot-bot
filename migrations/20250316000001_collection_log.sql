CREATE TABLE IF NOT EXISTS collection_log_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    discord_id TEXT NOT NULL,
    item_name TEXT NOT NULL,
    points INTEGER NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(discord_id) REFERENCES users(discord_id)
); 