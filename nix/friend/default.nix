{
  stdenv,
  gcc14,
  writeShellScriptBin,
  ...
}:
rec {
  # 4. friends C++ Solution (Optimized)
  friendSolution = stdenv.mkDerivation {
    pname = "1brc-solution-friend";
    version = "1.0.0";

    # Assumes friend_solution.ccp is in the same directory as flake.nix
    src = ./.;

    # GCC 14 required for C++23 <print> support
    nativeBuildInputs = [ gcc14 ];

    buildPhase = ''
      # Compilation Flags Explanation:
      # -std=c++23      : Required for <print> and <stdfloat>
      # -O3             : Maximum general optimization level
      # -march=native   : Optimize specifically for THIS cpu (SIMD, AVX, etc.)
      # -fopenmp        : Enable multi-threading via OpenMP
      # -flto           : Link Time Optimization

      echo "Compiling C++ solution with high optimizations..."
      g++ -std=c++23 -O3 -march=native -fopenmp -flto \
          friend_solution.cpp -o friend
    '';

    installPhase = ''
      mkdir -p $out/bin
      cp friend $out/bin/
    '';
  };

  friendScript = writeShellScriptBin "run-friend" ''
    set -e
    if [ ! -f "./measurements.txt" ]; then
      echo "Error: ./measurements.txt not found. Run 'nix run' first."
      exit 1
    fi

    echo "Running friends Optimized C++ Solution..."
    ${friendSolution}/bin/friend
  '';
}
