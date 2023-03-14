#!/bin/bash

set -ex

# echo $GH_TOKEN | docker login ghcr.io -u $GH_USER --password-stdin
docker pull ghcr.io/young-naive-simple/new-bing-api:latest
docker pull ghcr.io/young-naive-simple/new-bing-bot:latest

echo 'cookies:
  - 123
  - 456
' > /tmp/new_bing_cookies.yaml

PREV_DIR=$(pwd)
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

export API_HOST=
export TELOXIDE_TOKEN=
cd $SCRIPT_DIR
docker compose down
docker compose up -d
cd $PREV_DIR

yes | docker image prune
docker ps -a
