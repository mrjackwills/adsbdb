#!/bin/sh
set -e

main() {
	port=$(grep "API_PORT" /app_env/.api.env | cut -c 10-13)
	url="adsbdb_api:${port}/v0/online"

	# Make the request using wget and process the response
	response=$(wget -S -O - --timeout=3 "$url" 2>&1)

	# Extract the status code
	status_code=$(echo "$response" | grep -oP 'HTTP/[0-9\.]+\s+\K[0-9]+')

	# Extract the uptime value from the JSON response
	uptime=$(echo "$response" | grep -oP '\{.*\}' | grep -oP '"uptime":\K[0-9]+')

	# Check if the status code is 200 and uptime is a valid number
	if [ "$status_code" = "200" ]; then
		case "$uptime" in
		[0-9]*)
			echo "200 OK with valid uptime field: $uptime"
			exit 0
			;;
		*)
			echo "Error: Uptime field is missing or invalid"
			exit 1
			;;
		esac
	else
		echo "Error: Status code is not 200"
		exit 1
	fi
}

main
