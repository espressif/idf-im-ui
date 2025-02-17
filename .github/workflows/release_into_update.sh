#!/bin/bash

# Check if input file is provided
if [ $# -lt 1 ]; then
    echo "Usage: $0 <github_release_json_file>"
    exit 1
fi

input_file="$1"

# Create the JSON structure using jq
jq '
{
  version: .tag_name,
  notes: .body,
  pub_date: .published_at,
  platforms: {
    "linux-x86_64": {
      signature: (.assets[] | select(.name | contains("linux-x86_64")) | .browser_download_url + ".sig"),
      url: (.assets[] | select(.name | contains("linux-x86_64")) | .browser_download_url)
    },
    "windows-x86_64": {
      signature: (.assets[] | select(.name | contains("windows-x86_64")) | .browser_download_url + ".sig"),
      url: (.assets[] | select(.name | contains("windows-x86_64")) | .browser_download_url)
    },
    "darwin-x86_64": {
      signature: (.assets[] | select(.name | contains("macos-x86_64")) | .browser_download_url + ".sig"),
      url: (.assets[] | select(.name | contains("macos-x86_64")) | .browser_download_url)
    },
    "darwin-aarch64": {
      signature: (.assets[] | select(.name | contains("macos-aarm64")) | .browser_download_url + ".sig"),
      url: (.assets[] | select(.name | contains("macos-aarm64")) | .browser_download_url)
    }
  }
}' "$input_file" > "update.json"

# Validate the output file exists
if [ -f "update.json" ]; then
    echo "Successfully created update.json"
else
    echo "Failed to create update.json"
    exit 1
fi