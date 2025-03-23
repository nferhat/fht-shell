{
  description = "A custom shell written for fht-compositor";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {self, ...}:
    inputs.flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux"]; # TODO: aarch64? though I don't use it.
      perSystem = {
        self',
        pkgs,
        inputs',
        ...
      }: {
        # NOTE: This is for the Nix code formatter!!
        formatter = pkgs.alejandra;

        devShells.default = let
          rust-bin = inputs.rust-overlay.lib.mkRustBin {} pkgs;
        in
          pkgs.mkShell {
            packages = with pkgs; [
              # For developement purposes, a nightly toolchain is preferred.
              # We use nightly cargo for formatting, though compiling is limited to
              # whatever is specified inside ./rust-toolchain.toml
              (rust-bin.selectLatestNightlyWith (toolchain:
                toolchain.default.override {
                  extensions = ["rust-analyzer" "rust-src" "rustc-codegen-cranelift-preview"];
                }))

              pkgs.alejandra # for formatting this flake if needed
              librsvg
              glib
              gtk4
              gtk4-layer-shell
              libadwaita
              pipewire
              wireplumber
            ];

            nativeBuildInputs = with pkgs; [rustPlatform.bindgenHook pkg-config];
          };
      };
    };
}
