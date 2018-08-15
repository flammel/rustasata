.PHONY: release
release:
	cargo build --release

# for f in test/hard/*; do echo "$f"; cat "$f" | ./target/release/rustasata; done