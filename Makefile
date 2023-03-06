SHELL := /bin/bash

CARGO = cargo
CARGO_OPTS =

VERSION=$(shell grep -Em1 "^version" Cargo.toml | sed -r 's/.*"(.*)".*/\1/')
RUSTC_VERSION=$(shell rustc -V)
NAME := lucille BUILD_DIR := ./build

MKFILE_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
MKFILE_DIR := $(dir $(MKFILE_PATH))

MIGRATIONS := $(shell find database/migrations -type f -print)
RUST_SRC := $(shell find . -name \*.rs -print)
DATABASE_SRC := $(shell find database/src -name \*.rs -print)
USER_ID := $(shell id -u)
GROUP_ID := $(shell id -g)

#.DEFAULT: config
.PHONY: build clean build-docker build-lambda-docker win-build-local win-build-docker fmt fix test cov dist-manifest

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

dist-manifest:
	cargo dist manifest --artifacts=all --no-local-paths

cov:
	./scripts/coverage.sh

schema.sql: scripts/dump_schema.sh sqlx-init
	./scripts/dump_schema.sh > schema.sql

sqlx-init: $(MIGRATIONS)
	cargo sqlx database setup --source database/migrations

sqlx-data.json: $(DATABASE_SRC) sqlx-init
	cargo sqlx prepare --merged

build-docker:
	docker build . -t rust_cross_compile/windows -f Dockerfile.windows

build-lambda-docker:
	docker build . -t rust_lambda_build -f Dockerfile.lambda

build-lambda: build-lambda-docker 
	# docker run --rm -u "$(USER_ID)":"$(GROUP_ID)" -v $(MKFILE_DIR):/code -v${HOME}/.cargo:/cargo rust_lambda_build
	# docker run --rm -u "$(USER_ID)":"$(GROUP_ID)" -v $(MKFILE_DIR):/code rust_lambda_build
	docker run --rm -u "$(USER_ID)":"$(GROUP_ID)" -v $(MKFILE_DIR):/code -v ${HOME}/.cargo/registry:/cargo/registry -v ${HOME}/.cargo/git:/cargo/git -e BIN=render rust_lambda_build

win-build-docker: build-docker Dockerfile.windows sqlx-data.json
	docker run --rm -u "$(USER_ID)":"$(GROUP_ID)" -v $(MKFILE_DIR):/app -v ${HOME}/.cargo/registry:/usr/local/cargo/registry -v ${HOME}/.cargo/git:/usr/local/cargo/git -it rust_cross_compile/windows

win-build-local:
	cargo build --release --target x86_64-pc-windows-gnu
