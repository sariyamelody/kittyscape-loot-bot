CREATE TABLE IF NOT EXISTS runescape_accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    discord_id TEXT NOT NULL,
    runescape_name TEXT NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(discord_id) REFERENCES users(discord_id),
    UNIQUE(discord_id, runescape_name)
); 