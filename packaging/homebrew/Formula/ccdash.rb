class Ccdash < Formula
  desc "Local desktop dashboard for managing Claude Code sessions, projects, and ports"
  homepage "https://github.com/cjtaylor/ccdash"
  version "0.1.0"

  # Source-build formula. When precompiled release artifacts are hosted,
  # replace `url` and update `sha256`.
  url "https://github.com/cjtaylor/ccdash/archive/refs/tags/v#{version}.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  license "MIT"

  depends_on "rust" => :build
  depends_on "node" => :build
  depends_on "pnpm" => :build
  depends_on "tmux"

  def install
    system "cargo", "build", "--release",
           "-p", "ccdash-daemon",
           "-p", "ccdash-cli",
           "-p", "ccdash-ui"

    cd "apps/ccdash-ui/ui" do
      system "pnpm", "install", "--frozen-lockfile"
      system "pnpm", "run", "build"
    end

    bin.install "target/release/ccdash"
    bin.install "target/release/ccdash-daemon"
    bin.install "target/release/ccdash-ui"

    pkgshare.install "packaging/launchd/com.ccdash.daemon.plist"
    pkgshare.install "packaging/systemd/ccdash-daemon.service"
    pkgshare.install "packaging/scripts/install-service.sh"
    pkgshare.install "packaging/scripts/uninstall-service.sh"
  end

  service do
    run [opt_bin/"ccdash-daemon", "--log-level", "info"]
    keep_alive true
    log_path var/"log/ccdash/daemon.out.log"
    error_log_path var/"log/ccdash/daemon.err.log"
  end

  def post_install
    system Formula["bash"].opt_bin/"bash",
           pkgshare/"install-service.sh",
           HOMEBREW_PREFIX.to_s
  rescue => e
    opoo "Could not auto-install service (#{e.message}). Run manually:"
    opoo "  #{pkgshare}/install-service.sh #{HOMEBREW_PREFIX}"
  end

  test do
    system "#{bin}/ccdash", "--version"
    system "#{bin}/ccdash-daemon", "--help"
  end
end
