#!/bin/bash

set -a

if [ -f ".env" ]; then
  . ./.env
else
  echo "⚠️  .env file not found"
fi

set +a
