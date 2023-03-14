#!/bin/bash

set -ex

PREV_DIR=$(pwd)
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd $SCRIPT_DIR
docker compose down
cd $PREV_DIR

docker ps -a
