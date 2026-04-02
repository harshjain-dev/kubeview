class Kubeview < Formula
  desc "Terminal UI for Kubernetes — pods, logs, exec, port-forward, secrets and more"
  homepage "https://github.com/harshjain-dev/kubeview"
  version "0.2.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/harshjain-dev/kubeview/releases/download/v#{version}/kubeview-macos-aarch64.tar.gz"
      sha256 "04d3b617e9370d8d2eed427587c0c993f3b263f53f33ab821624af36add322d1"
    end
    on_intel do
      url "https://github.com/harshjain-dev/kubeview/releases/download/v#{version}/kubeview-macos-x86_64.tar.gz"
      sha256 "aa485e0e1409e934b2a7dba814bd98417560bcb739ef7bf190a676b6138d5ca0"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/harshjain-dev/kubeview/releases/download/v#{version}/kubeview-linux-aarch64.tar.gz"
      sha256 "b0a484bcff605aaec94ebaccd93a5eef146c4fc41bb969cee71c4d340713fad3"
    end
    on_intel do
      url "https://github.com/harshjain-dev/kubeview/releases/download/v#{version}/kubeview-linux-x86_64.tar.gz"
      sha256 "63f8697740bbaa8f8157ab22d0c63ea93d3eec547cdc809d079254c5ec053ead"
    end
  end

  def install
    bin.install "kubeview"
  end

  test do
    assert_match "kubeview 0.2.0", shell_output("#{bin}/kubeview --version")
  end
end
