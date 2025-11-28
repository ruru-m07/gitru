#!/bin/bash

if [ ! -f apps/api/.env ]; then
  echo "Linking .env file to apps/api/.env..."
  link .env apps/api/.env
fi

if [ ! -f apps/web/.env ]; then
  echo "Linking .env file to apps/web/.env..."
  link .env apps/web/.env
fi
