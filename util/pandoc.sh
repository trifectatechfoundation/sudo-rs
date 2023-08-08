#!/bin/bash

docspath=docs/man

docker run --rm --volume "`pwd`:/data" --user `id -u`:`id -g` pandoc/core -s -t man "$docspath/sudo.8.md" -o "$docspath/sudo.8"
docker run --rm --volume "`pwd`:/data" --user `id -u`:`id -g` pandoc/core -s -t man "$docspath/visudo.8.md" -o "$docspath/visudo.8"
