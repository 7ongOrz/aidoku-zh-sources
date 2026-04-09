#!/bin/bash

for src in ./src/as/*; do
  if grep -qxF "${src#./}" .deprecated-sources; then
    rm -f "$src/build/package.aix"
  else
    (cd "$src" && npm run build)
  fi
done

for src in ./src/rust/*; do
  if grep -qxF "${src#./}" .deprecated-sources; then
    rm -f "$src/package.aix"
  else
    aidoku package "$src"
  fi
done

aidoku build ./src/**/*.aix -n "Aidoku 中文图源"
