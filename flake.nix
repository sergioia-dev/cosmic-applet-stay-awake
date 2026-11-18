{
  description = "COSMIC applet that keeps the system awake while the lid is closed";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      crane,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        inherit (pkgs) lib;

        # libcosmic dlopens these, so the linker has to keep them as dependencies.
        linkLibs = with pkgs; [
          libglvnd
          libxkbcommon
          libX11
          libXcursor
          libXi
          libxcb
          wayland
          vulkan-loader
        ];

        commonArgs = {
          pname = "cosmic-applet-stay-awake";
          version = "0.1.0";
          src = ./.;
          strictDeps = true;
          nativeBuildInputs = with pkgs; [
            pkg-config
            libcosmicAppHook
          ];
          buildInputs = with pkgs; [
            fontconfig
            freetype
            mesa
          ] ++ linkLibs;
        };

        craneLib = crane.mkLib pkgs;
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        package = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            postInstall = ''
              install -Dm644 target/xdgen/app.desktop $out/share/applications/com.user.CosmicAppletStayAwake.desktop
              install -Dm644 target/xdgen/app.metainfo.xml $out/share/metainfo/com.user.CosmicAppletStayAwake.metainfo.xml
              install -Dm644 resources/icon.svg $out/share/icons/hicolor/scalable/apps/com.user.CosmicAppletStayAwake.svg
            '';
            meta = {
              description = "Keep the system awake while the lid is closed.";
              license = lib.licenses.mpl20;
              mainProgram = "cosmic-applet-stay-awake";
              platforms = lib.platforms.linux;
            };
          }
        );
      in
      {
        packages.default = package;
        packages.cosmic-applet-stay-awake = package;

        apps.default = {
          type = "app";
          program = "${package}/bin/cosmic-applet-stay-awake";
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            clippy
            just
            rustc
            rustfmt
          ];
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = with pkgs; [
            fontconfig
            freetype
            mesa
          ] ++ linkLibs;
          RUSTFLAGS = lib.concatStringsSep " " (
            map (libName: "-C link-arg=-l${libName}") [
              "EGL"
              "xkbcommon"
              "X11"
              "X11-xcb"
              "Xcursor"
              "Xi"
              "xcb"
              "wayland-client"
              "wayland-egl"
            ]
          );
        };
      }
    );
}
