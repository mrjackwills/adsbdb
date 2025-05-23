#!/bin/bash

# 2025-05-21
# v0.4.0

APP_NAME='adsbdb'

RED='\033[0;31m'
YELLOW='\033[0;33m'
GREEN='\033[0;32m'
RESET='\033[0m'

PRO=production
DEV=dev

# Get the directory of the script
APP_DIR=$(dirname "$(readlink -f "$0")")

# $1 error text to print
error_close() {
	echo -e "\n${RED}ERROR - EXITED: ${YELLOW}$1${RESET}\n"
	exit 1
}

# $1 string - question to ask
# Ask a yes no question, only accepts `y` or `n` as a valid answer, returns 0 for yes, 1 for no
ask_yn() {
	while true; do
		printf "\n%b%s? [y/N]:%b " "${GREEN}" "$1" "${RESET}"
		read -r answer
		if [[ "$answer" == "y" ]]; then
			return 0
		elif [[ "$answer" == "n" ]]; then
			return 1
		else
			echo -e "${RED}\nPlease enter 'y' or 'n'${RESET}"
		fi
	done
}

# $1 any variable name
# $2 variable name
check_variable() {
	if [ -z "$1" ]; then
		error_close "Missing variable $2"
	fi
}

set_base_dir() {
	local workspace="/workspaces"
	if [[ -d "$workspace" ]]; then
		BASE_DIR="${workspace}"
	else
		BASE_DIR=$HOME
	fi
}

init() {
	# Check if the directory is empty
	if [ -z "$(ls -A "$APP_DIR")" ]; then
		error_close "The ${APP_DIR} directory is empty."
	fi

	if ! [ -x "$(command -v dialog)" ]; then
		error_close "dialog is not installed"
	fi
	set_base_dir

}

DOCKER_DIR="${APP_DIR}/docker"

# Containers
API="${APP_NAME}_api"
BACKUP_CONTAINER="${APP_NAME}_postgres_backup"
BASE_CONTAINERS=("${APP_NAME}_postgres" "${APP_NAME}_redis")
ALL=("${BASE_CONTAINERS[@]}" "${API}" "${BACKUP_CONTAINER}")
TO_RUN=("${BASE_CONTAINERS[@]}")

# Make directories for database data
make_db_data_directories() {
	local pg_data="${BASE_DIR}/databases.d/${APP_NAME}/pg_data"
	local redis_data="${BASE_DIR}/databases.d/${APP_NAME}/redis_data"
	local backups="${BASE_DIR}/databases.d/${APP_NAME}/backups"
	for DIRECTORY in $pg_data $redis_data $backups; do
		if [[ ! -d "$DIRECTORY" ]]; then
			echo -e "${GREEN}making directory:${RESET} \"$DIRECTORY\""
			mkdir -p "$DIRECTORY"
		fi
	done
}

make_logs_directories() {
	local logs_dir="$BASE_DIR/logs.d/${APP_NAME}"
	if [[ ! -d "$logs_dir" ]]; then
		echo -e "${GREEN}making directory:${RESET} \"$logs_dir\""
		mkdir -p "$logs_dir"
	fi
}

make_all_directories() {
	make_db_data_directories
	make_logs_directories
}

dev_up() {
	cd "${DOCKER_DIR}" || error_close "${DOCKER_DIR} doesn't exist"
	echo "starting containers: ${TO_RUN[*]}"
	docker compose -f dev.docker-compose.yml up --force-recreate --build -d "${TO_RUN[@]}"
	run_migrations
}

dev_down() {
	cd "${DOCKER_DIR}" || error_close "${DOCKER_DIR} doesn't exist"
	docker compose -f dev.docker-compose.yml down
}

production_up() {
	if ask_yn "added crontab \"15 3 * * *  docker restart ${APP_NAME}_postgres_backup\""; then
		make_all_directories
		cd "${DOCKER_DIR}" || error_close "${DOCKER_DIR} doesn't exist"
		docker compose -f docker-compose.yml up -d
		run_migrations
	else
		exit
	fi
}

production_rebuild() {
	if ask_yn "added crontab \"15 3 * * *  docker restart ${APP_NAME}_postgres_backup\""; then
		make_all_directories
		cd "${DOCKER_DIR}" || error_close "${DOCKER_DIR} doesn't exist"
		docker compose -f docker-compose.yml up -d --build
		run_migrations
	else
		exit
	fi
}

production_down() {
	cd "${DOCKER_DIR}" || error_close "${DOCKER_DIR} doesn't exist"
	docker compose -f docker-compose.yml down
}

select_containers() {
	cmd=(dialog --separate-output --backtitle "Dev containers selection" --checklist "select: postgres + redis +" 14 80 16)
	options=(
		1 "$API" off
		2 "$BACKUP_CONTAINER" off
	)
	choices=$("${cmd[@]}" "${options[@]}" 2>&1 >/dev/tty)
	exitStatus=$?
	clear
	if [ $exitStatus -ne 0 ]; then
		exit
	fi
	for choice in $choices; do
		case $choice in
		0)
			exit
			;;
		1)
			TO_RUN=("${TO_RUN[@]}" "${API}")
			;;
		2)
			TO_RUN=("${TO_RUN[@]}" "${BACKUP_CONTAINER}")
			;;
		esac
	done
	dev_up
}

git_pull_branch() {
	git checkout -- .
	git checkout main
	git pull origin main
	git fetch --tags
	latest_tag=$(git tag | sort -V | tail -n 1)
	git checkout -b "$latest_tag"
	sleep 5
}

pull_branch() {
	GIT_CLEAN=$(git status --porcelain)
	if [ -n "$GIT_CLEAN" ]; then
		echo -e "\n${RED}GIT NOT CLEAN${RESET}\n"
		printf "%s\n" "${GIT_CLEAN}"
	fi
	if [[ -n "$GIT_CLEAN" ]]; then
		if ! ask_yn "Happy to clear git state"; then
			exit
		fi
	fi
	git_pull_branch
	main
}

run_migrations() {
	if ask_yn "run init_postgres.sh"; then
		docker exec -it "${APP_NAME}_postgres" /docker-entrypoint-initdb.d/init_postgres.sh "migrations"
	fi
}

main() {
	echo "in main"
	cmd=(dialog --backtitle "Start ${APP_NAME} containers" --radiolist "choose environment" 14 80 16)
	options=(
		1 "${DEV} up" off
		2 "${DEV} down" off
		3 "${PRO} up" off
		4 "${PRO} down" off
		5 "${PRO} rebuild" off
		6 "pull & branch" off
	)
	choices=$("${cmd[@]}" "${options[@]}" 2>&1 >/dev/tty)
	exitStatus=$?
	clear
	if [ $exitStatus -ne 0 ]; then
		exit
	fi
	for choice in $choices; do
		case $choice in
		0)
			exit
			;;
		1)
			select_containers
			break
			;;
		2)
			dev_down
			break
			;;
		3)
			echo "production up: ${ALL[*]}"
			production_up
			break
			;;
		4)
			production_down
			break
			;;
		5)
			production_rebuild
			break
			;;
		6)
			pull_branch
			;;
		esac
	done
}

init
main
