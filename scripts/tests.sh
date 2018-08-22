cargo build --release;

for f in test/easy/*; do
    echo "";
    echo $f;
    NO_LOG=1 ./target/release/rustasata $f;
done