#!/bin/sh

set -e

cargo install --path .
desktop-file-install rss-list.desktop --dir ~/.local/share/applications

