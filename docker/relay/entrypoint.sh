#!/bin/sh
set -e

# If ORACLE_KEYPAIR_JSON is set, write it to a file for the relay to load
if [ -n "$ORACLE_KEYPAIR_JSON" ]; then
    echo "$ORACLE_KEYPAIR_JSON" > /app/data/oracle-keypair.json
    chmod 600 /app/data/oracle-keypair.json
    export AMOS__SOLANA__ORACLE_KEYPAIR_PATH=/app/data/oracle-keypair.json
fi

exec amos-relay
