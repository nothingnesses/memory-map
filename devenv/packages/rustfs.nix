{
  lib,
  rustPlatform,
  rustfsSrc,
  openssl,
  pkg-config,
  protobuf,
}:

rustPlatform.buildRustPackage rec {
  pname = "rustfs";
  version = "1.0.0-beta.4";

  src = rustfsSrc;

  cargoLock = {
    lockFile = "${src}/Cargo.lock";
    outputHashes = {
      "mysql_async-0.36.1" = "sha256-DFE+VEK4D6Dl8/PcPJ53UaZ0AbVHD6Kc3dKJcA1ZZrs=";
      "pageant-0.2.0" = "sha256-laML4qIZBA4irXXWXPx7NyTGpEDCHoSalkKwcPSZOJY=";
      "russh-0.60.3" = "sha256-laML4qIZBA4irXXWXPx7NyTGpEDCHoSalkKwcPSZOJY=";
      "russh-cryptovec-0.60.3" = "sha256-laML4qIZBA4irXXWXPx7NyTGpEDCHoSalkKwcPSZOJY=";
      "russh-util-0.52.0" = "sha256-laML4qIZBA4irXXWXPx7NyTGpEDCHoSalkKwcPSZOJY=";
      "s3s-0.14.0-dev" = "sha256-J4EOD90XsvR0B8wJ+5j2v6pQXSVi5fs36j+Ge21Ky+c=";
    };
  };

  nativeBuildInputs = [
    pkg-config
    protobuf
  ];

  buildInputs = [
    openssl
  ];

  cargoBuildFlags = [
    "--package"
    "rustfs"
  ];

  PROTOC = "${protobuf}/bin/protoc";
  RUSTFLAGS = "--cfg tokio_unstable";

  doCheck = false;

  meta = {
    description = "High-performance S3-compatible object storage";
    homepage = "https://rustfs.com";
    license = lib.licenses.asl20;
    mainProgram = "rustfs";
  };
}
