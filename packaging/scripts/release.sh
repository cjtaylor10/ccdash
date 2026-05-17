#!/usr/bin/env bash
# Build all release binaries + the SvelteKit frontend, then tar them into
# packaging/dist/. Intended to be run from the repo root.
set -euo pipefail

cd "$(dirname "$0")/../.."
ROOT="$(pwd)"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed -E 's/version *= *"(.+)"/\1/')
DIST="${ROOT}/packaging/dist"

echo "==> Building Rust binaries (--release)"
cargo build --release -p ccdash-daemon -p ccdash-cli -p ccdash-ui

echo "==> Building frontend"
(cd apps/ccdash-ui/ui && pnpm install --frozen-lockfile && pnpm run build)

echo "==> Bundling tarball"
rm -rf "${DIST}" && mkdir -p "${DIST}/ccdash-${VERSION}/bin" "${DIST}/ccdash-${VERSION}/share/ccdash"
cp target/release/ccdash "${DIST}/ccdash-${VERSION}/bin/"
cp target/release/ccdash-daemon "${DIST}/ccdash-${VERSION}/bin/"
cp target/release/ccdash-ui "${DIST}/ccdash-${VERSION}/bin/"
cp packaging/launchd/com.ccdash.daemon.plist "${DIST}/ccdash-${VERSION}/share/ccdash/"
cp packaging/systemd/ccdash-daemon.service "${DIST}/ccdash-${VERSION}/share/ccdash/"
cp packaging/scripts/install-service.sh "${DIST}/ccdash-${VERSION}/share/ccdash/"
cp packaging/scripts/uninstall-service.sh "${DIST}/ccdash-${VERSION}/share/ccdash/"

tar -C "${DIST}" -czf "${DIST}/ccdash-${VERSION}.tar.gz" "ccdash-${VERSION}"
SHA=$(shasum -a 256 "${DIST}/ccdash-${VERSION}.tar.gz" | awk '{print $1}')
echo
echo "Built ${DIST}/ccdash-${VERSION}.tar.gz"
echo "  sha256 = ${SHA}"
echo
echo "Update Formula/ccdash.rb 'sha256' with the value above."
