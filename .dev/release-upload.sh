#!/bin/sh

GITHUB_TOKEN=$(cat ~/.keys/github)
ID=$(curl -s https://api.github.com/repos/alttch/pulr/releases | jq -r .\[0\].id)
UPLOAD_ID=$(curl -X POST -H "Authorization: token ${GITHUB_TOKEN}" \
    -H "Content-Type: application/gzip" \
    --data-binary @/tmp/pulr.x86_64.gz \
    "https://uploads.github.com/repos/alttch/pulr/releases/${ID}/assets?name=pulr.x86_64.gz" | jq -r .id)
[ -z "$UPLOAD_ID" ] && exit 1
[ "$UPLOAD_ID" = "null" ] && exit 1
echo "Uploaded"
exit 0
