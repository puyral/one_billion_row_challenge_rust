{
  description = "1 Billion Row Challenge (1BRC) Generator";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    onebrc = {
      url = "github:gunnarmorling/1brc";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      treefmt-nix,
      onebrc,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rust = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.complete);
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rust;
          rustc = rust;
        };
        treefmtEval = treefmt-nix.lib.evalModule pkgs ./nix/fmt.nix;

        e-packages = (import ./nix/packages.nix) { inherit pkgs onebrc; };
      in
      rec {
        # Expose the package
        packages = e-packages;

        # Expose as a runnable app
        # apps.default = flake-utils.lib.mkApp {
        #   drv = generateScript;
        # };

        # apps.solution = flake-utils.lib.mkApp {
        #   drv = solutionScript;
        # };
        # apps.friend = flake-utils.lib.mkApp { drv = friendScript; };

        apps = builtins.mapAttrs (n: drv: flake-utils.lib.mkApp { inherit drv; }) (
          with packages;
          {
            solution = solutionScript;
            friend = friendScript;
            default = generateScript;
            fastest_java = fastestJava;
          }
        );

        formatter = treefmtEval.config.build.wrapper;

        # Keep the devShell for exploring
        devShells.default = pkgs.mkShell {
          buildInputs = [
            packages.generateScript
            packages.fastestJava
            packages.solutionScript
            pkgs.graalvmPackages.graalvm-ce
            pkgs.nixd
            rust
          ]
          ++ (with pkgs; [
            cargo-expand
            cargo-limit
            cargo-flamegraph
            hyperfine
          ])
          ++ (with pkgs; lib.optional (!stdenv.isDarwin) perf)
          ++ (with pkgs; lib.optional (!stdenv.isDarwin) packages.friendScript)
          ++ (with rustPlatform; [
            bindgenHook
            cargoCheckHook
            cargoBuildHook
          ]);
        };
      }
    );
}
