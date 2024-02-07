#!/bin/bash

# rust create_release
# v0.5.

STAR_LINE='****************************************'
CWD=$(pwd)

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
PURPLE='\033[0;35m'
RESET='\033[0m'


# $1 string - error message
error_close() {
	echo -e "\n${RED}ERROR - EXITED: ${YELLOW}$1${RESET}\n";
	exit 1
}

# Check that dialog is installed
if ! [ -x "$(command -v dialog)" ]; then
	error_close "dialog is not installed"
fi

# $1 string - question to ask
ask_yn () {
	printf "%b%s? [y/N]:%b " "${GREEN}" "$1" "${RESET}"
}

# return user input
user_input() {
	read -r data
	echo "$data"
}

update_major () {
	local bumped_major
	bumped_major=$((MAJOR + 1))
	echo "${bumped_major}.0.0"
}

update_minor () {
	local bumped_minor
	bumped_minor=$((MINOR + 1))
	MINOR=bumped_minor
	echo "${MAJOR}.${bumped_minor}.0"
}

update_patch () {
	local bumped_patch
	bumped_patch=$((PATCH + 1))
	PATCH=bumped_patch
	echo "${MAJOR}.${MINOR}.${bumped_patch}"
}

# Get the url of the github repo, strip .git from the end of it
get_git_remote_url() {
	GIT_REPO_URL="$(git config --get remote.origin.url | sed 's/\.git$//')"
}

# Check that git status is clean
check_git_clean() {
	GIT_CLEAN=$(git status --porcelain)
	if [[ -n $GIT_CLEAN ]]
	then
		error_close "git dirty"
	fi
}

# Check currently on dev branch
check_git() {
	CURRENT_GIT_BRANCH=$(git branch --show-current)
	check_git_clean
	if [[ ! "$CURRENT_GIT_BRANCH" =~ ^dev$ ]]
	then
		error_close "not on dev branch"
	fi
}

# Ask user if current changelog is acceptable
ask_changelog_update() {
	echo "${STAR_LINE}"
	RELEASE_BODY_TEXT=$(sed '/# <a href=/Q' CHANGELOG.md)
	printf "%s" "$RELEASE_BODY_TEXT"
	printf "\n%s\n" "${STAR_LINE}"
	ask_yn "accept release body"
	if [[ "$(user_input)" =~ ^y$ ]] 
	then
		update_release_body_and_changelog "$RELEASE_BODY_TEXT"
	else
		exit
	fi
}

# Edit the release-body to include new lines from changelog
# add commit urls to changelog
# $1 RELEASE_BODY 
update_release_body_and_changelog () {
	echo -e
	DATE_SUBHEADING="### $(date +'%Y-%m-%d')\n\n"
	RELEASE_BODY_ADDITION="${DATE_SUBHEADING}$1"

	# Put new changelog entries into release-body, add link to changelog
	echo -e "${RELEASE_BODY_ADDITION}\n\nsee <a href='${GIT_REPO_URL}/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details" > .github/release-body.md

	# Add subheading with release version and date of release
	echo -e "# <a href='${GIT_REPO_URL}/releases/tag/${NEW_TAG_WITH_V}'>${NEW_TAG_WITH_V}</a>\n${DATE_SUBHEADING}${CHANGELOG_ADDITION}$(cat CHANGELOG.md)" > CHANGELOG.md

	# Update changelog to add links to commits [hex:8](url_with_full_commit)
	# "[aaaaaaaaaabbbbbbbbbbccccccccccddddddddd]" -> "[aaaaaaaa](https:/www.../commit/aaaaaaaaaabbbbbbbbbbccccccccccddddddddd),"
	sed -i -E "s=(\s)\[([0-9a-f]{8})([0-9a-f]{32})\]= [\2](${GIT_REPO_URL}/commit/\2\3)=g" ./CHANGELOG.md

	# Update changelog to add links to closed issues - comma included!
	# "closes #1" -> "closes [#1](https:/www.../issues/1)"
	sed -i -r -E "s=closes \#([0-9]+)=closes [#\1](${GIT_REPO_URL}/issues/\1)=g" ./CHANGELOG.md
}

update_version_number_in_files () {
	# update version in cargo.toml, to match selected current version
	sed -i "s|^version = .*|version = \"${MAJOR}.${MINOR}.${PATCH}\"|" Cargo.toml

	# update nginx api config location with new major version
	sed -i -r -E "s=location \/v[0-9]+\/=location /v${MAJOR}\/=g" ./nginx_confs/api.adsbdb.com.conf

	# update docker api healthcheck
	sed -i -r -E "s=v[0-9]+=v${MAJOR}=g" ./docker/healthcheck/health_api.sh

	# update docker compose image version
	sed -i -r -E "s=image: (\w+):[0-9]+\.[0-9]+\.[0-9]+=image: \1:${MAJOR}.${MINOR}.${PATCH}=g" ./docker/docker-compose.yml

	# update dev-docker compose image version
	sed -i -r -E "s=image: (\w+):[0-9]+\.[0-9]+\.[0-9]+=image: \1:${MAJOR}.${MINOR}.${PATCH}=g" ./docker/dev.docker-compose.yml

	# update endpoint in website js
	sed -i -r -E "s=https://api.adsbdb.com/v[0-9]+/online=https://api.adsbdb.com/v${MAJOR}/online=g" ./site/online.js

	# update endpoint in website html
	sed -i -r -E "s=https://api.adsbdb.com/v[0-9]+=https://api.adsbdb.com/v${MAJOR}=g" ./site/index.html

	# Update version number on api dockerfile, to download latest release from github
	# sed -i -r -E "s=download/v[0-9]+.[0-9]+.[0-9]+=download/v${MAJOR}.${MINOR}.${PATCH}=g" ./docker/dockerfile/api.Dockerfile
}

# Work out the current version, based on git tags
# create new semver version based on user input
# Set MAJOR MINOR PATCH
check_tag () {
	LATEST_TAG=$(git describe --tags "$(git rev-list --tags --max-count=1)")
	echo -e "\nCurrent tag: ${PURPLE}${LATEST_TAG}${RESET}\n"
	echo -e "${YELLOW}Choose new tag version:${RESET}\n"
	if [[ $LATEST_TAG =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-((0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*)(\.(0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*))*))?(\+([0-9a-zA-Z-]+(\.[0-9a-zA-Z-]+)*))?$ ]]
	then
		IFS="." read -r MAJOR MINOR PATCH <<< "${LATEST_TAG:1}"
	else
		MAJOR="0"
		MINOR="0"
		PATCH="0"
	fi
	OP_MAJOR="major___v$(update_major)"
	OP_MINOR="minor___v$(update_minor)"
	OP_PATCH="patch___v$(update_patch)"
	OPTIONS=("$OP_MAJOR" "$OP_MINOR" "$OP_PATCH")
	select choice in "${OPTIONS[@]}"
	do
		case $choice in
			"$OP_MAJOR" )
				MAJOR=$((MAJOR + 1))
				MINOR=0
				PATCH=0
				break;;
			"$OP_MINOR")
				MINOR=$((MINOR + 1))
				PATCH=0
				break;;
			"$OP_PATCH")
				PATCH=$((PATCH + 1))
				break;;
			*)
				error_close "invalid option $REPLY"
		esac
	done
}

# ask continue, or quit
ask_continue () {
	ask_yn "continue"
	if [[ ! "$(user_input)" =~ ^y$ ]] 
	then 
		exit
	fi
}

# run all tests
cargo_test () {
	cargo test -- --test-threads=1
	ask_continue
}

# $1 text to colourise
release_continue () {
	echo -e "\n${PURPLE}$1${RESET}"
	ask_continue
}

# Check for any typos
check_typos () {
	echo -e "\n${YELLOW}checking for typos${RESET}"
	typos
	ask_continue
}

#  Make sure the unused lint isn't used
check_allow_unused() {
	matches_any=$(find . -type d \( -name .git -o -name target \) -prune -o -type f -exec grep -lE '^#!\[allow\(unused\)\]$' {} +)
	matches_cargo=$(grep "^unused = \"allow\"" ./Cargo.toml)
	if [ -n "$matches_any" ]; then
		error_close "\"#[allow(unused)]\" in ${matches_any}"
	elif [ -n "$matches_cargo" ]; then
		error_close "\"unused = \"allow\"\" in Cargo.toml"
	fi
}

# Create sqlx-data.json file for offline mode
sqlx_prepare () {
	echo -e "\n${YELLOW}cargo sqlx prepare${RESET}"
	cargo sqlx prepare
	ask_continue
}

# Full flow to create a new release
release_flow() {
	check_allow_unused
	check_typos

	check_git
	get_git_remote_url

	cargo_test
	cargo_build_all

	cd "${CWD}" || error_close "Can't find ${CWD}"
	check_tag
	
	NEW_TAG_WITH_V="v${MAJOR}.${MINOR}.${PATCH}"
	printf "\nnew tag chosen: %s\n\n" "${NEW_TAG_WITH_V}"

	RELEASE_BRANCH=release-$NEW_TAG_WITH_V
	echo -e
	ask_changelog_update
	
	release_continue "checkout ${RELEASE_BRANCH}"
	git checkout -b "$RELEASE_BRANCH"

	release_continue "update_version_number_in_files"
	update_version_number_in_files
	
	echo "cargo fmt"
	cargo fmt

	echo -e "\n${PURPLE}cargo check${RESET}"
	cargo check

	sqlx_prepare

	release_continue "git add ."
	git add .

	release_continue "git commit -m \"chore: release \"${NEW_TAG_WITH_V}\""
	git commit -m "chore: release ${NEW_TAG_WITH_V}"

	release_continue "git checkout main"
	git checkout main

	echo -e "${PURPLE}git pull origin main${RESET}"
	git pull origin main

	echo -e "git merge --no-ff \"${RELEASE_BRANCH}\" -m \"chore: merge ${RELEASE_BRANCH} into main\"" 
	git merge --no-ff "$RELEASE_BRANCH" -m "chore: merge ${RELEASE_BRANCH} into main"

	release_continue "git tag -am \"${RELEASE_BRANCH}\" \"$NEW_TAG_WITH_V\""
	git tag -am "${RELEASE_BRANCH}" "$NEW_TAG_WITH_V"

	release_continue "git push --atomic origin main \"$NEW_TAG_WITH_V\""
	git push --atomic origin main "$NEW_TAG_WITH_V"

	release_continue "git checkout dev"
	git checkout dev

	release_continue "git merge --no-ff main -m \"chore: merge main into dev\""
	git merge --no-ff main -m 'chore: merge main into dev'

	release_continue "git push origin dev"
	git push origin dev

	release_continue "git branch -d \"$RELEASE_BRANCH\""
	git branch -d "$RELEASE_BRANCH"
}

cargo_build_x86() {
	echo -e "${YELLOW}cargo build --release${RESET}"
	cargo build --release
}

cargo_build_aarch64_linux() {
	echo -e "${YELLOW}cross build --target aarch64-unknown-linux-gnu --release${RESET}"
	cross build --target aarch64-unknown-linux-gnu --release
}

# Build all releases that GitHub workflow would
# This will download GB's of docker images
cargo_build_all() {
	cargo install cross
	cargo_build_x86
	ask_continue
	cargo_build_aarch64_linux
	ask_continue
}

build_choice() {
	cmd=(dialog --backtitle "Choose option" --radiolist "choose" 14 80 16)
	options=(
		1 "x86" off
		2 "aarch64" off
		5 "all" off
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
			cargo_build_x86
			exit
			;;
		2)
			cargo_build_aarch64
			exit
			;;
		5)
			cargo_build_all
			exit
			;;
		esac
	done
}


main() {
	cmd=(dialog --backtitle "Choose option" --radiolist "choose" 14 80 16)
	options=(
		1 "test" off
		2 "release" off
		3 "build" off
	)
	choices=$("${cmd[@]}" "${options[@]}" 2>&1 >/dev/tty)
	exitStatus=$?
	clear
	if [ $exitStatus -ne 0 ]; then
		exit
	fi
	for choice in $choices
	do
		case $choice in
			0)
				exit;;
			1)
				cargo_test
				main
				break;;
			2)
				release_flow
				break;;
			3) 
				build_choice
				main
				break
			;;
		esac
	done
}

main