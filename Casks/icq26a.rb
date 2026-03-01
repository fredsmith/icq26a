cask "icq26a" do
  version "2026.3.1-3"

  on_arm do
    url "https://github.com/fredsmith/icq26a/releases/download/v#{version}/ICQ26a_#{version}_aarch64.dmg"
    sha256 "73719e8170bbd50d0f3f70b9015fb5380ad238fa560b09052564a6ee7e819f5f" # :arm64
  end
  on_intel do
    url "https://github.com/fredsmith/icq26a/releases/download/v#{version}/ICQ26a_#{version}_x64.dmg"
    sha256 "db6382c550c02013b0cc167267e21e3388f8d822fd273984733b2df91cd93be4" # :x64
  end

  name "ICQ26a"
  desc "Matrix chat client styled after ICQ 98a"
  homepage "https://github.com/fredsmith/icq26a"

  app "ICQ26a.app"

  caveats <<~EOS
    #{token} is not signed with an Apple Developer certificate.
    On first launch, macOS Gatekeeper will block it. To allow it:
      System Settings > Privacy & Security > scroll down > click "Open Anyway"
    Or run:
      xattr -d com.apple.quarantine /Applications/ICQ26a.app
  EOS

  zap trash: ["~/Library/Application Support/com.icq26a.app"]
end
