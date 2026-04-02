# This file belongs in a SEPARATE repo named: homebrew-kubeview
# Create it at: https://github.com/YOUR_GITHUB_USERNAME/homebrew-kubeview
# Save it as: Formula/kubeview.rb
#
# After creating that repo, users can install with:
#   brew tap YOUR_GITHUB_USERNAME/kubeview
#   brew install kubeview

class Kubeview < Formula
  desc "Terminal UI for Kubernetes — pods, logs, exec, port-forward, secrets and more"
  homepage "https://github.com/YOUR_GITHUB_USERNAME/kubeview"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/YOUR_GITHUB_USERNAME/kubeview/releases/download/v#{version}/kubeview-macos-aarch64.tar.gz"
      sha256 "REPLACE_WITH_SHA256_FROM_RELEASE_ARTIFACTS"
    end
    on_intel do
      url "https://github.com/YOUR_GITHUB_USERNAME/kubeview/releases/download/v#{version}/kubeview-macos-x86_64.tar.gz"
      sha256 "REPLACE_WITH_SHA256_FROM_RELEASE_ARTIFACTS"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/YOUR_GITHUB_USERNAME/kubeview/releases/download/v#{version}/kubeview-linux-aarch64.tar.gz"
      sha256 "REPLACE_WITH_SHA256_FROM_RELEASE_ARTIFACTS"
    end
    on_intel do
      url "https://github.com/YOUR_GITHUB_USERNAME/kubeview/releases/download/v#{version}/kubeview-linux-x86_64.tar.gz"
      sha256 "REPLACE_WITH_SHA256_FROM_RELEASE_ARTIFACTS"
    end
  end

  def install
    bin.install "kubeview"
  end

  test do
    assert_match "kubeview", shell_output("#{bin}/kubeview --version 2>&1", 1)
  end
end
