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
  devToolchain = baseToolchain.override {extensions = ["rust-src"];};
in {
  # Built-in rustfmt/clippy hooks assume Cargo.toml at repo root; ours lives
  # in rust/, and cargo-fmt's --manifest-path doesn't resolve targets
  # correctly (a known cargo-fmt limitation, unlike other cargo subcommands),
  # so these cd into the workspace instead.
  pre-commit.settings.hooks.rustfmt = {
    enable = true;
    # cargo's subcommand search (cargo-fmt, cargo-clippy) needs the
    # toolchain's bin/ on PATH — invoking `${devToolchain}/bin/cargo`
    # directly by store path bypasses that search entirely.
    entry = "${pkgs.bash}/bin/bash -c 'export PATH=${devToolchain}/bin:$PATH; cd rust && cargo fmt --check'";
    files = "^rust/.*\\.rs$";
    pass_filenames = false;
    language = "system";
  };

  pre-commit.settings.hooks.clippy = {
    enable = true;
    entry = "${pkgs.bash}/bin/bash -c 'export PATH=${devToolchain}/bin:$PATH; cd rust && cargo clippy --all-targets --all-features -- -D warnings'";
    files = "^rust/.*\\.rs$";
    pass_filenames = false;
    language = "system";
  };

  # CI-usable alternative to the rustfmt hook above: bare rustfmt per file,
  # no cargo workspace resolution needed (avoids cargo-fmt's --manifest-path
  # bug and needs no network/registry access, unlike `cargo fmt`/`cargo
  # clippy` — see nix/modules/automation.nix in crazy-train-code for the
  # pattern this mirrors). Edition is hardcoded since bare rustfmt can't read
  # it from Cargo.toml the way cargo-fmt does — keep in step with
  # `package.edition` in rust/crates/*/Cargo.toml.
  packages.fmt-check-ci = pkgs.writeShellApplication {
    name = "fmt-check-ci";
    runtimeInputs = [devToolchain pkgs.git];
    text = ''
      cd "$(git rev-parse --show-toplevel)"
      mapfile -t files < <(git ls-files 'rust/*.rs')
      rustfmt --edition 2024 --check "''${files[@]}"
    '';
  };

  devShells.falanx = pkgs.mkShell {
    packages = [
      devToolchain
      pkgs.cargo-nextest
      pkgs.cargo-audit
      pkgs.ast-grep
      pkgs.git
      pkgs.jq
    ];

    shellHook = ''
      ${config.pre-commit.installationScript}

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
