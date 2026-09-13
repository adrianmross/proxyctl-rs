{ pkgs, ... }:
{
  # No rust-toolchain.toml in the repo, so use the stable toolchain from nixpkgs.
  languages.rust.enable = true;

  packages = with pkgs; [ git pkg-config ];
}
