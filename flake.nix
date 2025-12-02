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

        # 1. The Derivation: Fetches source and compiles ONLY the generator
        generatorJar = pkgs.stdenv.mkDerivation {
          pname = "1brc-generator";
          version = "1.0.0";

          src = onebrc;

          nativeBuildInputs = [ pkgs.jdk21_headless ];

          buildPhase = ''
            mkdir -p classes

            # Compile the specific generator class.
            # We include the sourcepath to handle package resolution if needed.
            javac -d classes \
                  -sourcepath src/main/java \
                  src/main/java/dev/morling/onebrc/CreateMeasurements.java

            # Create the JAR
            jar cf generator.jar -C classes .
          '';

          installPhase = ''
            mkdir -p $out/share/java
            cp generator.jar $out/share/java/
          '';
        };

        # 1.5 The Solution Derivation: Compiles the baseline Java solution
        solutionJar = pkgs.stdenv.mkDerivation {
          pname = "1brc-solution-baseline";
          version = "1.0.0";

          src = onebrc;

          nativeBuildInputs = [ pkgs.jdk21_headless ];

          buildPhase = ''
            mkdir -p classes

            # Compile the baseline solution
            javac -d classes \
                  -sourcepath src/main/java \
                  src/main/java/dev/morling/onebrc/CalculateAverage_baseline.java

            jar cf solution.jar -C classes .
          '';

          installPhase = ''
            mkdir -p $out/share/java
            cp solution.jar $out/share/java/
          '';
        };

        # 2. The Script: Wraps the JAR execution
        generateScript = pkgs.writeShellScriptBin "generate-measurements" ''
          set -e
          ROWS="''${1:-1000000000}" # Default to 1 billion if no arg provided

          echo "Generating $ROWS rows using the Nix-built generator..."

          ${pkgs.jdk21_headless}/bin/java \
            -cp ${generatorJar}/share/java/generator.jar \
            dev.morling.onebrc.CreateMeasurements \
            "$ROWS"

          echo "Done. File created at ./measurements.txt"
        '';

        # 3. The Solution Script
        solutionScript = pkgs.writeShellScriptBin "run-solution" ''
          set -e

          # The baseline implementation expects measurements.txt in the CWD
          if [ ! -f "./measurements.txt" ]; then
            echo "Error: ./measurements.txt not found."
            echo "Please run 'nix run' first to generate the data."
            exit 1
          fi

          echo "Running Java Baseline Solution..."
          ${pkgs.jdk21_headless}/bin/java \
            -cp ${solutionJar}/share/java/solution.jar \
            dev.morling.onebrc.CalculateAverage_baseline
        '';

      in
      {
        # Expose the package
        packages.default = generateScript;
        packages.solution = solutionScript;

        # Expose as a runnable app
        apps.default = flake-utils.lib.mkApp {
          drv = generateScript;
        };

        apps.solution = flake-utils.lib.mkApp {
          drv = solutionScript;
        };

        formatter = treefmtEval.config.build.wrapper;

        # Keep the devShell for exploring
        devShells.default = pkgs.mkShell {
          buildInputs = [
            generateScript
            pkgs.jdk21_headless
            pkgs.nixd
            rust
          ]
          ++ (with pkgs; [
            cargo-expand
            cargo-limit
            perf
            cargo-flamegraph
          ])
          ++ (with rustPlatform; [
            bindgenHook
            cargoCheckHook
            cargoBuildHook
          ]);
        };
      }
    );
}
