#!/bin/bash

# rust create_release
# v0.0.15

PACKAGE_NAME='adsbdb'
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

if [ -z "$PACKAGE_NAME" ]
then
	error_close "No package name"
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
	REMOTE_ORIGIN=$(git config --get remote.origin.url)
	TO_REMOVE=".git"
	GIT_REPO_URL="${REMOTE_ORIGIN//$TO_REMOVE}"
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
	sed -i -E "s=(\s)\[([0-9a-f]{8})([0-9a-f]{32})\]= [\2](${GIT_REPO_URL}/commit/\2\3),=g" ./CHANGELOG.md

	# Update changelog to add links to closed issues - comma included!
	# "closes [#1]," -> "closes [#1](https:/www.../issues/1),""
	sed -i -r -E "s=closes \[#([0-9]+)\],=closes [#\1](${GIT_REPO_URL}/issues/\1),=g" ./CHANGELOG.md
}

# update version in cargo.toml, to match selected current version
update_version_number_in_files () {
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
	sed -i -r -E "s=download/v[0-9]+.[0-9]+.[0-9]+=download/v${MAJOR}.${MINOR}.${PATCH}=g" ./docker/dockerfile/api.Dockerfile
}

# Work out the current version, based on git tags
# create new semver version based on user input
# Set MAJOR MINOR PATCH
check_tag () {
	LATEST_TAG=$(git describe --tags --abbrev=0 --always)
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
				break;;
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

# build for production, imitate GitHub workflow
cargo_build () {
	cargo install cross
	cross build --target x86_64-unknown-linux-musl --release
	ask_continue
}

# Full flow to create a new release
release_flow() {
	check_git
	get_git_remote_url
	cargo_test
	cargo_build
	cd "${CWD}" || error_close "Can't find ${CWD}"
	check_tag
	
	NEW_TAG_WITH_V="v${MAJOR}.${MINOR}.${PATCH}"
	printf "\nnew tag chosen: %s\n\n" "${NEW_TAG_WITH_V}"
	RELEASE_BRANCH=release-$NEW_TAG_WITH_V
	echo -e
	ask_changelog_update
	git checkout -b "$RELEASE_BRANCH"
	update_version_number_in_files
	cargo fmt
	git add .
	git commit -m "chore: release $NEW_TAG_WITH_V"

	git checkout main
	git merge --no-ff "$RELEASE_BRANCH" -m "chore: merge ${RELEASE_BRANCH} into main"
	git tag -am "${RELEASE_BRANCH}" "$NEW_TAG_WITH_V"
	echo "git tag -am \"${RELEASE_BRANCH}\" \"$NEW_TAG_WITH_V\""
	git push --atomic origin main "$NEW_TAG_WITH_V"
	git checkout dev
	git merge --no-ff main -m 'chore: merge main into dev'
	git branch -d "$RELEASE_BRANCH"
	git push origin dev
}


main() {
	cmd=(dialog --backtitle "Choose option" --radiolist "choose" 14 80 16)
	options=(
		1 "test" off
		2 "release" off
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
				exit
				break;;
			1)
				cargo_test
				main
				break;;
			2)
				release_flow
				break;;
		esac
	done
}

main