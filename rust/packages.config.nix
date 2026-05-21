{
  inputs,
  lib,
  self,
  ...
}: {
  pkgs,
  system,
  ...
}: let
  rustBin = inputs.rust-overlay.lib.mkRustBin {} pkgs.buildPackages;
  toolchainData = (builtins.fromTOML (builtins.readFile ./rust-toolchain.toml)).toolchain;
  baseToolchain = rustBin.fromRustupToolchain toolchainData;

  crossEnvFor = target: crossPkgs: let
    cc = crossPkgs.stdenv.cc;
    prefix = cc.targetPrefix;
    upper = lib.toUpper (lib.replaceStrings ["-"] ["_"] target);
  in {
    "CC_${target}" = "${cc}/bin/${prefix}cc";
    "CXX_${target}" = "${cc}/bin/${prefix}c++";
    "AR_${target}" = "${cc}/bin/${prefix}ar";
    "CARGO_TARGET_${upper}_LINKER" = "${cc}/bin/${prefix}cc";
  };

  falanxBuild = {
    target ? null,
    crossPkgs ? null,
  }: let
    toolchain =
      if target == null
      then baseToolchain
      else baseToolchain.override {targets = [target];};
    craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (_: toolchain);
    targetArgs = if target == null then {} else {CARGO_BUILD_TARGET = target;};
    crossArgs = if crossPkgs == null then {} else crossEnvFor target crossPkgs;
    commonArgs =
      {
        pname = "falanx-engine";
        src = craneLib.cleanCargoSource ./.;
        cargoExtraArgs = "-p falanx-engine";
        strictDeps = true;
      }
      // targetArgs
      // crossArgs;
    cargoArtifacts = craneLib.buildDepsOnly commonArgs;
  in
    craneLib.buildPackage (commonArgs
      // {
        inherit cargoArtifacts;
        doCheck = false;
        NIX_SHORT_COMMIT = self.shortRev or (lib.removeSuffix "-dirty" self.dirtyShortRev);
        meta.mainProgram = "falanx-engine";
      });

  docker-build = arch: package:
    pkgs.dockerTools.streamLayeredImage {
      name = "falanx/engine";
      tag = "${package.version}-${arch}";
      created = "now";
      contents = [package pkgs.cacert];
      config = {
        EntryPoint = ["/bin/falanx-engine"];
        Cmd = [];
        Env = ["SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"];
      };
    };

  platformPackages =
    if system == "x86_64-linux"
    then rec {
      falanx-engine-x86_64-linux = falanxBuild {target = "x86_64-unknown-linux-gnu";};
      falanx-engine-docker-amd64 = docker-build "amd64" falanx-engine-x86_64-linux;
    }
    else if system == "aarch64-linux"
    then rec {
      falanx-engine-aarch64-linux = falanxBuild {target = "aarch64-unknown-linux-gnu";};
      falanx-engine-docker-arm64 = docker-build "arm64" falanx-engine-aarch64-linux;
    }
    else if system == "aarch64-darwin"
    then rec {
      falanx-engine-aarch64-linux = falanxBuild {
        target = "aarch64-unknown-linux-gnu";
        crossPkgs = pkgs.pkgsCross.aarch64-multiplatform;
      };
      falanx-engine-docker-arm64 = docker-build "arm64" falanx-engine-aarch64-linux;
    }
    else if system == "x86_64-darwin"
    then rec {
      falanx-engine-x86_64-linux = falanxBuild {
        target = "x86_64-unknown-linux-gnu";
        crossPkgs = pkgs.pkgsCross.gnu64;
      };
      falanx-engine-docker-amd64 = docker-build "amd64" falanx-engine-x86_64-linux;
    }
    else {};
in {
  packages =
    platformPackages
    // {
      falanx-engine = falanxBuild {};
    };
}
