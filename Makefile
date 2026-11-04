CARGO   ?= cargo
OUT     := target/dist
NIGHTLY := -Zbuild-std=core,alloc
# ErisMacInjectLib as cargo resolves it.
INJECT   = $(shell $(CARGO) metadata --format-version 1 | perl -ne 'print $$1 and exit if m{"manifest_path":"([^"]*)/crates/inject/Cargo.toml"}')

export MACOSX_DEPLOYMENT_TARGET := 26.0

.PHONY: all payload thread cli app test accenthost self-test clean

all: payload thread cli app

# One arm64 build, mapped into every app by the daemon (arm64e apps included).
payload:
	$(CARGO) build --release -p payload $(NIGHTLY) --target aarch64-apple-darwin
	mkdir -p $(OUT)
	cp target/aarch64-apple-darwin/release/libcustommaccolors.dylib $(OUT)/

# eris-inject's thread helper. Must be arm64e: it signs the payload thread's start for arm64e apps.
thread:
	$(CARGO) build --release -p eris-inject-thread $(NIGHTLY) --target arm64e-apple-darwin \
		--manifest-path $(INJECT)/Cargo.toml --target-dir target
	mkdir -p $(OUT)
	cp target/arm64e-apple-darwin/release/eris-inject-thread $(OUT)/

cli:
	$(CARGO) build --release -p custommaccolors
	mkdir -p $(OUT)
	cp target/release/custommaccolors $(OUT)/

# The settings window as an app (the same binary; opened from a bundle it shows the window).
app: cli
	rm -rf $(OUT)/CustomMacColors.app
	mkdir -p $(OUT)/CustomMacColors.app/Contents/MacOS
	cp resources/Info.plist $(OUT)/CustomMacColors.app/Contents/
	cp target/release/custommaccolors $(OUT)/CustomMacColors.app/Contents/MacOS/CustomMacColors
	codesign --force --sign - $(OUT)/CustomMacColors.app

test:
	$(CARGO) test

# A test app showing the controls that follow the accent, captured to PNGs.
accenthost: payload
	clang -fobjc-arc -framework AppKit tools/accenthost.m -o $(OUT)/accenthost

# Maps the payload into the CLI itself and starts it, the way the daemon does (no root needed),
# and checks the thread helper can start a signed thread.
self-test: all
	$(OUT)/custommaccolors self-test $(OUT)/libcustommaccolors.dylib
	$(OUT)/eris-inject-thread --self-test

clean:
	$(CARGO) clean
