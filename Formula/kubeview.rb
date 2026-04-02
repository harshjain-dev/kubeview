class Kubeview < Formula
  desc "Terminal UI for Kubernetes — pods, logs, exec, port-forward, secrets and more"
  homepage "https://github.com/harshjain-dev/kubeview"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/harshjain-dev/kubeview/releases/download/v#{version}/kubeview-macos-aarch64.tar.gz"
      sha256 "f5f3270c5e8bf789ba460bdcb34bf09763b17f05e2e5ad7016725c7e7d317fb9"
    end
    on_intel do
      url "https://github.com/harshjain-dev/kubeview/releases/download/v#{version}/kubeview-macos-x86_64.tar.gz"
      sha256 "0aae8a04353bf946cfe959a67fec6a59bc25fe0d7e72bffcef3cc44b84f9ec80"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/harshjain-dev/kubeview/releases/download/v#{version}/kubeview-linux-aarch64.tar.gz"
      sha256 "07807aeaa2ac939b611c7d1a2341bc164af86955b7062207b3b0e77ea4e7c2ab"
    end
    on_intel do
      url "https://github.com/harshjain-dev/kubeview/releases/download/v#{version}/kubeview-linux-x86_64.tar.gz"
      sha256 "06191614374adbc9663900a98c883673e94f6ed9f07df0e895fcd84001bacee1"
    end
  end

  def install
    bin.install "kubeview"
  end

  test do
    assert_match "kubeview 0.1.0", shell_output("#{bin}/kubeview --version")
  end
end
