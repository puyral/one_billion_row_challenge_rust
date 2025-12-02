{
  onebrc,
  # mkDerivation,
  jdk21_headless,
  writeShellScriptBin,
  stdenv,

  ...
}:
let
  mkDerivation = stdenv.mkDerivation;
in
rec {
  # 1. The Derivation: Fetches source and compiles ONLY the generator
  generatorJar = mkDerivation {
    pname = "1brc-generator";
    version = "1.0.0";

    src = onebrc;

    nativeBuildInputs = [ jdk21_headless ];

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
  solutionJar = mkDerivation {
    pname = "1brc-solution-baseline";
    version = "1.0.0";

    src = onebrc;

    nativeBuildInputs = [ jdk21_headless ];

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
  generateScript = writeShellScriptBin "generate-measurements" ''
    set -e
    ROWS="''${1:-1000000000}" # Default to 1 billion if no arg provided

    echo "Generating $ROWS rows using the Nix-built generator..."

    ${jdk21_headless}/bin/java \
      -cp ${generatorJar}/share/java/generator.jar \
      dev.morling.onebrc.CreateMeasurements \
      "$ROWS"

    echo "Done. File created at ./measurements.txt"
  '';

  # 3. The Solution Script
  solutionScript = writeShellScriptBin "run-solution" ''
    set -e

    # The baseline implementation expects measurements.txt in the CWD
    if [ ! -f "./measurements.txt" ]; then
      echo "Error: ./measurements.txt not found."
      echo "Please run 'nix run' first to generate the data."
      exit 1
    fi

    echo "Running Java Baseline Solution..."
    ${jdk21_headless}/bin/java \
      -cp ${solutionJar}/share/java/solution.jar \
      dev.morling.onebrc.CalculateAverage_baseline
  '';
}
