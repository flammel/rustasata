cargo build --release;

for f in test/satlib/uf125*/*; do
    echo "";
    echo $f;
    ./target/release/rustasata $f;
    # ./minisat $f;
done