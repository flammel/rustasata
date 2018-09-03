cargo build --release;

for f in test/satlib/uf20*/*; do
    echo "";
    echo $f;
    NO_LOG=1 ./target/release/rustasata $f;
done