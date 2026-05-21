{
  inputs,
  lib,
  self,
  ...
}: {
  pkgs,
  config,
  system,
  ...
}: let
  rustBin = inputs.rust-overlay.lib.mkRustBin {} pkgs.buildPackages;
  toolchainData = (builtins.fromTOML (builtins.readFile ./rust-toolchain.toml)).toolchain;
  baseToolchain = rustBin.fromRustupToolchain toolchainData;
in {
  devShells.falanx = pkgs.mkShell {
    packages = [
      (baseToolchain.override {extensions = ["rust-src"];})
      pkgs.cargo-nextest
      pkgs.cargo-audit
      pkgs.ast-grep
      pkgs.git
      pkgs.jq
    ];

    shellHook = ''
      validate() {
        cargo fmt --check &&
        cargo clippy -- -D warnings &&
        cargo nextest run --no-tests=pass
      }

      validate-full() {
        validate &&
        cargo audit
      }

      export -f validate
      export -f validate-full

      echo "Falanx dev shell ready. Commands: validate, validate-full"
    '';
  };
}
