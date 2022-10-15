#!/bin/sh
set -e

####################
# SET UP VARIABLES #
####################

# current time, for file naming
TIMESTAMP=$(date +%F_%H.%M.%S)

# temp folder name is random uuid
UUID_DIR_NAME=$(cat /proc/sys/kernel/random/uuid)
FILE_SUFFIX=$(echo "$UUID_DIR_NAME" | head -c 8)
# UUID=$(cat /proc/sys/kernel/random/uuid) | cut -c 1
TEMP_DIR_NAME="/tmp/${FILE_SUFFIX}"

LOCATION_BACKUPS=/backups
LOCATION_ALL_LOGS=/logs
LOCATION_REDIS=/redis_data

# Final filename
FINAL_OUTPUT_NAME="adsbdb_${TIMESTAMP}_SQL_REDIS_LOGS_${FILE_SUFFIX}.tar.gz.gpg"

# Move into temp directory
cd "$LOCATION_BACKUPS" || exit 1

# Create tmp dir using random string
mkdir "$TEMP_DIR_NAME"

tar -C "$LOCATION_ALL_LOGS" -cf "$TEMP_DIR_NAME/logs.tar" ./

tar -C "$LOCATION_REDIS" -cf "$TEMP_DIR_NAME/redis_data.tar" ./

# Dump adbsdb database into a tar in tmp folder
pg_dump -U "$DB_NAME" -d "$DB_NAME" -h "$DOCKER_PG_HOST" -p "${DOCKER_PG_PORT}" --no-owner --format=t > "$TEMP_DIR_NAME/pg_dump.tar"

# gzip postgres output

tar -C "$TEMP_DIR_NAME" -cf "$TEMP_DIR_NAME/combined.tar" logs.tar pg_dump.tar redis_data.tar

gzip "$TEMP_DIR_NAME/combined.tar"

# Encrypt data using pass file
gpg --output "$LOCATION_BACKUPS/$FINAL_OUTPUT_NAME" --batch --passphrase "$GPG_PASSWORD" -c "$TEMP_DIR_NAME/combined.tar.gz"

chmod 440 "$LOCATION_BACKUPS/$FINAL_OUTPUT_NAME"

# Remove tmp dir
rm -rf "$TEMP_DIR_NAME"

# remove backup files that are older than 6 days
find "$LOCATION_BACKUPS" -type f -name '*tar.gz.gpg' -mtime +6 -delete

exit 0
