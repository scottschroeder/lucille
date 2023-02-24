#!/bin/bash

DB_PATH=$(echo $DATABASE_URL | sed -r 's/^sqlite:(.*)$/\1/')

if [[ -z "$DB_PATH" ]]; then
  exit 1
fi

sqlite3 "$DB_PATH" ".schema"
