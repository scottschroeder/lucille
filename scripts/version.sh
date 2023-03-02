#!/bin/bash

sed -i "s/^version = \".*\"/version = \"${1}\"/" gui/Cargo.toml
sed -i "s/^version = \".*\"/version = \"${1}\"/" cli/Cargo.toml
