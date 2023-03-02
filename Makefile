SHELL := /bin/bash

CARGO = cargo
CARGO_OPTS =

VERSION=$(shell grep -Em1 "^version" Cargo.toml | sed -r 's/.*"(.*)".*/\1/')
RUSTC_VERSION=$(shell rustc -V)
NAME := lucille
BUILD_DIR := ./build

MKFILE_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
MKFILE_DIR := $(dir $(MKFILE_PATH))

RUST_SRC := $(shell find . -name \*.rs -print)
DATABASE_SRC := $(shell find database/src -name \*.rs -print)

#.DEFAULT: config
.PHONY: build clean build-docker win-build-local win-build-docker fmt fix test cov

fmt:
	cargo +nightly fmt

fix:
	cargo fix --allow-staged
	cargo clippy --fix --allow-staged --allow-dirty

test:
	cargo test


pre-commit: fix fmt test

build:
	$(CARGO) $(CARGO_OPTS) build --release

clean:
	$(CARGO) $(CARGO_OPTS) clean

schema: schema.sql

cov:
	./scripts/coverage.sh

schema.sql: scripts/dump_schema.sh database/migrations/*
	./scripts/dump_schema.sh > schema.sql

sqlx-data.json: $(DATABASE_SRC)
	cargo sqlx prepare --merged

build-docker:
	docker build . -t rust_cross_compile/windows -f Dockerfile.windows

win-build-docker: build-docker Dockerfile.windows sqlx-data.json
	docker run -v $(MKFILE_DIR):/app -it rust_cross_compile/windows

win-build-local:
	cargo build --release --target x86_64-pc-windows-gnu
