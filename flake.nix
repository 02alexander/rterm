{
  description = "A serial monitor with support for graphs";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, naersk, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let 
          pkgs = nixpkgs.legacyPackages.${system}; 
          naersk' = pkgs.callPackage naersk {};
        in rec
        {
          pkg = naersk'.buildPackage {
            src = ./.;
          };
          packages.rterm = pkg;
          defaultPackage = pkg;

          devShells.default = pkgs.mkShell {
            buildInputs = [pkgs.cargo pkgs.rustc];
          };      
        }
      );

}
