{
  onebrc,
  writeShellScriptBin,
  stdenv,
  pkgs, # Add pkgs here
  ...
}:
let
  mkDerivation = stdenv.mkDerivation;
  graalvm = pkgs.graalvmPackages.graalvm-ce; # Define graalvm here
  mkJar =
    name: path:
    mkDerivation {
      pname = name;
      version = "1.0.0";

      src = onebrc;

      nativeBuildInputs = [ graalvm ]; # Use graalvm here

      buildPhase = ''
        mkdir -p classes

        # Compile the specific generator class.
        # We include the sourcepath to handle package resolution if needed.
        ${graalvm}/bin/javac --enable-preview --release 25 -d classes \
              -sourcepath src/main/java \
              ${path}
              # src/main/java/dev/morling/onebrc/CreateMeasurements.java

        # Create the JAR
        ${graalvm}/bin/jar cf ${name}.jar -C classes .
      '';

      installPhase = ''
        mkdir -p $out/share/java
        cp ${name}.jar $out/share/java/
      '';
    };

  mkScript =
    name: path: java_path:
    let
      jar = mkJar name path;
    in
    writeShellScriptBin "run-${name}" ''
      set -e

      # The baseline implementation expects measurements.txt in the CWD
      if [ ! -f "./measurements.txt" ]; then
        echo "Error: ./measurements.txt not found."
        echo "Please run 'nix run' first to generate the data."
        exit 1
      fi

      echo "Running Java Baseline Solution..."
      ${graalvm}/bin/java --enable-preview --release 25 \
        -cp ${jar}/share/java/${name}.jar \
        ${java_path}
        # dev.morling.onebrc.CalculateAverage_baseline
    '';
in
{
  generateScript =
    let
      generatorJar = mkJar "generator" "src/main/java/dev/morling/onebrc/CreateMeasurements.java";
    in
    writeShellScriptBin "generate-measurements" ''
      set -e
      ROWS="''${1:-1000000000}" # Default to 1 billion if no arg provided

      echo "Generating $ROWS rows using the Nix-built generator..."

      ${graalvm}/bin/java --enable-preview --release 25 \
        -cp ${generatorJar}/share/java/generator.jar \
        dev.morling.onebrc.CreateMeasurements \
        "$ROWS"

      echo "Done. File created at ./measurements.txt"
    '';

  solutionScript =
    mkScript "solution" "src/main/java/dev/morling/onebrc/CalculateAverage_baseline.java"
      "dev.morling.onebrc.CalculateAverage_baseline";

  fastestJava =
    mkScript "fastest-java" "src/main/java/dev/morling/onebrc/CalculateAverage_thomaswue.java"
      "dev.morling.onebrc.CalculateAverage_thomaswue";
}
