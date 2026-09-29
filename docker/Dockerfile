FROM archlinux:base-devel

RUN pacman -Syu --noconfirm \
      rust git \
      gtk3 libayatana-appindicator \
      libxkbcommon libxkbcommon-x11 wayland \
      librsvg \
    && rm -rf /var/cache/pacman/pkg

WORKDIR /work