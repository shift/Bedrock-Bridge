{
  description = "Bedrock-Bridge - Tauri v2 UDP relay for Minecraft Bedrock Edition";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    android-nixpkgs.url = "github:tadfisher/android-nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, android-nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
          config.allowUnfree = true;
          config.android_sdk.accept_license = true;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [ "aarch64-linux-android" "x86_64-linux-android" "armv7-linux-androideabi" "i686-linux-android" ];
        };

        android-sdk = android-nixpkgs.sdk.${system} (sdkPkgs: with sdkPkgs; [
          cmdline-tools-latest
          build-tools-35-0-1
          ndk-28-0-12433566
          platforms-android-35
          platform-tools
        ]);

        ndk-toolchain-bin = "${android-sdk}/share/android-sdk/ndk/28.0.12433566/toolchains/llvm/prebuilt/linux-x86_64/bin";

        # Tauri v2 Linux system dependencies
        tauriDeps = with pkgs; [
          pkg-config
          cmake
          gtk3
          webkitgtk_4_1
          libayatana-appindicator
          librsvg
          openssl
          glib
          cairo
          gdk-pixbuf
          atk
          pango
          mold
        ];

      in {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.nodejs_22
            pkgs.bun
            pkgs.cargo-tauri
            pkgs.cargo-ndk
            pkgs.jdk17
          ] ++ tauriDeps;

          env = {
            WEBKIT_DISABLE_DMABUF_RENDERER = "1";
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath tauriDeps;
            PKG_CONFIG_PATH = pkgs.lib.makeSearchPathOutput "lib" "pkgconfig" tauriDeps;
            JAVA_HOME = "${pkgs.jdk17.home}";

            # Android NDK linkers
            CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = "${ndk-toolchain-bin}/aarch64-linux-android24-clang";
            CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER = "${ndk-toolchain-bin}/armv7a-linux-androideabi24-clang";
            CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER = "${ndk-toolchain-bin}/x86_64-linux-android24-clang";
            CARGO_TARGET_I686_LINUX_ANDROID_LINKER = "${ndk-toolchain-bin}/i686-linux-android24-clang";
          };

          shellHook = ''
            # Setup local writable Android SDK (Nix store is read-only)
            export ANDROID_HOME="$HOME/.local/share/bedrock-bridge/android-sdk"
            export ANDROID_SDK_ROOT="$ANDROID_HOME"
            mkdir -p "$ANDROID_HOME"

            # Symlink Nix SDK components into local writable dir on first run
            NIX_SDK="${android-sdk}/share/android-sdk"
            if [ ! -d "$ANDROID_HOME/platforms" ]; then
              echo "Initializing local Android SDK from Nix store..."
              cp -rs "$NIX_SDK/"* "$ANDROID_HOME/" 2>/dev/null || true
              chmod -R u+w "$ANDROID_HOME"
            fi

            # Write local.properties for Gradle
            if [ -d "src-tauri/gen/android" ]; then
              cat > src-tauri/gen/android/local.properties << PROPS
            sdk.dir=$ANDROID_HOME
            PROPS
            fi

            echo "⛏️  Bedrock-Bridge Dev Shell"
            echo "Rust: $(rustc --version)"
            echo "Cargo: $(cargo --version)"
            echo "Android SDK: $ANDROID_HOME"
            echo ""
            echo "Commands:"
            echo "  Linux:    cargo tauri build"
            echo "  Android:  cargo tauri android init   # first time"
            echo "            cargo tauri android build"
          '';
        };

        formatter = pkgs.nixpkgs-fmt;
      }
    );
}
