#!/bin/bash

docker kill new-bing-api new-bing-bot
docker rm new-bing-api new-bing-bot
docker network rm new-bing-net
