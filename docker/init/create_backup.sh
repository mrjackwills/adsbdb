#!/bin/sh
set -e -o pipefail


# Check the required env's are present
check_env() {
	missing=false
	for env_var in \
		AGE_RECIPIENT \
		DB_NAME \
		DOCKER_PG_HOST \
		DOCKER_PG_PORT \
		DOCKER_REDIS_HOST \
		DOCKER_REDIS_PORT \
		PGPASSWORD \
		REDISCLI_AUTH
	do
		case "$env_var" in
			AGE_RECIPIENT) value="$AGE_RECIPIENT" ;;
			DB_NAME) value="$DB_NAME" ;;
			DOCKER_PG_HOST) value="$DOCKER_PG_HOST" ;;
			DOCKER_PG_PORT) value="$DOCKER_PG_PORT" ;;
			DOCKER_REDIS_HOST) value="$DOCKER_REDIS_HOST" ;;
			DOCKER_REDIS_PORT) value="$DOCKER_REDIS_PORT" ;;
			PGPASSWORD) value="$PGPASSWORD" ;;
			REDISCLI_AUTH) value="$REDISCLI_AUTH" ;;
		esac

		if [ -z "$value" ]; then
			echo "ERROR: environment variable $env_var is not set" >&2
			missing=true
		fi
	done

	if [ "$missing" = true ]; then
		exit 1
	fi
}


# Verify a file is a non-empty, intact gzip stream
check_gzip() {
	if [ ! -s "$1" ]; then
		echo "ERROR: $1 is empty" >&2
		exit 1
	fi
	if ! gzip -t "$1" >/dev/null 2>&1; then
		echo "ERROR: $1 is not a valid or intact gzip stream" >&2
		exit 1
	fi
}

# Verify the age output is present, non-empty, and has the expected age header
check_age_file() {
	if [ ! -s "$1" ]; then
		echo "ERROR: $1 is empty" >&2
		exit 1
	fi
	# Every age-encrypted file starts with the 22-byte preamble "age-encryption.org/v1\n"
	if [ "$(head -c 22 "$1")" != "age-encryption.org/v1" ]; then
		echo "ERROR: $1 is not a valid age-encrypted file" >&2
		exit 1
	fi
}

# Tar gzip all the logs
backup_logs() {
	echo "attempting to backup logs"
	tar -C "$LOCATION_ALL_LOGS" -czf "$TEMP_DIR_NAME/$FILE_NAME_LOGS" ./
	check_gzip "$TEMP_DIR_NAME/$FILE_NAME_LOGS"
	echo "logs gzipped correctly"
}

# Create a redis backup, validate, and compress
backup_redis() {
	echo "attempting to backup redis"
	if ! redis-cli -h "$DOCKER_REDIS_HOST" -p "$DOCKER_REDIS_PORT" --rdb "$REDIS_DUMP_FILE"; then
		echo "ERROR: redis-cli fail" >&2
		exit 1
	fi

	if [ ! -s "$REDIS_DUMP_FILE" ]; then
		echo "ERROR: RDB empty" >&2
		exit 1
	fi

	if [ "$(head -c 5 "$REDIS_DUMP_FILE")" != "REDIS" ]; then
		echo "ERROR: Invalid RDB file" >&2
		exit 1
	fi

	gzip -c "$REDIS_DUMP_FILE" > "$TEMP_DIR_NAME/$FILE_NAME_REDIS"
	check_gzip "$TEMP_DIR_NAME/$FILE_NAME_REDIS"
	rm -f "$REDIS_DUMP_FILE"
	echo "redis gzipped correctly"
}

# Dump adsbdb database into a gzip tar in tmp folder
backup_postgres() {
	echo "attempting to backup postgres"
	pg_dump -U "$DB_NAME" -d "$DB_NAME" -h "$DOCKER_PG_HOST" -p "$DOCKER_PG_PORT" --no-owner --format=t | gzip -6 > "$TEMP_DIR_NAME/$FILE_NAME_POSTGRES.partial"
	check_gzip "$TEMP_DIR_NAME/$FILE_NAME_POSTGRES.partial"
	mv "$TEMP_DIR_NAME/$FILE_NAME_POSTGRES.partial" "$TEMP_DIR_NAME/$FILE_NAME_POSTGRES"
	echo "postgres gzipped correctly"
}

combine_encrypt_files() {
	echo "Combining and encrypting files"

	# Tar all the files, then pipe to age
	tar -C "$TEMP_DIR_NAME" -cf - "$FILE_NAME_LOGS" "$FILE_NAME_POSTGRES" "$FILE_NAME_REDIS" | age -r "$AGE_RECIPIENT" -o "$TMP_FILE_AGE"

	# Only promote the temp file to a completed backup once it's been validated
	check_age_file "$TMP_FILE_AGE"

	mv -f "$TMP_FILE_AGE" "$LOCATION_BACKUPS/$FILE_NAME_BACKUP"
	chmod 440 "$LOCATION_BACKUPS/$FILE_NAME_BACKUP"
	echo "backup created!"
}

cleanup_files() {
	echo "clean up temp files"
	find "$LOCATION_BACKUPS" -maxdepth 1 -type f -name '.*.tmp' -mtime +1 -delete
	find "$LOCATION_BACKUPS" -maxdepth 1 -type f -name '*.tar.age' -mtime +6 -delete
	find "$LOCATION_TEMP_BACKUP" -mindepth 1 -maxdepth 1 -mtime +1 -exec rm -rf {} +
}

main() {
	# Sleep, so that one can kill the container before it starts to do anything
	sleep 60

	TIMESTAMP=$(date +%F_%H.%M.%S)

	LOCATION_TEMP_BACKUP=/temp_backup
	LOCATION_BACKUPS=/backups
	LOCATION_ALL_LOGS=/logs
	
	# Safety net: ensure base directories exist
	mkdir -p "$LOCATION_TEMP_BACKUP" "$LOCATION_BACKUPS"

	FILE_NAME_REDIS="redis_data.rdb.gz"
	FILE_NAME_LOGS="logs.tar.gz"
	FILE_NAME_POSTGRES="pg_dump.tar.gz"
	FILE_NAME_BACKUP="adsbdb_${TIMESTAMP}_SQL_REDIS_LOGS.tar.age"

	# Create tmp dir using random string
	TEMP_DIR_NAME=$(mktemp -d "$LOCATION_TEMP_BACKUP/backup.XXXXXX")
	REDIS_DUMP_FILE="$TEMP_DIR_NAME/redis_dump.rdb"
	TMP_FILE_AGE="$LOCATION_BACKUPS/.${FILE_NAME_BACKUP}.tmp"

	# Move into backup directory
	cd "$LOCATION_BACKUPS" || exit 1

	# Remove working dir and any partial age output on exit
	cleanup() {
		[ -n "$TEMP_DIR_NAME" ] && rm -rf "$TEMP_DIR_NAME"
		[ -n "$TMP_FILE_AGE" ] && rm -f "$TMP_FILE_AGE"
	}

	# Trap EXIT for normal cleanup, and trap signals to simply exit (which triggers EXIT)
	trap cleanup EXIT
	trap 'exit 1' INT TERM

	backup_logs
	backup_redis
	backup_postgres

	combine_encrypt_files

	cleanup_files
}

check_env
main
exit 0