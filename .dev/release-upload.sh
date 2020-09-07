#!/bin/sh

if [ ! -f "/tmp/$1" ]; then
  echo "$1 - file not found in /tmp"
  exit 2
fi

GITHUB_TOKEN=$(cat ~/.keys/github)
ID=$(curl -s https://api.github.com/repos/alttch/pulr/releases | jq -r .\[0\].id)
UPLOAD_ID=$(curl -X POST -H "Authorization: token ${GITHUB_TOKEN}" \
    -H "Content-Type: application/gzip" \
    --data-binary @/tmp/$1 \
    "https://uploads.github.com/repos/alttch/pulr/releases/${ID}/assets?name=$1" | jq -r .id)
[ -z "$UPLOAD_ID" ] && exit 1
[ "$UPLOAD_ID" = "null" ] && exit 1
echo "Uploaded $1"
exit 0
