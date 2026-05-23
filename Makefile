# Deskdrop top-level Makefile
#
# Usage:
#   make               # build core + CLI for host platform
#   make test          # run all tests
#   make bench         # run benchmarks
#   make macos         # build macOS .app
#   make windows       # build Windows .exe + DLL
#   make android       # build Android APK (all ABIs)
#   make linux         # build Linux binaries
#   make all           # build everything
#   make clean         # remove build artifacts
#   make audit         # security audit
#   make fmt           # auto-format all Rust code
#   make lint          # clippy with -D warnings
#   make docs          # open rustdoc in browser
#   make release TAG=v0.1.0  # tag and push a release

SHELL   := /bin/bash
CARGO   ?= cargo
RUSTUP  ?= rustup
ANDROID_ABIS := aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

# ── Colours ───────────────────────────────────────────────────────────────────
GREEN  := \033[0;32m
YELLOW := \033[0;33m
CYAN   := \033[0;36m
RESET  := \033[0m

.PHONY: all build test bench macos windows android linux clean audit fmt lint \
        docs release check install-tools help

# ── Default: build core + CLI ─────────────────────────────────────────────────

build:
	@echo -e "$(CYAN)Building deskdrop-core + deskdrop-cli...$(RESET)"
	$(CARGO) build --release
	@echo -e "$(GREEN)✓ Build complete.$(RESET)"
	@echo "  Daemon: target/release/deskdrop-daemon"
	@echo "  CLI:    target/release/deskdrop-cli"

# ── Test ──────────────────────────────────────────────────────────────────────

test:
	@echo -e "$(CYAN)Running all tests...$(RESET)"
	$(CARGO) test --lib --doc 2>&1
	@echo ""
	@echo -e "$(CYAN)Running integration + mesh + crypto-vector tests...$(RESET)"
	$(CARGO) test --tests 2>&1
	@echo -e "$(GREEN)✓ All tests passed.$(RESET)"

test-unit:
	$(CARGO) test --lib

test-integration:
	$(CARGO) test --tests

test-crypto:
	$(CARGO) test --test crypto_vectors_test

test-mesh:
	$(CARGO) test --test mesh_test

test-e2e:
	$(CARGO) test --test e2e_test

# ── Benchmarks ────────────────────────────────────────────────────────────────

bench:
	@echo -e "$(CYAN)Running benchmarks (HTML report: target/criterion/report/index.html)$(RESET)"
	$(CARGO) bench
	@echo -e "$(GREEN)✓ Benchmarks complete.$(RESET)"

bench-check:
	$(CARGO) bench --no-run

# ── Code quality ──────────────────────────────────────────────────────────────

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

lint:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

check: fmt-check lint test-unit
	@echo -e "$(GREEN)✓ All checks passed.$(RESET)"

audit:
	@echo -e "$(CYAN)Running cargo audit...$(RESET)"
	$(CARGO) audit
	@echo -e "$(GREEN)✓ No known vulnerabilities.$(RESET)"

sbom:
	$(CARGO) cyclonedx --format json --output-file bom.json
	@echo "SBOM written to bom.json"

docs:
	$(CARGO) doc --open --no-deps

# ── Platform builds ───────────────────────────────────────────────────────────

macos:
	@echo -e "$(CYAN)Building macOS universal binary...$(RESET)"
	$(RUSTUP) target add aarch64-apple-darwin x86_64-apple-darwin 2>/dev/null || true
	$(CARGO) build --release --target aarch64-apple-darwin
	$(CARGO) build --release --target x86_64-apple-darwin
	lipo -create \
		deskdrop-core/target/aarch64-apple-darwin/release/libdeskdrop_core.dylib \
		deskdrop-core/target/x86_64-apple-darwin/release/libdeskdrop_core.dylib \
		-output libdeskdrop_core.dylib
	@echo -e "$(GREEN)✓ Universal dylib: libdeskdrop_core.dylib$(RESET)"
	@echo "  Next: bash scripts/build-macos.sh"

windows:
	@echo -e "$(CYAN)Building Windows DLL + C# app...$(RESET)"
	$(CARGO) build --release
	cp deskdrop-core/target/release/deskdrop_core.dll \
	   platforms/windows/Deskdrop.Windows/
	dotnet build platforms/windows/Deskdrop.Windows/Deskdrop.Windows.csproj \
		-c Release
	@echo -e "$(GREEN)✓ Windows build complete.$(RESET)"

android: install-android-targets
	@echo -e "$(CYAN)Building Android APK ($(ANDROID_ABIS))...$(RESET)"
	$(MAKE) -C . _android-native
	cd platforms/android && ./gradlew assembleRelease
	@echo -e "$(GREEN)✓ APK: platforms/android/app/build/outputs/apk/release/$(RESET)"

_android-native:
	cargo ndk \
		-p 26 \
		-t aarch64-linux-android \
		-t armv7-linux-androideabi \
		-t x86_64-linux-android \
		-o platforms/android/app/src/main/jniLibs \
		build --lib --release -p deskdrop-core

linux:
	@echo -e "$(CYAN)Building Linux binaries...$(RESET)"
	$(CARGO) build --release \
		--bin deskdrop-daemon \
		--bin deskdrop-cli
	@echo -e "$(GREEN)✓ Linux build complete.$(RESET)"
	@echo "  Daemon: target/release/deskdrop-daemon"
	@echo "  CLI:    target/release/deskdrop-cli"
	@echo ""
	@echo "  Install:"
	@echo "    sudo cp target/release/deskdrop-daemon /usr/local/bin/"
	@echo "    sudo cp target/release/deskdrop-cli    /usr/local/bin/"
	@echo "    cp platforms/linux/deskdrop.service ~/.config/systemd/user/"
	@echo "    systemctl --user enable --now deskdrop"

all: build macos linux android windows
	@echo -e "$(GREEN)✓ All platforms built.$(RESET)"

# ── Install dev tools ─────────────────────────────────────────────────────────

install-tools:
	$(CARGO) install cargo-audit      --locked
	$(CARGO) install cargo-ndk        --locked
	$(CARGO) install cargo-cyclonedx  --locked
	$(CARGO) install cargo-watch      --locked

install-android-targets:
	$(RUSTUP) target add $(ANDROID_ABIS) 2>/dev/null || true

# ── Release ───────────────────────────────────────────────────────────────────

release:
ifndef TAG
	$(error TAG is required. Usage: make release TAG=v0.1.0)
endif
	@echo -e "$(CYAN)Releasing $(TAG)...$(RESET)"
	@grep -q "## \[Unreleased\]" CHANGELOG.md || (echo "Update CHANGELOG.md first"; exit 1)
	git tag -a $(TAG) -m "Release $(TAG)"
	git push origin $(TAG)
	@echo -e "$(GREEN)✓ Tag $(TAG) pushed. GitHub Actions will build and publish.$(RESET)"

# ── Install locally (Linux/macOS) ─────────────────────────────────────────────

install: build
	install -Dm755 deskdrop-core/target/release/deskdrop-daemon \
		$(DESTDIR)$(PREFIX)/bin/deskdrop-daemon
	install -Dm755 deskdrop-core/target/release/deskdrop-cli \
		$(DESTDIR)$(PREFIX)/bin/deskdrop-cli
	@echo -e "$(GREEN)✓ Installed to $(DESTDIR)$(PREFIX)/bin/$(RESET)"

PREFIX ?= /usr/local

# ── Clean ─────────────────────────────────────────────────────────────────────

clean:
	$(CARGO) clean
	rm -f libdeskdrop_core.dylib deskdrop_core.dll
	rm -f bom.json
	rm -rf platforms/android/app/build
	rm -rf platforms/windows/Deskdrop.Windows/bin
	rm -rf platforms/windows/Deskdrop.Windows/obj
	@echo -e "$(GREEN)✓ Clean.$(RESET)"

# ── Help ──────────────────────────────────────────────────────────────────────

help:
	@echo ""
	@echo -e "$(CYAN)Deskdrop Build System$(RESET)"
	@echo ""
	@echo "  make              Build core + CLI for host"
	@echo "  make test         Run all tests"
	@echo "  make bench        Run Criterion benchmarks"
	@echo "  make lint         Clippy with -D warnings"
	@echo "  make fmt          Auto-format Rust code"
	@echo "  make audit        Security vulnerability scan"
	@echo "  make docs         Build and open rustdoc"
	@echo ""
	@echo "  make macos        Build macOS universal dylib"
	@echo "  make windows      Build Windows DLL + C# app"
	@echo "  make android      Build Android APK (all ABIs)"
	@echo "  make linux        Build Linux daemon + CLI"
	@echo "  make all          Build all platforms"
	@echo ""
	@echo "  make clean        Remove all build artifacts"
	@echo "  make install      Install to PREFIX=/usr/local"
	@echo "  make release TAG=v0.1.0  Tag and push a release"
	@echo ""
