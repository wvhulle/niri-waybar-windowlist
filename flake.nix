{
  description = "Waybar CFFI module for niri window list";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      crane,
      rust-overlay,
      ...
    }:
    let
      forEachSystem =
        f:
        nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ] (
          system:
          f rec {
            inherit system;
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ (import rust-overlay) ];
            };
            craneLib = (crane.mkLib pkgs).overrideToolchain (
              pkgs.rust-bin.nightly.latest.default.override {
                extensions = [ "rust-src" ];
              }
            );
          }
        );
    in
    {
      packages = forEachSystem (
        { pkgs, craneLib, ... }:
        let
          commonArgs = {
            src = craneLib.cleanCargoSource ./.;
            strictDeps = true;
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [
              pkgs.dbus
              pkgs.glib
              pkgs.gtk3
              pkgs.wayland
            ];
          };

          deps = craneLib.buildDepsOnly commonArgs;
          package = craneLib.buildPackage (
            commonArgs
            // {
              cargoArtifacts = deps;
              doCheck = false;
              postInstall = ''
                mkdir -p $out/lib
                find target -name "libniri_waybar_windowlist.so" -exec cp {} $out/lib/ \;
              '';
              meta = {
                description = "Waybar CFFI module for niri window list";
                homepage = "https://github.com/wvhulle/niri-waybar-windowlist";
                license = pkgs.lib.licenses.gpl3Plus;
              };
            }
          );
        in
        {
          default = package;
          deps = deps;
        }
      );

      devShells = forEachSystem (
        { pkgs, craneLib, ... }:
        let
          commonArgs = {
            src = craneLib.cleanCargoSource ./.;
            strictDeps = true;
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [
              pkgs.dbus
              pkgs.glib
              pkgs.gtk3
              pkgs.wayland
            ];
          };
        in
        {
          default = craneLib.devShell {
            inputsFrom = [ (craneLib.buildDepsOnly commonArgs) ];
          };
        }
      );
    };
}
