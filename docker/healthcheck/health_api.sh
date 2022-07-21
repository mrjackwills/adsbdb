#!/bin/sh

API_PORT=$(grep "API_PORT" /app_env/.api.env | cut -c 10-13)
URL="adsbdb_api:${API_PORT}/v1/incognito/online"
wget -nv -t1 --spider "${URL}" || exit 1