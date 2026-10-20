#!/usr/bin/env bash
# Installs the right-click entry for Nemo (Linux Mint): same as install-nautilus.sh with MANDARINA_FM=nemo.
# Usage / Uso: packaging/install-nemo.sh [path/to/mandarina]
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MANDARINA_FM=nemo exec bash "$here/install-nautilus.sh" "$@"
