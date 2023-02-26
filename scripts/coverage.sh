#!/bin/bash

if ! command -v grcov &> /dev/null
then
    echo "grcov could not be found"
    exit
fi

coverage_dir="coverage"
rm -rfv "${coverage_dir}"
mkdir -pv "${coverage_dir}"

CARGO_INCREMENTAL=0 RUSTFLAGS="-Cinstrument-coverage" LLVM_PROFILE_FILE="cargo-test-%p-%m.profraw" cargo test

# grcov . --binary-path ./target/debug/deps -s . -t html --branch --ignore-not-existing --ignore ../* -o "${coverage_dir}/html"
grcov . --binary-path ./target/debug/deps -s . -t html --branch --ignore-not-existing --ignore ./database --ignore ../* -o "${coverage_dir}/html"
# grcov . --binary-path ./target/debug/deps -s . -t lcov --branch --ignore-not-existing --ignore ./target/debug/* --ignore ../* -o "${coverage_dir}/tests.lcov"

rm -v **/*.profraw
rm -v *.profraw

echo "${coverage_dir}/html"
