DEB_BUILD_PATH ?= target/debian/zenoh-gamepad_*.deb


.PHONY: build
build:
	cargo build --release

.PHONY: build-xinput
build-xinput:
	cargo build --release --features xinput --no-default-features

.PHONE: install
install: build-deb
	cargo install

.PHONY: install-dependencies
install-dependencies:
	sudo apt update && sudo apt install libudev-dev

.PHONY: install-dependencies-steam-deck
install-dependencies-steam-deck:
	@echo "disable readonly file system"
	sudo steamos-readonly disable

	@echo "Generate pgp keys for repos"
	sudo pacman-key --init
	sudo pacman-key --populate archlinux
	sudo pacman-key --populate holo

	@echo "Install build essentials and tooling"
	sudo pacman --sync --noconfirm base-devel glibc linux-api-headers cmake

	@echo "re-enable read only"
	sudo steamos-readonly enable


.PHONY: install-desktop
install-desktop:
	cargo install --path .
	cp desktop/hamilton.desktop ~/.local/share/applications/
	ln -sf ~/.local/share/applications/hamilton.desktop ~/Desktop/hamilton.desktop
