#!/bin/sh
set -e

# Automatically updated via create-release.sh, if major bump
wget -nv -t1 --spider "${API_HOST}:${API_PORT}/v0/online" || exit 1