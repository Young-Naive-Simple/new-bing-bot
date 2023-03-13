#!/bin/bash

set -ex

echo $GH_TOKEN | docker login ghcr.io -u $GH_USER --password-stdin
docker pull ghcr.io/young-naive-simple/new-bing-api:latest
docker pull ghcr.io/young-naive-simple/new-bing-bot:latest

set +e
docker kill new-bing-api new-bing-bot
docker rm new-bing-api new-bing-bot
docker rm new-bing-net
set -e

echo 'cookies:
  - 123
  - 456
' > /tmp/new_bing_cookies.yaml

docker run -d --name new-bing-api --restart=always \
    --net new-bing-net \
    -p 127.0.0.1:30180:3000 \
    -v /tmp/new_bing_cookies.yaml:/app/cookies.yaml \
    -v /etc/localtime:/etc/localtime \
    ghcr.io/young-naive-simple/new-bing-api:latest

docker run -d --name new-bing-bot --restart=always \
    --net new-bing-net \
    -v /etc/localtime:/etc/localtime \
    --env TELOXIDE_TOKEN=fff:abc \
    --env API_HOST=new-bing-api \
    ghcr.io/young-naive-simple/new-bing-bot:latest

docker ps -a
