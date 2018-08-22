#! /bin/bash

for f in test/*/*; do
    sed -i '/^%$/d' $f
    sed -i '/^0$/d' $f
done