#!/bin/bash

# Ensure the script stops on first error
set -e

# Display usage information
function show_usage {
    echo "RuneLite Message Analyzer"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -c, --channel-id CHANNEL_ID  Discord channel ID to analyze"
    echo "  -l, --limit LIMIT            Maximum number of messages to analyze (default: 100)"
    echo "  -h, --help                   Show this help message"
    echo ""
    exit 1
}

# Default values
CHANNEL_ID=""
LIMIT=100

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        -c|--channel-id)
            CHANNEL_ID="$2"
            shift 2
            ;;
        -l|--limit)
            LIMIT="$2"
            shift 2
            ;;
        -h|--help)
            show_usage
            ;;
        *)
            echo "Unknown option: $1"
            show_usage
            ;;
    esac
done

# Check if channel ID is provided
if [ -z "$CHANNEL_ID" ]; then
    echo "Error: Discord channel ID is required"
    show_usage
fi

# Run migrations first to ensure the database is up to date
echo "Running database migrations..."
cargo run --bin migrate

# Export variables for the analyzer
export RUNELITE_CHANNEL_ID="$CHANNEL_ID"
export ANALYZE_LIMIT="$LIMIT"
export RUST_LOG=info,kittyscape_loot_bot=debug

# Run the analyzer
echo "Starting analysis of channel $CHANNEL_ID (limit: $LIMIT messages)..."
cargo run --bin analyze_runelite

echo "Analysis complete!" 