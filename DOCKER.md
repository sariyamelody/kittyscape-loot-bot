# Running KittyScape Loot Bot with Docker

This guide explains how to run the KittyScape Loot Bot using Docker without having to compile the code on your server.

## Prerequisites

- Docker installed on your server
- Your Discord bot token and channel IDs

## Setup

1. Clone the repository on your server:
   ```
   git clone https://github.com/yourusername/kittyscape-loot-bot.git
   cd kittyscape-loot-bot
   ```

2. Create a `.env` file in the project directory with your configuration:
   ```
   DISCORD_TOKEN=your_discord_bot_token_here
   DATABASE_URL=sqlite:/app/kittyscape.db
   MOD_CHANNEL_ID=your_mod_channel_id_here
   RUNELITE_CHANNEL_ID=your_runelite_channel_id_here
   BOT_LOG_CHANNEL_ID=your_log_channel_id_here
   ```

## Building and Running

Build and start the bot with this one-liner:
```
docker build -t kittyscape-bot . && docker run -d --name kittyscape-bot --restart unless-stopped --env-file .env kittyscape-bot
```

This will:
1. Build the Docker image (compiling the Rust code)
2. Run the container in detached mode
3. Use the environment variables from your `.env` file
4. Store the database inside the container at `/app/kittyscape.db`

## Viewing Logs

To see the bot's logs:
```
docker logs -f kittyscape-bot
```

## Stopping the Bot

To stop the bot:
```
docker stop kittyscape-bot
```

To remove the container:
```
docker rm kittyscape-bot
```

## Updating the Bot

When you want to update to a new version:

1. Pull the latest code:
   ```
   git pull
   ```

2. Stop and remove the old container:
   ```
   docker stop kittyscape-bot
   docker rm kittyscape-bot
   ```

3. Build and start the new container:
   ```
   docker build -t kittyscape-bot . && docker run -d --name kittyscape-bot --restart unless-stopped -v "./.env:/app/.env" kittyscape-bot
   ```

## Database Persistence

Note that with this setup, the database is stored inside the container. If you remove the container, you will lose your data.

If you want to persist the database between container removals (*STRONGLY RECOMMENDED*), you can modify the run command to mount a volume:
```
docker build -t kittyscape-bot . && docker run -d --name kittyscape-bot --restart unless-stopped -v "./.env:/app/.env" -v "$(pwd)/kittyscape.db:/app/kittyscape.db" kittyscape-bot
```


I build like this:

```
docker build --network host  -t sariyamelody/kittyscape-loot-bot:latest .
docker build --network host  -t sariyamelody/kittyscape-loot-bot:$(git rev-parse --short=8 HEAD) .
docker push sariyamelody/kittyscape-loot-bot:latest
docker push sariyamelody/kittyscape-loot-bot:$(git rev-parse --short=8 HEAD)
```

## Troubleshooting

- **Connection Issues**: Verify your Discord token and channel IDs are correct in the `.env` file
- **Container Won't Start**: Check the logs with `docker logs kittyscape-bot` to see if there are any errors
- **Database Issues**: If you're using volume mounting and have permission issues, make sure the local database file is writable 