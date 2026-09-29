IMAGE   := ofg-dev
UID     := $(shell id -u)
GID     := $(shell id -g)
PROJECT := openfortivpn-gui

DRUN := docker run --rm -u $(UID):$(GID) -e HOME=/tmp -e CARGO_HOME=/work/.cargo-home \
        -v $(CURDIR):/work -w /work $(IMAGE)

.PHONY: dev-image icons fmt clippy test build dev-run install-user uninstall clean

dev-image:
	docker build -t $(IMAGE) -f docker/Dockerfile .

icons: dev-image
	$(DRUN) bash assets/gen-icons.sh

fmt: dev-image
	$(DRUN) cargo fmt

clippy: dev-image icons
	$(DRUN) cargo clippy -- -D warnings

test: dev-image icons
	$(DRUN) cargo test

build: dev-image icons
	$(DRUN) cargo build --release
	@mkdir -p dist
	@cp target/release/$(PROJECT) dist/

dev-run: dev-image icons
	@xhost +si:localuser:$(USER) >/dev/null 2>&1 || true
	docker run --rm -it -u $(UID):$(GID) -e HOME=/tmp -e CARGO_HOME=/work/.cargo-home \
	  -v $(CURDIR):/work -w /work \
	  -e DISPLAY=$(DISPLAY) -v /tmp/.X11-unix:/tmp/.X11-unix \
	  -e XDG_RUNTIME_DIR=/run/user/$(UID) -v /run/user/$(UID):/run/user/$(UID) \
	  $(IMAGE) cargo run

install-user: build
	./packaging/install-user.sh

uninstall:
	./packaging/install-user.sh --uninstall

clean:
	$(DRUN) cargo clean || true
	rm -rf dist