SHELL := /bin/bash

CARGO = cargo
CARGO_OPTS =

VERSION=$(shell grep -Em1 "^version" Cargo.toml | sed -r 's/.*"(.*)".*/\1/')
RUSTC_VERSION=$(shell rustc -V)
NAME := lucile
BUILD_DIR := ./build

MKFILE_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
MKFILE_DIR := $(dir $(MKFILE_PATH))

#.DEFAULT: config
.PHONY: all build clean

build:
	$(CARGO) $(CARGO_OPTS) build --release

clean:
	$(CARGO) $(CARGO_OPTS) clean

sqlx-prepare:
	cargo sqlx prepare --merged

build-docker: sqlx-prepare
	docker build . -t rust_cross_compile/windows -f Dockerfile.windows

win-build-docker: build-docker
	docker run -v $(MKFILE_DIR):/app -it rust_cross_compile/windows

win-build-local:
	cargo build --release --target x86_64-pc-windows-gnu
