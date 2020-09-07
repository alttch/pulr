#!/bin/sh

FILE="/tmp/$1"

if [ ! -f "$FILE" ]; then
  echo "$1 - file not found ($FILE)"
  exit 2
fi

MIME_TYPE=$(file --mime-type $FILE|awk '{ print $2 }')

GITHUB_TOKEN=$(cat ~/.keys/github)
ID=$(curl -s https://api.github.com/repos/alttch/pulr/releases | jq -r .\[0\].id)
UPLOAD_ID=$(curl -X POST -H "Authorization: token ${GITHUB_TOKEN}" \
    -H "Content-Type: $MIME_TYPE" \
    --data-binary @${FILE} \
    "https://uploads.github.com/repos/alttch/pulr/releases/${ID}/assets?name=$1" | jq -r .id)
[ -z "$UPLOAD_ID" ] && exit 1
[ "$UPLOAD_ID" = "null" ] && exit 1
echo "Uploaded $1 ($FILE : $MIME_TYPE)"
exit 0
