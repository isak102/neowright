{
  pkgs,
  lib,
  config,
  ...
}:
{
  languages.rust = {
    enable = true;
    channel = "stable";
    version = "1.96.0";
  };

  packages = [
    pkgs.bump-my-version
    pkgs.just
  ];
}
