{
  inputs = {
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
    };
    import-tree = {
      url = "github:vic/import-tree";
    };
    nixpkgs = {
      url = "github:nixos/nixpkgs/nixpkgs-unstable";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake {inherit inputs;} ({
      inputs,
      lib,
      self,
      ...
    }: let
      import-tree = inputs.import-tree.new.initFilter (
        p:
          lib.hasSuffix ".nix" p
          && !lib.hasInfix "/helpers/" p
          && !lib.hasSuffix ".disabled.nix" p
          && !lib.hasSuffix "flake.nix" p
      );
      is-config = i: i.filter (p: lib.hasSuffix "/config.nix" p || lib.hasSuffix ".config.nix" p);
      load-config = i:
        i.map (p: (inputs.flake-parts.lib.importApply p {inherit self lib inputs;}));
    in {
      imports = [inputs.git-hooks.flakeModule];

      perSystem = {config, ...}: {
        imports = [
          (lib.pipe import-tree [is-config load-config] ./.)
          ({
            config,
            pkgs,
            ...
          }: {
            devShells.default = pkgs.mkShell {
              inputsFrom = lib.pipe (config.devShells or {}) [
                (lib.filterAttrs (k: v: k != "default" && !(lib.hasPrefix "ci-" k)))
                lib.attrValues
              ];
            };
          })
        ];
      };

      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
    });
}
