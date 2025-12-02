{
  pkgs,
  onebrc,
  ...
}:
(pkgs.callPackages ./java.nix { inherit onebrc; }) // (pkgs.callPackages ./friend { })
