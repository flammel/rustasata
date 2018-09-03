#! /bin/bash

for f in test/satlib/*/*; do
    sed -i '/^%$/d' $f
    sed -i '/^0$/d' $f
done