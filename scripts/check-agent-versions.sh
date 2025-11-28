#!/bin/bash

set -euo pipefail

HOSTS_FILE="${1:-}"
TIMEOUT="${2:-5}"

if [ -z "$HOSTS_FILE" ]; then
    echo "Usage: $0 <hosts-file> [timeout-seconds]"
    echo ""
    echo "hosts-file: Text file with one host:port per line"
    echo "timeout-seconds: Optional timeout for each request (default: 5)"
    echo ""
    echo "Example hosts file format:"
    echo "  validator1.example.com:9090"
    echo "  relayer1.example.com:9090"
    echo "  192.168.1.10:9090"
    exit 1
fi

if [ ! -f "$HOSTS_FILE" ]; then
    echo "Error: Hosts file '$HOSTS_FILE' not found"
    exit 1
fi

echo "Querying agent versions from hosts in: $HOSTS_FILE"
echo "Timeout: ${TIMEOUT}s"
echo "================================================"
echo ""

while IFS= read -r host || [ -n "$host" ]; do
    # Skip empty lines and comments
    if [ -z "$host" ] || [[ "$host" =~ ^[[:space:]]*# ]]; then
        continue
    fi

    # Remove leading/trailing whitespace
    host=$(echo "$host" | xargs)

    # Construct URL
    url="http://${host}/version"

    # Query the endpoint
    echo -n "Checking $host ... "

    if response=$(curl -s --max-time "$TIMEOUT" "$url" 2>/dev/null); then
        # Try to parse JSON response
        if git_sha=$(echo "$response" | jq -r '.git_sha' 2>/dev/null); then
            if [ "$git_sha" != "null" ] && [ -n "$git_sha" ]; then
                echo "OK - version: $git_sha"
            else
                echo "ERROR - Invalid response format: $response"
            fi
        else
            echo "ERROR - Failed to parse JSON: $response"
        fi
    else
        echo "ERROR - Failed to connect or timed out"
    fi
done < "$HOSTS_FILE"

echo ""
echo "================================================"
echo "Version check complete"
