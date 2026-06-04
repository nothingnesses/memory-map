{
  lib,
  stdenvNoCC,
  fetchurl,
  unzip,
}:

let
  # TODO: RustFS is still pinned to a pre-1.0 release. When bumping this
  # version, revalidate the process-compose readiness assertion plus the S3
  # contract used by the app: bucket bootstrap, put/head object behavior,
  # multipart upload and abort, presigned upload-part PUTs, bucket CORS for
  # browser PUTs, and exposed ETag headers for multipart completion.
  version = "1.0.0-beta.4";
  assets = {
    aarch64-darwin = {
      name = "rustfs-macos-aarch64-v${version}.zip";
      hash = "sha256-NtdXf3tm17REnadhKzSSy/WdDuqcvxOYT6iK72Qz28Q=";
    };
    aarch64-linux = {
      name = "rustfs-linux-aarch64-musl-v${version}.zip";
      hash = "sha256-ORpcshno95IkugWPCpVHISpIDPrGWd2WF0BUR5EuaGs=";
    };
    x86_64-darwin = {
      name = "rustfs-macos-x86_64-v${version}.zip";
      hash = "sha256-iZNdTcT2XVVQVK7tWsLyqUECEBKVaLtXYCFecLqEiU4=";
    };
    x86_64-linux = {
      name = "rustfs-linux-x86_64-musl-v${version}.zip";
      hash = "sha256-Aqt3ctoxv4WWTMmXvVFNu9oZHuGZGT5yOU8fRf4U0pg=";
    };
  };
  asset =
    assets.${stdenvNoCC.hostPlatform.system}
      or (throw "RustFS ${version} has no release asset for ${stdenvNoCC.hostPlatform.system}");
in
stdenvNoCC.mkDerivation {
  pname = "rustfs";
  inherit version;

  src = fetchurl {
    url = "https://github.com/rustfs/rustfs/releases/download/${version}/${asset.name}";
    inherit (asset) hash;
  };

  nativeBuildInputs = [ unzip ];

  dontConfigure = true;
  dontBuild = true;

  unpackPhase = ''
    runHook preUnpack

    unzip -q "$src" -d source

    runHook postUnpack
  '';

  installPhase = ''
    runHook preInstall

    rustfs_bin="$(find source -type f -name rustfs | head -n 1)"
    if [ -z "$rustfs_bin" ]; then
      echo "ERROR: RustFS binary not found in release archive." >&2
      find source -maxdepth 3 -type f >&2
      exit 1
    fi

    install -Dm755 "$rustfs_bin" "$out/bin/rustfs"

    runHook postInstall
  '';

  meta = {
    description = "High-performance S3-compatible object storage";
    homepage = "https://rustfs.com";
    license = lib.licenses.asl20;
    mainProgram = "rustfs";
  };
}
