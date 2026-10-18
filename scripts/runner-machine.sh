#!/usr/bin/env bash
# The runner as every benchmark result names it: image, OS, CPU model, vCPUs.
set -euo pipefail
os=$(. /etc/os-release && echo "$PRETTY_NAME")
cpu=$(lscpu | sed -n 's/^Model name:[[:space:]]*//p')
echo "GitHub ubuntu-latest, $os, $cpu, $(nproc) vCPU"
